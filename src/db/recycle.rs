use crate::error::AppError;
use chrono::Utc;
use sqlx::{FromRow, SqliteConnection, SqlitePool};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecordKey {
    Submission(i64),
    Declaration(i64),
}
impl RecordKey {
    pub fn parse(value: &str) -> Result<Self, AppError> {
        let (kind, id) = value.split_once(':').ok_or(AppError::BadRequest)?;
        let id: i64 = id.parse().map_err(|_| AppError::BadRequest)?;
        if id <= 0 {
            return Err(AppError::BadRequest);
        }
        match kind {
            "s" => Ok(Self::Submission(id)),
            "d" => Ok(Self::Declaration(id)),
            _ => Err(AppError::BadRequest),
        }
    }
    pub fn value(self) -> String {
        match self {
            Self::Submission(id) => format!("s:{id}"),
            Self::Declaration(id) => format!("d:{id}"),
        }
    }
    fn table_id(self) -> (&'static str, i64) {
        match self {
            Self::Submission(id) => ("submissions", id),
            Self::Declaration(id) => ("student_declarations", id),
        }
    }
}

#[derive(FromRow)]
pub struct RecycleRecord {
    pub key: String,
    pub year_name: String,
    pub student_name: String,
    pub student_no: String,
    pub label: String,
    pub deleted_at: Option<String>,
}

pub struct RecycleRepo;
impl RecycleRepo {
    pub async fn list(
        pool: &SqlitePool,
        deleted: bool,
        year_id: Option<i64>,
    ) -> Result<Vec<RecycleRecord>, sqlx::Error> {
        sqlx::query_as("SELECT 's:' || s.id AS key, y.name AS year_name, student_name, student_no, result_name AS label, deleted_at FROM submissions s JOIN academic_years y ON y.id = s.academic_year_id WHERE (deleted_at IS NOT NULL) = ? AND (? IS NULL OR y.id = ?) UNION ALL SELECT 'd:' || d.id, y.name, student_name, student_no, '无材料声明', deleted_at FROM student_declarations d JOIN academic_years y ON y.id = d.academic_year_id WHERE (deleted_at IS NOT NULL) = ? AND (? IS NULL OR y.id = ?) ORDER BY deleted_at DESC, key")
            .bind(deleted).bind(year_id).bind(year_id).bind(deleted).bind(year_id).bind(year_id).fetch_all(pool).await
    }

    pub async fn change(
        pool: &SqlitePool,
        keys: &[RecordKey],
        restore: bool,
    ) -> Result<(), AppError> {
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        for key in keys {
            let (table, id) = key.table_id();
            let sql = if restore {
                format!(
                    "UPDATE {table} SET deleted_at = NULL WHERE id = ? AND deleted_at IS NOT NULL"
                )
            } else {
                format!("UPDATE {table} SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL")
            };
            let mut query = sqlx::query(&sql);
            if !restore {
                query = query.bind(Utc::now());
            }
            let result = query.bind(id).execute(&mut *tx).await.map_err(|error| {
                if matches!(&error, sqlx::Error::Database(db) if db.is_unique_violation()) {
                    AppError::Conflict
                } else {
                    AppError::Database(error)
                }
            })?;
            if result.rows_affected() != 1 {
                return Err(AppError::Conflict);
            }
        }
        tx.commit().await?;
        Ok(())
    }
}

#[derive(FromRow)]
pub struct YearDeletionSummary {
    pub id: i64,
    pub name: String,
    pub is_active: bool,
    pub students: i64,
    pub submissions: i64,
    pub declarations: i64,
    pub attachments: i64,
}

pub struct YearDeletionRepo;
impl YearDeletionRepo {
    pub async fn summary(pool: &SqlitePool, id: i64) -> Result<YearDeletionSummary, AppError> {
        sqlx::query_as("SELECT id, name, is_active, (SELECT COUNT(*) FROM academic_year_students WHERE academic_year_id = y.id) AS students, (SELECT COUNT(*) FROM submissions WHERE academic_year_id = y.id) AS submissions, (SELECT COUNT(*) FROM student_declarations WHERE academic_year_id = y.id) AS declarations, (SELECT COUNT(*) FROM attachments WHERE submission_id IN (SELECT id FROM submissions WHERE academic_year_id = y.id)) AS attachments FROM academic_years y WHERE id = ?")
            .bind(id).fetch_optional(pool).await?.ok_or(AppError::NotFound)
    }

    pub async fn delete(pool: &SqlitePool, id: i64, expected_name: &str) -> Result<(), AppError> {
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let name: Option<String> =
            sqlx::query_scalar("SELECT name FROM academic_years WHERE id = ?")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if name.as_deref() != Some(expected_name) {
            return Err(AppError::Conflict);
        }
        sqlx::query("INSERT OR IGNORE INTO pending_attachment_deletions (stored_name, created_at) SELECT stored_name, ? FROM attachments WHERE submission_id IN (SELECT id FROM submissions WHERE academic_year_id = ?)")
            .bind(Utc::now()).bind(id).execute(&mut *tx).await?;
        delete_records(&mut tx, id).await?;
        sqlx::query("DELETE FROM academic_years WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn pending_files(pool: &SqlitePool) -> Result<Vec<String>, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT stored_name FROM pending_attachment_deletions ORDER BY created_at LIMIT 5000",
        )
        .fetch_all(pool)
        .await
    }
    pub async fn finish_file(pool: &SqlitePool, name: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM pending_attachment_deletions WHERE stored_name = ?")
            .bind(name)
            .execute(pool)
            .await?;
        Ok(())
    }
}

async fn delete_records(connection: &mut SqliteConnection, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM submissions WHERE academic_year_id = ?")
        .bind(id)
        .execute(&mut *connection)
        .await?;
    sqlx::query("DELETE FROM student_declarations WHERE academic_year_id = ?")
        .bind(id)
        .execute(&mut *connection)
        .await?;
    Ok(())
}
