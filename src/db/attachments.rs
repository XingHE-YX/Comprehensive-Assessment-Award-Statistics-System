use chrono::Utc;
use sqlx::{Row, SqliteConnection, SqlitePool};

use crate::domain::Attachment;

#[derive(Debug, Clone)]
pub struct NewAttachment {
    pub submission_id: i64,
    pub original_name: String,
    pub stored_name: String,
    pub mime_type: String,
    pub file_size: i64,
}

pub struct AttachmentRepo;

impl AttachmentRepo {
    /// Fetch selected metadata in one query; exporting never opens stored files.
    pub async fn list_for_filter<'e>(
        executor: impl sqlx::Executor<'e, Database = sqlx::Sqlite>,
        filter: &super::SubmissionFilter,
    ) -> Result<Vec<Attachment>, sqlx::Error> {
        let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "SELECT * FROM attachments WHERE submission_id IN (SELECT id FROM submissions WHERE 1 = 1",
        );
        filter.push_predicates(&mut query);
        query.push(") ORDER BY submission_id, created_at, id");
        query
            .build()
            .fetch_all(executor)
            .await?
            .into_iter()
            .map(row_to_attachment)
            .collect()
    }

    pub async fn count_in_transaction(
        connection: &mut SqliteConnection,
        submission_id: i64,
    ) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar("SELECT COUNT(*) FROM attachments WHERE submission_id = ?")
            .bind(submission_id)
            .fetch_one(connection)
            .await
    }

    pub async fn insert_stored(
        connection: &mut SqliteConnection,
        submission_id: i64,
        item: &crate::storage::StoredAttachment,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO attachments (submission_id, original_name, stored_name, mime_type, file_size, created_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(submission_id).bind(&item.original_name).bind(&item.stored_name)
            .bind(&item.mime_type).bind(item.file_size).bind(Utc::now()).execute(connection).await?;
        Ok(())
    }

    pub async fn insert(
        pool: &SqlitePool,
        input: &NewAttachment,
    ) -> Result<Attachment, sqlx::Error> {
        let result = sqlx::query(
            "INSERT INTO attachments
             (submission_id, original_name, stored_name, mime_type, file_size, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(input.submission_id)
        .bind(&input.original_name)
        .bind(&input.stored_name)
        .bind(&input.mime_type)
        .bind(input.file_size)
        .bind(Utc::now())
        .execute(pool)
        .await?;
        Self::find_by_id(pool, result.last_insert_rowid())
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn find_by_id(pool: &SqlitePool, id: i64) -> Result<Option<Attachment>, sqlx::Error> {
        sqlx::query("SELECT * FROM attachments WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?
            .map(row_to_attachment)
            .transpose()
    }

    pub async fn list_for_submission(
        pool: &SqlitePool,
        submission_id: i64,
    ) -> Result<Vec<Attachment>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT * FROM attachments WHERE submission_id = ? ORDER BY created_at ASC, id ASC",
        )
        .bind(submission_id)
        .fetch_all(pool)
        .await?;
        rows.into_iter().map(row_to_attachment).collect()
    }
}

fn row_to_attachment(row: sqlx::sqlite::SqliteRow) -> Result<Attachment, sqlx::Error> {
    Ok(Attachment {
        id: row.try_get("id")?,
        submission_id: row.try_get("submission_id")?,
        original_name: row.try_get("original_name")?,
        stored_name: row.try_get("stored_name")?,
        mime_type: row.try_get("mime_type")?,
        file_size: row.try_get("file_size")?,
        created_at: row.try_get("created_at")?,
    })
}
