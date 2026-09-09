//! Local browser-test harness. All data and uploads are temporary.
use zongce_web::{
    auth::hash_secret, config::Config, db::SettingsRepo, state::AppState,
    storage::AttachmentStorage,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let password = std::env::var("ZONGCE_PREVIEW_PASSWORD")?;
    let uploads = tempfile::tempdir()?;
    let mut state = AppState::initialize("sqlite::memory:").await?;
    state.storage = AttachmentStorage::new(uploads.path());
    SettingsRepo::set(
        &state.db,
        "class_access_code_hash",
        &hash_secret("preview-only")?,
    )
    .await?;
    let config = Config {
        app_env: "development".into(),
        bind_addr: "127.0.0.1:0".parse()?,
        admin_username: "preview-admin".into(),
        admin_password_hash: hash_secret(&password)?,
        session_secret: uuid::Uuid::new_v4().as_bytes().repeat(4),
        database_url: "sqlite::memory:".into(),
        upload_dir: uploads.path().into(),
        cookie_secure: false,
        max_body_bytes: 115_343_360,
    };
    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    println!("Admin preview: http://{}", listener.local_addr()?);
    axum::serve(
        listener,
        zongce_web::routes::build_router_with_config(state, &config)?,
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
    Ok(())
}
