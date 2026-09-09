use std::str::FromStr;

use chrono::{Datelike, NaiveDate, Utc};
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
    /// Call under the submission transaction's write lock. Academic years can
    /// share an ending year, so sequence allocation must use the public prefix.
    pub async fn next_number(
        connection: &mut SqliteConnection,
        year: &crate::domain::AcademicYear,
    ) -> Result<String, sqlx::Error> {
        let sequence: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(CAST(substr(submission_no, 8) AS INTEGER)), 0) + 1 FROM submissions WHERE substr(submission_no, 3, 4) = ?",
        )
        .bind(format!("{:04}", year.end_date.year()))
        .fetch_one(connection).await?;
        if sequence > 999_999 {
            return Err(sqlx::Error::protocol("submission sequence exhausted"));
        }
        Ok(crate::auth::generate_submission_no(year, sequence as u64))
    }

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

    pub async fn list<'e>(
        executor: impl sqlx::Executor<'e, Database = Sqlite>,
        filter: &SubmissionFilter,
    ) -> Result<Vec<Submission>, sqlx::Error> {
        let mut query = QueryBuilder::<Sqlite>::new("SELECT * FROM submissions WHERE 1 = 1");
        filter.push_predicates(&mut query);
        query.push(" ORDER BY created_at DESC, id DESC");
        let rows = query.build().fetch_all(executor).await?;
        rows.into_iter().map(row_to_submission).collect()
    }

    pub async fn update_review(
        pool: &SqlitePool,
        id: i64,
        status: SubmissionStatus,
        review_note: Option<&str>,
        approved_score: Option<f64>,
    ) -> Result<(), sqlx::Error> {
        let mut transaction = pool.begin().await?;
        let result = sqlx::query(
            "UPDATE submissions SET status = ?, review_note = ?, approved_score = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(status.as_str())
        .bind(review_note)
        .bind(approved_score)
        .bind(Utc::now())
        .bind(id)
        .execute(&mut *transaction)
        .await?;
        if result.rows_affected() != 1 {
            return Err(sqlx::Error::RowNotFound);
        }
        transaction.commit().await?;
        Ok(())
    }
}

impl SubmissionFilter {
    /// Shared by the dashboard, export records and export attachment subquery.
    pub(super) fn push_predicates(&self, query: &mut QueryBuilder<'_, Sqlite>) {
        self.push_identity_predicates(query);
        if let Some(category) = self.category {
            query.push(" AND category = ").push_bind(category.as_str());
        }
        if let Some(status) = self.status {
            query.push(" AND status = ").push_bind(status.as_str());
        }
    }

    /// No-material declarations have no category or review status.
    pub(super) fn push_identity_predicates(&self, query: &mut QueryBuilder<'_, Sqlite>) {
        if let Some(id) = self.academic_year_id {
            query.push(" AND academic_year_id = ").push_bind(id);
        }
        for (column, value) in [
            ("student_name", &self.name),
            ("student_no", &self.student_no),
        ] {
            if let Some(value) = value {
                query
                    .push(" AND ")
                    .push(column)
                    .push(" LIKE ")
                    .push_bind(literal_keyword(value))
                    .push(" ESCAPE '\\'");
            }
        }
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

// A keyword is literal text, including SQL LIKE metacharacters.
pub(super) fn literal_keyword(value: &str) -> String {
    format!(
        "%{}%",
        value
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    )
}
