//! Read a consistent database snapshot, then generate XLSX off the async workers.
use std::collections::BTreeMap;

use crate::{
    db::{AcademicYearRepo, AttachmentRepo, DeclarationRepo, SubmissionRepo},
    error::AppError,
    export::{ExportData, export_xlsx},
    validation::admin::dashboard_filters,
};

pub struct ExportDownload {
    pub filename: String,
    pub bytes: Vec<u8>,
}

/// None means invalid filters: return to the dashboard's visible filter notice.
pub async fn prepare(
    pool: &sqlx::SqlitePool,
    values: &BTreeMap<String, String>,
) -> Result<Option<ExportDownload>, AppError> {
    let mut transaction = pool.begin().await?;
    let years = AcademicYearRepo::list(&mut *transaction).await?;
    let (filter, invalid) = dashboard_filters(values, &years);
    if invalid {
        transaction.rollback().await?;
        return Ok(None);
    }
    let year_name = years
        .iter()
        .find(|year| Some(year.id) == filter.academic_year_id)
        .map_or("全部学年", |year| year.name.as_str());
    // Display names may contain path separators. Only the UTF-8 filename parameter
    // carries this name; the response also provides a fixed ASCII fallback.
    let safe_name: String = year_name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let filename = format!("{safe_name}综测申报汇总.xlsx");
    let submissions = SubmissionRepo::list(&mut *transaction, &filter).await?;
    let declarations = DeclarationRepo::list(&mut *transaction, &filter).await?;
    let mut attachments = BTreeMap::new();
    for item in AttachmentRepo::list_for_filter(&mut *transaction, &filter).await? {
        attachments
            .entry(item.submission_id)
            .or_insert_with(Vec::new)
            .push(item);
    }
    transaction.commit().await?;
    let data = ExportData {
        years,
        submissions,
        declarations,
        attachments,
    };
    let bytes = tokio::task::spawn_blocking(move || export_xlsx(&data))
        .await
        .map_err(|_| {
            tracing::error!(route = "/admin/export.xlsx", status = 500, "导出任务失败");
            AppError::Export
        })?
        .map_err(|_| {
            // XlsxError may contain cell data or internal paths; never log it raw.
            tracing::error!(route = "/admin/export.xlsx", status = 500, "工作簿生成失败");
            AppError::Export
        })?;
    tracing::info!(route = "/admin/export.xlsx", status = 200, "工作簿生成成功");
    Ok(Some(ExportDownload { filename, bytes }))
}
