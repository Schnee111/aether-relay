use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub auth: AuthConfig,
    pub worker: WorkerConfig,
    pub logging: LoggingConfig,
    pub metrics: MetricsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub body_limit_bytes: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub path: String,
    pub busy_timeout_ms: u64,
    pub mmap_size: usize,
    pub cache_size: i64,
    pub pool_size: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    #[serde(default)]
    pub api_keys: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkerConfig {
    pub poll_interval_ms: u64,
    pub max_attempts: u32,
    pub backoff_base_ms: u64,
    pub backoff_cap_ms: u64,
    pub circuit_failure_threshold: u32,
    pub circuit_recovery_timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetricsConfig {
    pub enabled: bool,
}

impl AppConfig {
    pub fn load() -> Result<Self, config::ConfigError> {
        let builder = config::Config::builder()
            .add_source(config::File::with_name("config/default").required(false))
            .add_source(config::Environment::with_prefix("RELAY").separator("__"));

        builder.build()?.try_deserialize()
    }
}
