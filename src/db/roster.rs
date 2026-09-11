use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteConnection, SqlitePool};

#[derive(Clone, Deserialize, Serialize, FromRow, PartialEq, Eq)]
pub struct RosterStudent {
    pub student_name: String,
    pub student_no: String,
}

pub struct RosterRepo;

impl RosterRepo {
    pub async fn list(pool: &SqlitePool, year_id: i64) -> Result<Vec<RosterStudent>, sqlx::Error> {
        sqlx::query_as("SELECT student_name, student_no FROM academic_year_students WHERE academic_year_id = ? ORDER BY student_no")
            .bind(year_id).fetch_all(pool).await
    }

    pub async fn counts(
        pool: &SqlitePool,
    ) -> Result<std::collections::BTreeMap<i64, i64>, sqlx::Error> {
        let rows: Vec<(i64, i64)> = sqlx::query_as("SELECT academic_year_id, COUNT(*) FROM academic_year_students GROUP BY academic_year_id").fetch_all(pool).await?;
        Ok(rows.into_iter().collect())
    }

    pub async fn matches(
        connection: &mut SqliteConnection,
        year_id: i64,
        name: &str,
        number: &str,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM academic_year_students WHERE academic_year_id = ? AND student_name = ? AND student_no = ?)")
            .bind(year_id).bind(name.trim()).bind(number.trim()).fetch_one(connection).await
    }

    /// Validate every conflict before writing; the caller owns the write lock.
    pub async fn append(
        connection: &mut SqliteConnection,
        year_id: i64,
        students: &[RosterStudent],
    ) -> Result<usize, crate::error::AppError> {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM academic_years WHERE id = ?)")
                .bind(year_id)
                .fetch_one(&mut *connection)
                .await?;
        if !exists {
            return Err(crate::error::AppError::NotFound);
        }
        let mut added = 0;
        for student in students {
            let existing: Option<String> = sqlx::query_scalar("SELECT student_name FROM academic_year_students WHERE academic_year_id = ? AND student_no = ?")
                .bind(year_id).bind(&student.student_no).fetch_optional(&mut *connection).await?;
            match existing {
                Some(name) if name != student.student_name => {
                    let mut errors = crate::validation::ValidationErrors::new();
                    errors.add(
                        "roster",
                        "导入学号与现有名单中的姓名不一致，本次名单未保存，请核对后重试",
                    );
                    return Err(crate::error::AppError::Validation(errors));
                }
                Some(_) => {}
                None => {
                    sqlx::query("INSERT INTO academic_year_students (academic_year_id, student_no, student_name, created_at) VALUES (?, ?, ?, ?)")
                        .bind(year_id).bind(&student.student_no).bind(&student.student_name).bind(Utc::now()).execute(&mut *connection).await?;
                    added += 1;
                }
            }
        }
        Ok(added)
    }
}
