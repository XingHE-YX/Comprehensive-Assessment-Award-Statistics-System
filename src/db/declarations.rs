use chrono::Utc;
use sqlx::{Row, SqlitePool};

use crate::domain::StudentDeclaration;

pub struct DeclarationRepo;

impl DeclarationRepo {
    pub async fn upsert(
        pool: &SqlitePool,
        academic_year_id: i64,
        student_name: &str,
        student_no: &str,
    ) -> Result<StudentDeclaration, sqlx::Error> {
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO student_declarations
             (academic_year_id, student_name, student_no, has_submission_material, created_at, updated_at)
             VALUES (?, ?, ?, 0, ?, ?)
             ON CONFLICT (academic_year_id, student_no, student_name)
             DO UPDATE SET updated_at = excluded.updated_at",
        )
        .bind(academic_year_id)
        .bind(student_name)
        .bind(student_no)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;
        sqlx::query(
            "SELECT * FROM student_declarations
             WHERE academic_year_id = ? AND student_name = ? AND student_no = ?",
        )
        .bind(academic_year_id)
        .bind(student_name)
        .bind(student_no)
        .fetch_one(pool)
        .await
        .and_then(row_to_declaration)
    }
}

fn row_to_declaration(row: sqlx::sqlite::SqliteRow) -> Result<StudentDeclaration, sqlx::Error> {
    Ok(StudentDeclaration {
        id: row.try_get("id")?,
        academic_year_id: row.try_get("academic_year_id")?,
        student_name: row.try_get("student_name")?,
        student_no: row.try_get("student_no")?,
        has_submission_material: row.try_get::<i64, _>("has_submission_material")? != 0,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
