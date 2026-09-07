#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:data/app.db".to_owned());
    if database_url == "sqlite:data/app.db" {
        std::fs::create_dir_all("data")?;
    }
    let state = zongce_web::state::AppState::initialize(&database_url).await?;
    println!(
        "database initialized with {} active year(s)",
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM academic_years WHERE is_active = 1")
            .fetch_one(&state.db)
            .await?
    );
    Ok(())
}
