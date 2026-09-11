use std::process::ExitCode;
use tracing_subscriber::{
    filter::{LevelFilter, Targets},
    prelude::*,
};
use zongce_web::{
    config::Config, routes::build_router_with_config, state::AppState, storage::AttachmentStorage,
};

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--hash-secret"] {
        return hash_stdin();
    }
    if !args.is_empty() {
        eprintln!("用法：zongce-web [--hash-secret]");
        return ExitCode::FAILURE;
    }
    let _ = dotenvy::dotenv();
    // Dependency debug logs may contain SQL, session payloads and file paths.
    // Only our audited fields are eligible for output, including at debug level.
    let level = std::env::var("RUST_LOG")
        .ok()
        .and_then(|value| value.parse::<LevelFilter>().ok())
        .unwrap_or(LevelFilter::INFO);
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().json())
        .with(Targets::new().with_target("zongce_web", level))
        .init();
    std::panic::set_hook(Box::new(|_| {
        tracing::error!(event = "panic", status = "failed")
    }));
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(stage) => {
            tracing::error!(event = "startup_failed", stage);
            ExitCode::FAILURE
        }
    }
}

fn hash_stdin() -> ExitCode {
    use std::io::Read;
    let mut input = String::new();
    if std::io::stdin()
        .take(4097)
        .read_to_string(&mut input)
        .is_err()
        || input.len() > 4096
    {
        eprintln!("无法读取口令");
        return ExitCode::FAILURE;
    }
    let line = input.strip_suffix('\n').unwrap_or(&input);
    let secret = line.strip_suffix('\r').unwrap_or(line);
    if secret.is_empty() || secret.contains(['\n', '\r']) {
        eprintln!("请输入一行非空口令");
        return ExitCode::FAILURE;
    }
    match zongce_web::auth::hash_secret(secret) {
        Ok(hash) => {
            println!("{hash}");
            ExitCode::SUCCESS
        }
        Err(_) => {
            eprintln!("生成哈希失败");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), &'static str> {
    let config = Config::from_env().map_err(|error| {
        let (zongce_web::config::ConfigError::Missing(key)
        | zongce_web::config::ConfigError::Invalid(key)) = error;
        tracing::error!(event = "configuration_invalid", key);
        "configuration"
    })?;
    tokio::fs::create_dir_all(&config.upload_dir)
        .await
        .map_err(|_| "upload_directory")?;
    let mut state = AppState::initialize(&config.database_url)
        .await
        .map_err(|_| "database")?;
    state.storage = AttachmentStorage::new(&config.upload_dir);
    zongce_web::services::recycle::cleanup(&state)
        .await
        .map_err(|_| "attachment_cleanup")?;
    let router = build_router_with_config(state.clone(), &config).map_err(|_| "router")?;
    let listener = tokio::net::TcpListener::bind(config.bind_addr)
        .await
        .map_err(|_| "listener")?;
    tracing::info!(event = "startup", status = "ready");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|_| "server")?;
    state.db.close().await;
    tracing::info!(event = "shutdown", status = "complete");
    Ok(())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut terminate) => {
                tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
            }
            Err(_) => {
                tracing::error!(event = "signal_registration_failed");
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
    tracing::info!(event = "shutdown", status = "draining");
}
