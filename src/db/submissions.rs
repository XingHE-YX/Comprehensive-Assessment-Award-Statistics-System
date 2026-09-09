use std::str::FromStr;

use chrono::{NaiveDate, Utc};
use serde_json::Value;
use sqlx::{QueryBuilder, Row, Sqlite, SqliteConnection, SqlitePool};

use crate::domain::{Category, Submission, SubmissionStatus};

#[derive(Debug, Clone)]
pub struct NewSubmission {
    pub submission_no: String,
    pub academic_year_id: i64,
    pub student_name: String,
    pub student_no: String,
    pub category: Category,
    pub result_name: String,
    pub obtained_date: NaiveDate,
    pub detail: Option<String>,
    pub remark: Option<String>,
    pub category_data: Value,
    pub edit_code_hash: String,
}

#[derive(Debug, Clone, Default)]
pub struct SubmissionFilter {
    pub academic_year_id: Option<i64>,
    pub name: Option<String>,
    pub student_no: Option<String>,
    pub category: Option<Category>,
    pub status: Option<SubmissionStatus>,
}

pub struct SubmissionRepo;

impl SubmissionRepo {
    pub async fn update_student(
        connection: &mut SqliteConnection,
        id: i64,
        input: &crate::validation::ValidatedSubmission,
    ) -> Result<bool, sqlx::Error> {
        let category_data = serde_json::to_string(&input.category_data)
            .map_err(|error| sqlx::Error::Encode(Box::new(error)))?;
        let result = sqlx::query(
            "UPDATE submissions SET student_name = ?, student_no = ?, category = ?, result_name = ?,
             obtained_date = ?, detail = ?, remark = ?, category_data = ?, status = 'pending',
             student_modified_after_review = 1, updated_at = ?
             WHERE id = ? AND status IN ('pending', 'needs_revision')",
        )
        .bind(&input.student_name).bind(&input.student_no).bind(input.category.as_str())
        .bind(&input.result_name).bind(input.obtained_date).bind(&input.detail).bind(&input.remark)
        .bind(category_data).bind(Utc::now()).bind(id).execute(connection).await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn insert(
        pool: &SqlitePool,
        input: &NewSubmission,
    ) -> Result<Submission, sqlx::Error> {
        let now = Utc::now();
        let category_data = serde_json::to_string(&input.category_data)
            .map_err(|error| sqlx::Error::Decode(Box::new(error)))?;
        let result = sqlx::query(
            "INSERT INTO submissions
             (submission_no, academic_year_id, student_name, student_no, category, result_name,
              obtained_date, detail, remark, category_data, edit_code_hash, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&input.submission_no)
        .bind(input.academic_year_id)
        .bind(&input.student_name)
        .bind(&input.student_no)
        .bind(input.category.as_str())
        .bind(&input.result_name)
        .bind(input.obtained_date)
        .bind(&input.detail)
        .bind(&input.remark)
        .bind(category_data)
        .bind(&input.edit_code_hash)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;
        Self::find_by_id(pool, result.last_insert_rowid())
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn find_by_id(pool: &SqlitePool, id: i64) -> Result<Option<Submission>, sqlx::Error> {
        sqlx::query("SELECT * FROM submissions WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?
            .map(row_to_submission)
            .transpose()
    }

    pub async fn find_by_no(
        pool: &SqlitePool,
        submission_no: &str,
    ) -> Result<Option<Submission>, sqlx::Error> {
        sqlx::query("SELECT * FROM submissions WHERE submission_no = ?")
            .bind(submission_no)
            .fetch_optional(pool)
            .await?
            .map(row_to_submission)
            .transpose()
    }

    pub async fn list(
        pool: &SqlitePool,
        filter: &SubmissionFilter,
    ) -> Result<Vec<Submission>, sqlx::Error> {
        let mut query = QueryBuilder::<Sqlite>::new("SELECT * FROM submissions WHERE 1 = 1");
        if let Some(year_id) = filter.academic_year_id {
            query.push(" AND academic_year_id = ").push_bind(year_id);
        }
        if let Some(name) = &filter.name {
            query
                .push(" AND student_name LIKE ")
                .push_bind(format!("%{name}%"));
        }
        if let Some(student_no) = &filter.student_no {
            query
                .push(" AND student_no LIKE ")
                .push_bind(format!("%{student_no}%"));
        }
        if let Some(category) = filter.category {
            query.push(" AND category = ").push_bind(category.as_str());
        }
        if let Some(status) = filter.status {
            query.push(" AND status = ").push_bind(status.as_str());
        }
        query.push(" ORDER BY created_at DESC, id DESC");
        let rows = query.build().fetch_all(pool).await?;
        rows.into_iter().map(row_to_submission).collect()
    }

    pub async fn update_review(
        pool: &SqlitePool,
        id: i64,
        status: SubmissionStatus,
        review_note: Option<&str>,
        approved_score: Option<f64>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE submissions SET status = ?, review_note = ?, approved_score = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(status.as_str())
        .bind(review_note)
        .bind(approved_score)
        .bind(Utc::now())
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }
}

fn row_to_submission(row: sqlx::sqlite::SqliteRow) -> Result<Submission, sqlx::Error> {
    let category = Category::from_str(row.try_get::<String, _>("category")?.as_str())
        .map_err(|error| sqlx::Error::Decode(Box::new(std::io::Error::other(error))))?;
    let status = SubmissionStatus::from_str(row.try_get::<String, _>("status")?.as_str())
        .map_err(|error| sqlx::Error::Decode(Box::new(std::io::Error::other(error))))?;
    let category_data = serde_json::from_str::<Value>(&row.try_get::<String, _>("category_data")?)
        .map_err(|error| sqlx::Error::Decode(Box::new(error)))?;
    Ok(Submission {
        id: row.try_get("id")?,
        submission_no: row.try_get("submission_no")?,
        academic_year_id: row.try_get("academic_year_id")?,
        student_name: row.try_get("student_name")?,
        student_no: row.try_get("student_no")?,
        category,
        result_name: row.try_get("result_name")?,
        obtained_date: row.try_get("obtained_date")?,
        detail: row.try_get("detail")?,
        remark: row.try_get("remark")?,
        category_data,
        status,
        review_note: row.try_get("review_note")?,
        approved_score: row.try_get("approved_score")?,
        edit_code_hash: row.try_get("edit_code_hash")?,
        student_modified_after_review: row.try_get::<i64, _>("student_modified_after_review")? != 0,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
