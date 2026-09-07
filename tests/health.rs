use std::{net::SocketAddr, path::PathBuf};

use axum::{body::to_bytes, http::StatusCode, response::IntoResponse};
use axum_test::TestServer;
use sqlx::sqlite::SqlitePoolOptions;
use zongce_web::{
    AppEnvironment, AppError, AppState, Config, DEFAULT_MAX_BODY_BYTES, build_router,
};

fn test_config() -> Config {
    Config {
        environment: AppEnvironment::Development,
        bind_addr: SocketAddr::from(([127, 0, 0, 1], 3000)),
        admin_username: "test-admin".to_owned(),
        admin_password_hash: "test-password-hash".to_owned(),
        session_secret: "QUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUE=".to_owned(),
        database_url: "sqlite::memory:".to_owned(),
        upload_dir: PathBuf::from("uploads"),
        cookie_secure: false,
        max_body_bytes: DEFAULT_MAX_BODY_BYTES,
        rust_log: "info".to_owned(),
    }
}

async fn test_state() -> Result<AppState, sqlx::Error> {
    let db = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;

    Ok(AppState::new(db, test_config()))
}

#[tokio::test]
async fn healthz_returns_ok() -> Result<(), Box<dyn std::error::Error>> {
    let server = TestServer::new(build_router(test_state().await?))?;

    let response = server.get("/healthz").await;

    assert_eq!(response.status_code(), StatusCode::OK);
    assert_eq!(response.text(), "ok");
    Ok(())
}

#[tokio::test]
async fn healthz_rejects_request_bodies_above_the_configured_limit()
-> Result<(), Box<dyn std::error::Error>> {
    let mut config = test_config();
    config.max_body_bytes = 4;
    let db = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;
    let server = TestServer::new(build_router(AppState::new(db, config)))?;

    let response = server.get("/healthz").bytes("12345".into()).await;

    assert_eq!(response.status_code(), StatusCode::PAYLOAD_TOO_LARGE);
    Ok(())
}

#[tokio::test]
async fn missing_static_assets_render_the_chinese_not_found_page()
-> Result<(), Box<dyn std::error::Error>> {
    let server = TestServer::new(build_router(test_state().await?))?;

    let response = server.get("/static/missing.css").await;

    assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
    assert!(response.text().contains("您访问的页面不存在或已失效"));
    Ok(())
}

#[test]
fn config_reports_the_missing_required_environment_variable() {
    let previous_value = std::env::var_os("APP_ENV");
    unsafe {
        std::env::remove_var("APP_ENV");
    }

    let result = Config::from_env();

    unsafe {
        match previous_value {
            Some(value) => std::env::set_var("APP_ENV", value),
            None => std::env::remove_var("APP_ENV"),
        }
    }

    let error = match result {
        Ok(_) => panic!("missing APP_ENV must be rejected"),
        Err(error) => error.to_string(),
    };
    assert!(error.contains("APP_ENV"));
}

#[tokio::test]
async fn internal_errors_render_a_safe_chinese_page() -> Result<(), Box<dyn std::error::Error>> {
    let response = AppError::internal().into_response();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await?;
    let body = String::from_utf8(body.to_vec())?;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(body.contains("系统暂时无法处理请求"));
    assert!(!body.contains("internal error"));
    Ok(())
}
