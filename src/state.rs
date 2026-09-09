use std::{str::FromStr, time::Duration};

use sqlx::{
    ConnectOptions, SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};

use crate::db::{migrate, seed_default_academic_year};
use crate::storage::AttachmentStorage;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub storage: AttachmentStorage,
}

impl AppState {
    pub async fn initialize(database_url: &str) -> Result<Self, sqlx::Error> {
        let options = SqliteConnectOptions::from_str(database_url)?
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5))
            .disable_statement_logging();
        let db = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        migrate(&db).await.map_err(sqlx::Error::protocol)?;
        seed_default_academic_year(&db).await?;
        tracing::info!(event = "migration", status = "complete");
        let sqlite_version = crate::db::engine_version(&db).await?;
        // Query the connected engine, not a compiled driver or external CLI version.
        tracing::info!(event = "database_ready", sqlite_version = %sqlite_version);
        Ok(Self {
            db,
            storage: AttachmentStorage::new("uploads"),
        })
    }
}
