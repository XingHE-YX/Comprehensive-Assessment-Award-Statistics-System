use std::{collections::BTreeMap, str::FromStr};

use axum::extract::Multipart;
use chrono::NaiveDate;
use serde_json::{Map, Value};

use crate::{
    domain::Category,
    error::AppError,
    storage::{MAX_ATTACHMENT_BYTES, MAX_ATTACHMENTS_PER_SUBMISSION, StorageError, UploadInput},
    validation::{SubmissionInput, ValidationErrors},
};

pub(super) async fn parse_multipart(
    multipart: &mut Multipart,
) -> Result<(BTreeMap<String, String>, Vec<UploadInput>, String), AppError> {
    let mut values = BTreeMap::new();
    let mut uploads = Vec::new();
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::Multipart)?
    {
        let name = field.name().unwrap_or_default().to_owned();
        if let Some(original_name) = field.file_name().map(str::to_owned) {
            let mime_type = field
                .content_type()
                .unwrap_or("application/octet-stream")
                .to_owned();
            let mut bytes = Vec::new();
            while let Some(chunk) = field.chunk().await.map_err(|_| AppError::Multipart)? {
                if bytes.len().saturating_add(chunk.len()) as u64 > MAX_ATTACHMENT_BYTES {
                    return Err(StorageError::InvalidSize.into());
                }
                bytes.extend_from_slice(&chunk);
            }
            // Browsers send an empty file part when an optional file input is untouched.
            if original_name.is_empty() && bytes.is_empty() {
                continue;
            }
            if uploads.len() >= MAX_ATTACHMENTS_PER_SUBMISSION {
                return Err(StorageError::TooManyAttachments.into());
            }
            uploads.push(UploadInput::new(original_name, mime_type, bytes));
        } else {
            values.insert(name, field.text().await.map_err(|_| AppError::Multipart)?);
        }
    }
    let csrf = values.get("csrf_token").cloned().unwrap_or_default();
    Ok((values, uploads, csrf))
}

pub(super) fn submission_input(
    values: &BTreeMap<String, String>,
) -> Result<SubmissionInput, ValidationErrors> {
    let mut errors = ValidationErrors::new();
    let category = values
        .get("category")
        .and_then(|value| Category::from_str(value).ok());
    let obtained_date = values
        .get("obtained_date")
        .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok());
    if category.is_none() {
        errors.add("category", "成果类别为必填项");
    }
    if obtained_date.is_none() {
        errors.add("obtained_date", "取得日期格式无效");
    }
    let (Some(category), Some(obtained_date)) = (category, obtained_date) else {
        return Err(errors);
    };
    let category_data = values
        .iter()
        .filter(|(key, _)| {
            !matches!(
                key.as_str(),
                "csrf_token"
                    | "has_result"
                    | "student_name"
                    | "student_no"
                    | "result_name"
                    | "obtained_date"
                    | "category"
                    | "detail"
                    | "remark"
                    | "no_result_confirm"
            )
        })
        .filter(|(_, value)| !value.trim().is_empty())
        .map(|(key, value)| (key.clone(), Value::String(value.clone())))
        .collect::<Map<String, Value>>();
    Ok(SubmissionInput {
        student_name: values.get("student_name").cloned().unwrap_or_default(),
        student_no: values.get("student_no").cloned().unwrap_or_default(),
        result_name: values.get("result_name").cloned().unwrap_or_default(),
        obtained_date,
        detail: optional_value(values, "detail"),
        remark: optional_value(values, "remark"),
        category,
        category_data: Value::Object(category_data),
    })
}

fn optional_value(values: &BTreeMap<String, String>, key: &str) -> Option<String> {
    values
        .get(key)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
