use std::{str::FromStr, time::Duration};

use sqlx::{
    SqlitePool,
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
            .busy_timeout(Duration::from_secs(5));
        let db = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        migrate(&db).await.map_err(sqlx::Error::protocol)?;
        seed_default_academic_year(&db).await?;
        Ok(Self {
            db,
            storage: AttachmentStorage::new("uploads"),
        })
    }
}
