use sqlx::SqlitePool;

use crate::{
    auth,
    db::{AcademicYearRepo, SettingsRepo},
    error::AppError,
    validation::{
        ValidationErrors,
        settings::{AcademicYearInput, validate_class_code, validate_year},
    },
};

pub enum SettingsError {
    Invalid(ValidationErrors),
    Conflict,
    Internal(AppError),
}

impl From<sqlx::Error> for SettingsError {
    fn from(error: sqlx::Error) -> Self {
        if matches!(error, sqlx::Error::RowNotFound) {
            return Self::Internal(AppError::NotFound);
        }
        if let sqlx::Error::Database(ref db) = error {
            if db.is_unique_violation() {
                let mut errors = ValidationErrors::new();
                errors.add("name", "学年名称已存在，请使用其他名称");
                return Self::Invalid(errors);
            }
            // SQLite's extended error codes keep SQLITE_BUSY/LOCKED in the low byte.
            if db
                .code()
                .and_then(|code| code.parse::<i32>().ok())
                .is_some_and(|code| matches!(code & 0xff, 5 | 6))
            {
                return Self::Conflict;
            }
        }
        tracing::error!("settings database operation failed");
        Self::Internal(AppError::Database(error))
    }
}

pub async fn save_year(
    pool: &SqlitePool,
    id: Option<i64>,
    input: &AcademicYearInput,
) -> Result<(), SettingsError> {
    let year = validate_year(input).map_err(SettingsError::Invalid)?;
    if let Some(id) = id {
        AcademicYearRepo::update(pool, id, &year).await?;
    } else {
        AcademicYearRepo::insert(pool, &year).await?;
    }
    tracing::info!(academic_year_id = id, "academic year settings saved");
    Ok(())
}

pub async fn activate_year(pool: &SqlitePool, id: i64) -> Result<(), SettingsError> {
    AcademicYearRepo::activate(pool, id).await?;
    tracing::info!(academic_year_id = id, "academic year activated");
    Ok(())
}

pub async fn replace_class_code(pool: &SqlitePool, code: String) -> Result<(), SettingsError> {
    validate_class_code(&code).map_err(SettingsError::Invalid)?;
    let hash = tokio::task::spawn_blocking(move || auth::hash_secret(&code))
        .await
        .map_err(|_| SettingsError::Internal(AppError::Auth(auth::AuthError::Verification)))?
        .map_err(|_| SettingsError::Internal(AppError::Auth(auth::AuthError::Verification)))?;
    SettingsRepo::set(pool, "class_access_code_hash", &hash).await?;
    tracing::info!("class access code replaced");
    Ok(())
}
