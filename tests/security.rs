use std::{
    io::Write,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{body::to_bytes, response::IntoResponse};
use axum_test::TestServer;
use serde_json::json;
use zongce_web::{
    auth::{generate_edit_code, hash_secret},
    config::Config,
    error::AppError,
    state::AppState,
    storage::StorageError,
};

async fn fixture(limit: usize, secure: bool) -> (TestServer, AppState) {
    let state = AppState::initialize("sqlite::memory:").await.unwrap();
    let config = Config {
        app_env: if secure { "production" } else { "development" }.into(),
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        admin_username: "test-admin".into(),
        admin_password_hash: hash_secret(&generate_edit_code()).unwrap(),
        session_secret: uuid::Uuid::new_v4().as_bytes().repeat(4),
        database_url: "sqlite::memory:".into(),
        upload_dir: "unused-test-upload".into(),
        cookie_secure: secure,
        max_body_bytes: limit,
    };
    let mut server = TestServer::new(
        zongce_web::routes::build_router_with_config(state.clone(), &config).unwrap(),
    )
    .unwrap();
    server.save_cookies();
    (server, state)
}

fn csrf(html: &str) -> String {
    html.split("name=\"csrf_token\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .into()
}

#[tokio::test]
async fn health_checks_database_and_missing_routes_have_safe_html() {
    let (server, state) = fixture(4096, false).await;
    let health = server.get("/healthz").await;
    assert_eq!(health.status_code(), 200);
    assert_eq!(health.text(), "ok");
    let missing = server.get("/missing-private-path").await;
    assert_eq!(missing.status_code(), 404);
    assert!(
        missing.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/html")
    );
    assert!(missing.text().contains("页面不存在"));
    assert!(!missing.text().contains("missing-private-path"));
    state.db.close().await;
    let unavailable = server.get("/healthz").await;
    assert_eq!(unavailable.status_code(), 500);
    assert!(!unavailable.text().contains("PoolClosed"));
}

#[tokio::test]
async fn internal_errors_never_render_sources_or_validation_keys() {
    for error in [
        AppError::Storage(StorageError::Io(std::io::Error::other(
            "/private/secret/path sensitive-marker",
        ))),
        AppError::Database(sqlx::Error::Protocol(
            "SELECT secret FROM private_table".into(),
        )),
        AppError::Auth(zongce_web::auth::AuthError::Verification),
    ] {
        let response = error.into_response();
        assert_eq!(response.status(), 500);
        assert!(
            response.headers()["content-type"]
                .to_str()
                .unwrap()
                .starts_with("text/html")
        );
        let body = String::from_utf8(
            to_bytes(response.into_body(), 16384)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(body.contains("服务暂时不可用"));
        for forbidden in [
            "/private",
            "sensitive-marker",
            "SELECT",
            "private_table",
            "sqlx",
            "Verification",
        ] {
            assert!(!body.contains(forbidden));
        }
    }
}

#[tokio::test]
async fn configured_cookie_and_csrf_protect_mutations() {
    let (server, state) = fixture(4096, true).await;
    let login = server.get("/admin/login").await;
    let cookie = login.headers()["set-cookie"].to_str().unwrap();
    for attribute in ["HttpOnly", "SameSite=Lax", "Secure"] {
        assert!(cookie.contains(attribute));
    }
    let response = server
        .post("/admin/login")
        .form(&json!({"username":"test-admin","password":generate_edit_code()}))
        .await;
    assert_eq!(response.status_code(), 400);
    assert_eq!(
        server
            .post("/admin/years")
            .form(&json!({"name":"unauthorized"}))
            .await
            .status_code(),
        403
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM academic_years")
            .fetch_one(&state.db)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn body_limit_rejects_form_and_multipart_with_chinese_413() {
    let (server, state) = fixture(1024, false).await;
    let declared = server
        .post("/submit")
        .add_header("content-length", "2048")
        .content_type("multipart/form-data; boundary=test-boundary")
        .bytes(vec![b'x'; 2048].into())
        .await;
    assert_eq!(declared.status_code(), 413);
    assert!(declared.text().contains("请求内容过大"));
    for path in ["/access", "/admin/login", "/query"] {
        let response = server
            .post(path)
            .content_type("application/x-www-form-urlencoded")
            .bytes(vec![b'a'; 2048].into())
            .await;
        assert_eq!(response.status_code(), 413, "{path}");
        assert!(response.text().contains("请求内容过大"));
        assert!(!response.text().contains("Failed to"));
    }
    let code = generate_edit_code();
    zongce_web::db::SettingsRepo::set(
        &state.db,
        "class_access_code_hash",
        &hash_secret(&code).unwrap(),
    )
    .await
    .unwrap();
    let token = csrf(&server.get("/").await.text());
    assert_eq!(
        server
            .post("/access")
            .form(&json!({"csrf_token":token,"access_code":code}))
            .await
            .status_code(),
        303
    );
    let multipart = format!(
        "--test-boundary\r\nContent-Disposition: form-data; name=\"csrf_token\"\r\n\r\n{token}\r\n--test-boundary\r\nContent-Disposition: form-data; name=\"files\"; filename=\"proof.pdf\"\r\nContent-Type: application/pdf\r\n\r\n%PDF-{}\r\n--test-boundary--\r\n",
        "x".repeat(2048)
    );
    let response = server
        .post("/submit")
        .content_type("multipart/form-data; boundary=test-boundary")
        .bytes(multipart.into())
        .await;
    assert_eq!(response.status_code(), 413);
    assert!(response.text().contains("请求内容过大"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM submissions")
            .fetch_one(&state.db)
            .await
            .unwrap(),
        0
    );
}

#[derive(Clone)]
struct Capture(Arc<Mutex<Vec<u8>>>);
impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test(flavor = "current_thread")]
async fn request_events_use_generated_ids_and_route_templates_without_sensitive_input() {
    let captured = Capture(Arc::new(Mutex::new(Vec::new())));
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_max_level(tracing::Level::INFO)
        .with_writer(move || writer.clone())
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    let (server, _) = fixture(4096, false).await;
    let marker = uuid::Uuid::new_v4().simple().to_string();
    let token = csrf(&server.get("/admin/login").await.text());
    let started = Instant::now();
    let response = server
        .post("/admin/login")
        .add_header("x-request-id", marker.clone())
        .form(&json!({"csrf_token":token,"username":"unknown","password":marker}))
        .await;
    assert_eq!(response.status_code(), 401);
    assert!(started.elapsed() >= Duration::from_millis(100));
    let request_id = response
        .headers()
        .get("x-request-id")
        .expect("generated request id")
        .to_str()
        .unwrap();
    assert_ne!(request_id, marker);
    assert!(uuid::Uuid::parse_str(request_id).is_ok());
    server
        .get(&format!("/query/{marker}?edit_code={marker}"))
        .await;
    let logs = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
    assert!(logs.contains("admin_login_failed"), "{logs}");
    assert!(logs.contains("/query/{submission_no}"), "{logs}");
    assert!(logs.contains("request_id"));
    assert!(!logs.contains(&marker));
    assert!(!logs.contains(&token));
}
