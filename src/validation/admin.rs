use std::collections::BTreeMap;

use super::{ValidationErrors, validate_score};
use crate::{
    db::SubmissionFilter,
    domain::{AcademicYear, SubmissionStatus},
};

pub struct ValidatedReview {
    pub status: SubmissionStatus,
    pub review_note: Option<String>,
    pub approved_score: Option<f64>,
}

pub fn validate_review(
    status: &str,
    note: &str,
    score: &str,
) -> Result<ValidatedReview, ValidationErrors> {
    let mut errors = ValidationErrors::new();
    let status = status
        .parse::<SubmissionStatus>()
        .map_err(|_| errors.add("status", "请选择有效的审核状态"))
        .ok();
    let approved_score = match validate_score(Some(score)) {
        Ok(value) => value,
        Err(other) => {
            errors.extend(other);
            None
        }
    };
    if status == Some(SubmissionStatus::Approved) && score.trim().is_empty() {
        errors.add("approved_score", "已通过的申报必须填写核定分值，可填写 0");
    }
    let note = note.trim();
    if note.chars().count() > 4000 {
        errors.add("review_note", "审核备注不能超过 4000 个字符");
    }
    match status {
        Some(status) if errors.is_empty() => Ok(ValidatedReview {
            status,
            approved_score,
            review_note: (!note.is_empty()).then(|| note.to_owned()),
        }),
        _ => Err(errors),
    }
}

pub fn dashboard_filters(
    values: &BTreeMap<String, String>,
    years: &[AcademicYear],
) -> (SubmissionFilter, bool) {
    let current = years.iter().find(|year| year.is_active).map(|year| year.id);
    let mut filter = SubmissionFilter {
        academic_year_id: current,
        ..Default::default()
    };
    let mut invalid = false;
    if let Some(raw) = values.get("academic_year_id") {
        if raw.trim().is_empty() {
            filter.academic_year_id = None;
        } else if let Ok(id) = raw.parse::<i64>() {
            if years.iter().any(|year| year.id == id) {
                filter.academic_year_id = Some(id);
            } else {
                invalid = true;
            }
        } else {
            invalid = true;
        }
    }
    for (key, max, target) in [
        ("name", 50, &mut filter.name),
        ("student_no", 30, &mut filter.student_no),
    ] {
        if let Some(value) = values.get(key).map(|s| s.trim()).filter(|s| !s.is_empty()) {
            if value.chars().count() <= max && !value.chars().any(char::is_control) {
                *target = Some(value.to_owned());
            } else {
                invalid = true;
            }
        }
    }
    if let Some(value) = values.get("category").filter(|s| !s.is_empty()) {
        match value.parse() {
            Ok(category) => filter.category = Some(category),
            Err(_) => invalid = true,
        }
    }
    if let Some(value) = values.get("status").filter(|s| !s.is_empty()) {
        match value.parse() {
            Ok(status) => filter.status = Some(status),
            Err(_) => invalid = true,
        }
    }
    (filter, invalid)
}
