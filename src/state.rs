use sqlx::SqlitePool;

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub config: Config,
}

impl AppState {
    #[must_use]
    pub fn new(db: SqlitePool, config: Config) -> Self {
        Self { db, config }
    }
}
