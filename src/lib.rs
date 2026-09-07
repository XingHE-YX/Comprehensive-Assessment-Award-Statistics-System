pub mod config;
pub mod error;
pub mod state;

use std::{convert::Infallible, path::Path, str::FromStr, time::Duration};

use axum::{
    Router, body::Bytes, extract::DefaultBodyLimit, http::Request, response::IntoResponse,
    routing::get,
};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use thiserror::Error;
use tokio::net::TcpListener;
use tower::service_fn;
use tower_http::services::ServeDir;
use tracing_subscriber::EnvFilter;

pub use config::{AppEnvironment, Config, ConfigError, DEFAULT_MAX_BODY_BYTES};
pub use error::AppError;
pub use state::AppState;

#[derive(Debug, Error)]
pub enum StartupError {
    #[error("应用配置无效：{0}")]
    Configuration(#[from] ConfigError),
    #[error("数据库配置无效")]
    DatabaseConfiguration,
    #[error("数据库初始化失败")]
    DatabaseInitialization,
    #[error("数据库迁移失败")]
    DatabaseMigration,
    #[error("无法监听服务地址")]
    Bind,
    #[error("服务运行异常")]
    Serve,
}

impl StartupError {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Configuration(_) => "configuration",
            Self::DatabaseConfiguration => "database_configuration",
            Self::DatabaseInitialization => "database_initialization",
            Self::DatabaseMigration => "database_migration",
            Self::Bind => "bind",
            Self::Serve => "serve",
        }
    }
}

pub fn build_router(state: AppState) -> Router {
    let max_body_bytes = state.config.max_body_bytes;
    let static_files = ServeDir::new("static").not_found_service(service_fn(static_not_found));

    Router::new()
        .route("/healthz", get(healthz))
        .nest_service("/static", static_files)
        .fallback(not_found)
        .layer(DefaultBodyLimit::max(max_body_bytes))
        .with_state(state)
}

pub async fn run() -> Result<(), StartupError> {
    let config = Config::from_env()?;
    initialize_tracing(&config.rust_log);

    let db = initialize_database(&config).await?;
    let state = AppState::new(db, config);
    let bind_addr = state.config.bind_addr;
    let listener = TcpListener::bind(bind_addr)
        .await
        .map_err(|_| StartupError::Bind)?;

    tracing::info!(%bind_addr, "应用已启动");
    axum::serve(listener, build_router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|_| StartupError::Serve)
}

pub fn initialize_tracing(rust_log: &str) {
    let filter = EnvFilter::try_new(rust_log).unwrap_or_else(|_| EnvFilter::new("info"));
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .finish();

    if tracing::subscriber::set_global_default(subscriber).is_err() {
        tracing::debug!("tracing subscriber was already initialized");
    }
}

async fn initialize_database(config: &Config) -> Result<SqlitePool, StartupError> {
    let options = SqliteConnectOptions::from_str(&config.database_url)
        .map_err(|_| StartupError::DatabaseConfiguration)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    let db = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .map_err(|_| StartupError::DatabaseInitialization)?;

    sqlx::migrate::Migrator::new(Path::new("./migrations"))
        .await
        .map_err(|_| StartupError::DatabaseMigration)?
        .run(&db)
        .await
        .map_err(|_| StartupError::DatabaseMigration)?;

    Ok(db)
}

async fn healthz(_: Bytes) -> &'static str {
    "ok"
}

async fn not_found() -> AppError {
    AppError::not_found()
}

async fn static_not_found<B>(_: Request<B>) -> Result<axum::response::Response, Infallible> {
    Ok(AppError::not_found().into_response())
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};

    let terminate = async {
        match signal(SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };

    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            if result.is_err() {
                tracing::warn!("未能监听中断信号");
            }
        }
        _ = terminate => {}
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    if tokio::signal::ctrl_c().await.is_err() {
        tracing::warn!("未能监听中断信号");
    }
}
