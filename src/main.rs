use aether_relay::{AppState, api, config, db, worker};
use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app_config = config::AppConfig::load()?;

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&app_config.logging.level));

    if app_config.logging.format == "json" {
        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer())
            .init();
    }

    info!(
        version = env!("CARGO_PKG_VERSION"),
        "Initializing AetherRelay gateway"
    );

    // An empty key list makes the API completely unauthenticated: every request
    // that should carry X-Api-Key is served. That is a valid choice for a
    // loopback-only test run, but it must never be a silent one.
    if app_config.auth.api_keys.is_empty() {
        tracing::warn!(
            "auth.api_keys is empty: the management API is UNAUTHENTICATED. \
             Anyone who can reach this port can create endpoints and replay \
             the dead letter queue. Set RELAY__AUTH__API_KEYS or \
             auth.api_keys before exposing this service."
        );
    }

    let pool = db::create_pool(
        &app_config.database.path,
        app_config.database.pool_size,
        app_config.database.busy_timeout_ms,
        app_config.database.mmap_size,
        app_config.database.cache_size,
    )?;

    // Start the background dispatch worker. Without this the gateway would only
    // ever accept events; nothing would deliver them downstream.
    let breakers = worker::circuit_breaker::create_shared_breakers();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let worker_cfg = worker::runner::DispatchLoopConfig::from_app_config(&app_config);

    tokio::spawn(worker::runner::run_dispatch_loop(
        pool.clone(),
        breakers,
        client,
        worker_cfg,
    ));

    let state = AppState {
        pool,
        config: app_config.clone(),
    };

    let app = api::create_router(state);

    let addr: SocketAddr =
        format!("{}:{}", app_config.server.host, app_config.server.port).parse()?;
    info!("Server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Drain in-flight requests on SIGINT instead of dropping them, so an
/// ingestion that has already returned 202 is not lost mid-write.
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    info!("shutdown signal received, draining connections");
}
