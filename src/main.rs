use aether_relay::{AppState, api, config, db};
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

    let pool = db::create_pool(
        &app_config.database.path,
        app_config.database.pool_size,
        app_config.database.busy_timeout_ms,
        app_config.database.mmap_size,
        app_config.database.cache_size,
    )?;

    let state = AppState {
        pool,
        config: app_config.clone(),
    };

    let app = api::create_router(state);

    let addr: SocketAddr =
        format!("{}:{}", app_config.server.host, app_config.server.port).parse()?;
    info!("Server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
