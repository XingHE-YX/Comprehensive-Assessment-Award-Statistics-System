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
    if AcademicYearRepo::find_by_id(pool, id).await?.is_none() {
        return Err(SettingsError::Internal(AppError::NotFound));
    }
    if crate::db::RosterRepo::list(pool, id).await?.is_empty() {
        let mut errors = ValidationErrors::new();
        errors.add("roster", "请先为该学年导入学生名单，再激活申报");
        return Err(SettingsError::Invalid(errors));
    }
    AcademicYearRepo::activate(pool, id).await?;
    tracing::info!(academic_year_id = id, "academic year activated");
    Ok(())
}

pub async fn create_year(
    pool: &SqlitePool,
    input: &AcademicYearInput,
    students: &[crate::db::RosterStudent],
) -> Result<(), SettingsError> {
    let year = validate_year(input).map_err(SettingsError::Invalid)?;
    if students.is_empty() {
        let mut errors = ValidationErrors::new();
        errors.add("roster", "请先导入学生名单，再创建学年");
        return Err(SettingsError::Invalid(errors));
    }
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let id = AcademicYearRepo::insert_in_transaction(&mut tx, &year).await?;
    crate::db::RosterRepo::append(&mut tx, id, students)
        .await
        .map_err(SettingsError::Internal)?;
    tx.commit().await?;
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
