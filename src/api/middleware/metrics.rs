use axum::extract::{MatchedPath, Request};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};
use std::sync::OnceLock;

static PROMETHEUS_HANDLE: OnceLock<metrics_exporter_prometheus::PrometheusHandle> = OnceLock::new();

/// Install the Prometheus exporter and metric descriptions exactly once, and
/// only when `metrics.enabled` is true. Returns the handle used by the
/// `/metrics` endpoint to render the exposition text.
///
/// Uses `get_or_init` so concurrent callers cannot race, and tolerates
/// `SetRecorderError`: if another component already installed a global
/// recorder (e.g. a parallel test harness), we keep our own handle —
/// it still renders this exporter's registry.
pub fn install_prometheus_exporter() -> metrics_exporter_prometheus::PrometheusHandle {
    PROMETHEUS_HANDLE
        .get_or_init(|| {
            let recorder = metrics_exporter_prometheus::PrometheusBuilder::new().build_recorder();
            let handle = recorder.handle();
            let _ = metrics::set_global_recorder(recorder);

            describe_counter!(
                "aether_webhook_ingest_total",
                "Webhook ingest requests by endpoint, provider, and HTTP status"
            );
            describe_histogram!(
                "aether_webhook_ingest_duration_seconds",
                "Ingest handler latency in seconds by endpoint and provider"
            );
            describe_counter!(
                "aether_dispatch_attempts_total",
                "Downstream dispatch attempts by endpoint and outcome"
            );
            describe_counter!(
                "aether_sqlite_write_errors_total",
                "SQLite acquisition or write failures surfaced inside the API"
            );
            describe_gauge!(
                "aether_circuit_breaker_state",
                "Circuit breaker state per endpoint (0=closed, 1=half-open, 2=open)"
            );
            describe_gauge!("aether_dlq_size", "Dead-letter queue depth");

            handle
        })
        .clone()
}

/// Render the Prometheus exposition text. Returns 404 when metrics are
/// disabled so scraping a disabled instance fails loudly instead of
/// presenting an empty document that looks like a zero-traffic service.
pub async fn metrics_endpoint() -> impl IntoResponse {
    match PROMETHEUS_HANDLE.get() {
        Some(handle) => (
            StatusCode::OK,
            [("content-type", "text/plain; version=0.0.4")],
            handle.render(),
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "metrics disabled").into_response(),
    }
}

/// Per-request HTTP metrics. Registered as a layer for every route.
///
/// The `path` label uses the matched route template (e.g.
/// `/v1/ingest/{endpoint_id}`), falling back to `"unmatched"`, so client
/// input can never inflate Prometheus series cardinality.
pub async fn track_metrics(req: Request, next: Next) -> Response {
    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "unmatched".to_string());
    let method = req.method().to_string();

    let response = next.run(req).await;

    let status = response.status().as_u16().to_string();

    counter!(
        "aether_http_requests_total",
        "method" => method,
        "path" => path,
        "status" => status
    )
    .increment(1);

    response
}

/// Ingest-specific metrics recorded by the handler itself, because the
/// provider label is only known after signature parsing.
pub fn record_ingest_metrics(endpoint_id: &str, provider: &str, status: u16, latency_secs: f64) {
    counter!(
        "aether_webhook_ingest_total",
        "endpoint" => endpoint_id.to_string(),
        "provider" => provider.to_string(),
        "status" => status.to_string()
    )
    .increment(1);

    histogram!(
        "aether_webhook_ingest_duration_seconds",
        "endpoint" => endpoint_id.to_string(),
        "provider" => provider.to_string()
    )
    .record(latency_secs);
}

/// Dispatch-loop metrics recorded by the worker on every attempt.
pub fn record_dispatch_metrics(endpoint_id: &str, outcome: &str) {
    counter!(
        "aether_dispatch_attempts_total",
        "endpoint" => endpoint_id.to_string(),
        "outcome" => outcome.to_string()
    )
    .increment(1);
}

/// Circuit breaker state as a gauge: 0=closed, 1=half-open, 2=open.
pub fn set_circuit_breaker_gauge(endpoint_id: &str, state_value: f64) {
    gauge!("aether_circuit_breaker_state", "endpoint" => endpoint_id.to_string()).set(state_value);
}

/// Current dead-letter queue depth.
pub fn set_dlq_gauge(size: f64) {
    gauge!("aether_dlq_size").set(size);
}
