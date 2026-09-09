mod academic_years;
mod attachments;
mod declarations;
mod settings;
mod submissions;

use sqlx::SqlitePool;

pub use academic_years::{AcademicYearRepo, NewAcademicYear};
pub use attachments::{AttachmentRepo, NewAttachment};
pub use declarations::DeclarationRepo;
pub use settings::SettingsRepo;
pub use submissions::{NewSubmission, SubmissionFilter, SubmissionRepo};

pub async fn readiness(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(pool)
        .await?;
    Ok(())
}

pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

pub async fn seed_default_academic_year(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM academic_years")
        .fetch_one(pool)
        .await?;
    if count == 0 {
        AcademicYearRepo::insert(
            pool,
            &NewAcademicYear {
                name: "2025-2026学年".to_owned(),
                start_date: chrono::NaiveDate::from_ymd_opt(2025, 8, 31)
                    .expect("static default date is valid"),
                end_date: chrono::NaiveDate::from_ymd_opt(2026, 8, 28)
                    .expect("static default date is valid"),
                deadline: None,
                is_active: true,
                announcement: None,
            },
        )
        .await?;
    }
    Ok(())
}
