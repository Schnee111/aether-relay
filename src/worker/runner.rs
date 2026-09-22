use crate::config::AppConfig;
use crate::db::DbPool;
use crate::db::models::EventStatus;
use crate::error::AppError;
use crate::worker::backoff::decorrelated_jitter;
use crate::worker::circuit_breaker::SharedCircuitBreakers;
use crate::worker::dispatcher::{dispatch_single_event, requeue_event};
use reqwest::Client;
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};

pub struct DispatchLoopConfig {
    pub poll_interval: Duration,
    pub max_attempts: u32,
    pub backoff_base: Duration,
    pub backoff_cap: Duration,
    pub circuit_failure_threshold: u32,
    pub circuit_recovery: Duration,
}

impl DispatchLoopConfig {
    pub fn from_app_config(cfg: &AppConfig) -> Self {
        Self {
            poll_interval: Duration::from_millis(cfg.worker.poll_interval_ms),
            max_attempts: cfg.worker.max_attempts,
            backoff_base: Duration::from_millis(cfg.worker.backoff_base_ms),
            backoff_cap: Duration::from_millis(cfg.worker.backoff_cap_ms),
            circuit_failure_threshold: cfg.worker.circuit_failure_threshold,
            circuit_recovery: Duration::from_secs(cfg.worker.circuit_recovery_timeout_secs),
        }
    }
}

/// What a single pass of the worker did, so the loop knows whether it may spin
/// straight on to the next event or has to back off first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchOutcome {
    /// The queue was empty.
    Idle,
    /// An event was claimed and its state is now settled: delivered, or parked
    /// in the dead letter queue once its budget ran out.
    Settled,
    /// An event was claimed but withheld because the downstream circuit is open.
    /// It was returned to the queue and will be retried later.
    Deferred,
}

/// Atomically move the oldest RECEIVED event to PROCESSING.
///
/// Uses `BEGIN IMMEDIATE` so two workers can never claim the same row.
/// Returns the `(event_id, endpoint_id)` pair so callers that record
/// per-endpoint observations (metrics, logs) use the stable endpoint
/// identifier rather than the unique event id.
fn claim_next_event(pool: &DbPool) -> Result<Option<(String, String)>, AppError> {
    let mut conn = pool.get().map_err(|e| AppError::Database(e.to_string()))?;

    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| AppError::Database(e.to_string()))?;

    let claimed: Option<(String, String)> = tx
        .query_row(
            "SELECT id, endpoint_id FROM incoming_events WHERE status = ?1 ORDER BY created_at ASC, id ASC LIMIT 1",
            params![EventStatus::Received.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| AppError::Database(e.to_string()))?;

    if let Some((id, _)) = &claimed {
        tx.execute(
            "UPDATE incoming_events SET status = ?1 WHERE id = ?2",
            params![EventStatus::Processing.as_str(), id],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
    }

    tx.commit().map_err(|e| AppError::Database(e.to_string()))?;
    Ok(claimed)
}

/// Claim and dispatch at most one event.
///
/// Exposed separately from the loop so the dispatch path is directly testable
/// without spawning an infinite task.
pub async fn run_dispatch_once(
    pool: &DbPool,
    breakers: &SharedCircuitBreakers,
    client: &Client,
    cfg: &DispatchLoopConfig,
) -> Result<DispatchOutcome, AppError> {
    let claim_pool = pool.clone();
    let claimed = tokio::task::spawn_blocking(move || claim_next_event(&claim_pool))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;

    let Some((event_id, endpoint_id)) = claimed else {
        return Ok(DispatchOutcome::Idle);
    };

    let outcome = dispatch_single_event(
        client,
        pool,
        breakers,
        cfg.circuit_failure_threshold,
        cfg.circuit_recovery,
        cfg.max_attempts,
        &event_id,
    )
    .await;

    let outcome_label = match &outcome {
        Ok(_) => "success",
        Err(_) => "failure",
    };
    crate::api::middleware::metrics::record_dispatch_metrics(&endpoint_id, outcome_label);

    match outcome {
        Ok(result) if result.deferred => Ok(DispatchOutcome::Deferred),
        Ok(_) => Ok(DispatchOutcome::Settled),
        Err(err) => {
            error!(event_id = %event_id, error = %err, "dispatch errored; requeueing event");
            let requeue_pool = pool.clone();
            let id = event_id.clone();
            let _ = tokio::task::spawn_blocking(move || requeue_event(&requeue_pool, &id)).await;
            Err(err)
        }
    }
}

/// Background worker: drain RECEIVED events, spacing retries with the
/// decorrelated-jitter backoff and letting the dispatcher handle DLQ eviction
/// once `max_attempts` is exhausted.
pub async fn run_dispatch_loop(
    pool: DbPool,
    breakers: SharedCircuitBreakers,
    client: Client,
    cfg: DispatchLoopConfig,
) {
    info!(
        poll_interval_ms = cfg.poll_interval.as_millis() as u64,
        max_attempts = cfg.max_attempts,
        "dispatch loop started"
    );

    let mut prev_sleep = cfg.backoff_base;

    loop {
        match run_dispatch_once(&pool, &breakers, &client, &cfg).await {
            Ok(DispatchOutcome::Settled) => {
                // Real progress: keep draining without an artificial delay.
                prev_sleep = cfg.backoff_base;
            }
            Ok(DispatchOutcome::Idle) => sleep(cfg.poll_interval).await,
            Ok(DispatchOutcome::Deferred) | Err(_) => {
                // The downstream is either protecting itself or misbehaving.
                // Backing off here is what stops the requeued event from being
                // re-claimed in a tight loop.
                let delay = decorrelated_jitter(cfg.backoff_base, cfg.backoff_cap, prev_sleep);
                prev_sleep = delay;
                sleep(delay).await;
            }
        }
    }
}
