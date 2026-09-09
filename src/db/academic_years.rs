use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{Row, SqlitePool};

use crate::domain::AcademicYear;

#[derive(Debug, Clone)]
pub struct NewAcademicYear {
    pub name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub deadline: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub announcement: Option<String>,
}

pub struct AcademicYearRepo;

impl AcademicYearRepo {
    pub async fn list(pool: &SqlitePool) -> Result<Vec<AcademicYear>, sqlx::Error> {
        sqlx::query("SELECT * FROM academic_years ORDER BY start_date DESC, id DESC")
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(row_to_academic_year)
            .collect()
    }

    pub async fn insert(
        pool: &SqlitePool,
        input: &NewAcademicYear,
    ) -> Result<AcademicYear, sqlx::Error> {
        let now = Utc::now();
        let result = sqlx::query(
            "INSERT INTO academic_years
             (name, start_date, end_date, deadline, is_active, announcement, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&input.name)
        .bind(input.start_date)
        .bind(input.end_date)
        .bind(input.deadline)
        .bind(input.is_active)
        .bind(&input.announcement)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;
        Self::find_by_id(pool, result.last_insert_rowid())
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn find_by_id(
        pool: &SqlitePool,
        id: i64,
    ) -> Result<Option<AcademicYear>, sqlx::Error> {
        sqlx::query(
            "SELECT id, name, start_date, end_date, deadline, is_active, announcement, created_at, updated_at
             FROM academic_years WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?
        .map(row_to_academic_year)
        .transpose()
    }

    pub async fn current(pool: &SqlitePool) -> Result<Option<AcademicYear>, sqlx::Error> {
        sqlx::query(
            "SELECT id, name, start_date, end_date, deadline, is_active, announcement, created_at, updated_at
             FROM academic_years WHERE is_active = 1 LIMIT 1",
        )
        .fetch_optional(pool)
        .await?
        .map(row_to_academic_year)
        .transpose()
    }
}

fn row_to_academic_year(row: sqlx::sqlite::SqliteRow) -> Result<AcademicYear, sqlx::Error> {
    Ok(AcademicYear {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        start_date: row.try_get("start_date")?,
        end_date: row.try_get("end_date")?,
        deadline: row.try_get("deadline")?,
        is_active: row.try_get::<i64, _>("is_active")? != 0,
        announcement: row.try_get("announcement")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
