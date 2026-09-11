mod fixtures;
use zongce_web::{
    auth::hash_secret, db::SettingsRepo, state::AppState, storage::AttachmentStorage,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let uploads = tempfile::tempdir()?;
    let mut state = AppState::initialize("sqlite::memory:").await?;
    fixtures::seed(&state.db).await?;
    state.storage = AttachmentStorage::new(uploads.path());
    SettingsRepo::set(
        &state.db,
        "class_access_code_hash",
        &hash_secret("preview-only")?,
    )
    .await?;
    SettingsRepo::set(&state.db, "class_name", "本地测试班级").await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    println!("Student preview: http://{}", listener.local_addr()?);
    println!("Test-only class code: preview-only. Data is temporary.");
    axum::serve(listener, zongce_web::routes::build_router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
