use chrono::{NaiveDate, Utc};
use serde_json::Value;

use crate::{
    domain::{AcademicYear, Category},
    storage::UploadInput,
};

use super::{
    ValidationErrors, category::validate_category, upload::validate_uploads_with_existing,
};

#[derive(Debug, Clone)]
pub struct SubmissionInput {
    pub student_name: String,
    pub student_no: String,
    pub result_name: String,
    pub obtained_date: NaiveDate,
    pub detail: Option<String>,
    pub remark: Option<String>,
    pub category: Category,
    pub category_data: Value,
}

#[derive(Debug, Clone)]
pub struct ValidatedSubmission {
    pub student_name: String,
    pub student_no: String,
    pub result_name: String,
    pub obtained_date: NaiveDate,
    pub detail: Option<String>,
    pub remark: Option<String>,
    pub category: Category,
    pub category_data: Value,
    pub uploads: Vec<super::ValidatedUpload>,
}

pub fn validate_submission(
    input: SubmissionInput,
    current_year: &AcademicYear,
    uploads: &[UploadInput],
) -> Result<ValidatedSubmission, ValidationErrors> {
    validate_fields(input, current_year, uploads, 0, true)
}

pub fn validate_student_update(
    input: SubmissionInput,
    submission_year: &AcademicYear,
    uploads: &[UploadInput],
    existing_count: usize,
) -> Result<ValidatedSubmission, ValidationErrors> {
    validate_fields(input, submission_year, uploads, existing_count, false)
}

fn validate_fields(
    input: SubmissionInput,
    current_year: &AcademicYear,
    uploads: &[UploadInput],
    existing_count: usize,
    check_deadline: bool,
) -> Result<ValidatedSubmission, ValidationErrors> {
    let mut errors = ValidationErrors::new();
    let student_name = trimmed(&input.student_name);
    let student_no = trimmed(&input.student_no);
    let result_name = trimmed(&input.result_name);
    if !in_length(&student_name, 1, 50) {
        errors.add("student_name", "姓名为 1-50 个字符");
    }
    if !in_length(&student_no, 1, 30) {
        errors.add("student_no", "学号为 1-30 个字符");
    }
    if !in_length(&result_name, 1, 200) {
        errors.add("result_name", "成果名称为 1-200 个字符");
    }
    if input.obtained_date < current_year.start_date || input.obtained_date > current_year.end_date
    {
        errors.add("obtained_date", "取得日期必须在当前学年的起止日期内");
    }
    if let Some(detail) = &input.detail {
        if detail.trim().chars().count() > 4000 {
            errors.add("detail", "详细说明不能超过 4000 个字符");
        }
    }
    if let Some(remark) = &input.remark {
        if remark.trim().chars().count() > 4000 {
            errors.add("remark", "备注不能超过 4000 个字符");
        }
    }
    if let Err(category_errors) = validate_category(input.category, &input.category_data) {
        errors.extend(category_errors);
    }
    let validated_uploads = match validate_uploads_with_existing(uploads, existing_count) {
        Ok(value) => Some(value),
        Err(upload_errors) => {
            errors.extend(upload_errors);
            None
        }
    };
    if check_deadline && let Err(deadline_errors) = validate_deadline(current_year, Utc::now()) {
        errors.extend(deadline_errors);
    }

    if errors.is_empty() {
        Ok(ValidatedSubmission {
            student_name,
            student_no,
            result_name,
            obtained_date: input.obtained_date,
            detail: normalize_optional(input.detail),
            remark: normalize_optional(input.remark),
            category: input.category,
            category_data: input.category_data,
            uploads: validated_uploads.unwrap_or_default(),
        })
    } else {
        Err(errors)
    }
}

pub fn validate_deadline(
    current_year: &AcademicYear,
    now: chrono::DateTime<Utc>,
) -> Result<(), ValidationErrors> {
    let mut errors = ValidationErrors::new();
    if let Some(deadline) = current_year.deadline {
        if now > deadline {
            errors.add("deadline", "当前学年已超过成果申报截止时间");
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn validate_declaration(student_name: &str, student_no: &str) -> Result<(), ValidationErrors> {
    let mut errors = ValidationErrors::new();
    if !in_length(&trimmed(student_name), 1, 50) {
        errors.add("student_name", "姓名为 1-50 个字符");
    }
    if !in_length(&trimmed(student_no), 1, 30) {
        errors.add("student_no", "学号为 1-30 个字符");
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn validate_score(score: Option<&str>) -> Result<Option<f64>, ValidationErrors> {
    let Some(value) = score.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let mut errors = ValidationErrors::new();
    let parsed = value.parse::<f64>().ok();
    let decimal_places = value
        .split_once('.')
        .map_or(0, |(_, decimals)| decimals.len());
    let plain_decimal = value.split_once('.').map_or_else(
        || value.bytes().all(|byte| byte.is_ascii_digit()),
        |(integer, fraction)| {
            !integer.is_empty()
                && !fraction.is_empty()
                && integer.bytes().all(|byte| byte.is_ascii_digit())
                && fraction.bytes().all(|byte| byte.is_ascii_digit())
        },
    );
    if !plain_decimal
        || decimal_places > 2
        || parsed
            .is_none_or(|number| !number.is_finite() || number < 0.0 || round_two(number) != number)
    {
        errors.add("approved_score", "分值必须是非负且最多两位小数的数字");
        return Err(errors);
    }
    Ok(parsed)
}

fn round_two(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn trimmed(value: &str) -> String {
    value.trim().to_owned()
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_owned();
        (!value.is_empty()).then_some(value)
    })
}

fn in_length(value: &str, min: usize, max: usize) -> bool {
    let length = value.chars().count();
    (min..=max).contains(&length)
}
