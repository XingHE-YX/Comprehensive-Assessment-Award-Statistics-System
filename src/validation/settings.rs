use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, Utc};
use serde::Deserialize;

use super::ValidationErrors;
use crate::{db::NewAcademicYear, domain::AcademicYear};

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct AcademicYearInput {
    pub name: String,
    pub start_date: String,
    pub end_date: String,
    pub deadline: String,
    pub announcement: String,
}

impl From<&AcademicYear> for AcademicYearInput {
    fn from(year: &AcademicYear) -> Self {
        Self {
            name: year.name.clone(),
            start_date: year.start_date.to_string(),
            end_date: year.end_date.to_string(),
            deadline: year
                .deadline
                .map(|d| d.format("%Y-%m-%dT%H:%M:%S%.f").to_string())
                .unwrap_or_default(),
            announcement: year.announcement.clone().unwrap_or_default(),
        }
    }
}

fn date(value: &str) -> Option<NaiveDate> {
    let parsed = NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()?;
    (value.len() == 10 && parsed.to_string() == value && (1..=9999).contains(&parsed.year()))
        .then_some(parsed)
}

pub fn validate_year(input: &AcademicYearInput) -> Result<NewAcademicYear, ValidationErrors> {
    let mut errors = ValidationErrors::new();
    let name = input.name.trim();
    if !(1..=40).contains(&name.chars().count()) || name.chars().any(char::is_control) {
        errors.add("name", "学年名称须为 1-40 个字符，不能包含控制字符");
    }
    let start_date = date(input.start_date.trim());
    let end_date = date(input.end_date.trim());
    if start_date.is_none() {
        errors.add("start_date", "请输入有效的开始日期（YYYY-MM-DD）");
    }
    if end_date.is_none() {
        errors.add("end_date", "请输入有效的结束日期（YYYY-MM-DD）");
    }
    if let (Some(start), Some(end)) = (start_date, end_date)
        && end < start
    {
        errors.add("end_date", "结束日期不能早于开始日期");
    }
    let deadline = input.deadline.trim();
    let deadline = if deadline.is_empty() {
        None
    } else {
        // Native datetime-local values are explicitly UTC in this form. API-style
        // RFC 3339 values carry their own offset and are normalized before storage.
        let parsed = DateTime::parse_from_rfc3339(deadline)
            .map(|d| d.with_timezone(&Utc))
            .ok()
            .or_else(|| {
                let (day, _) = deadline.split_once('T')?;
                date(day)?;
                ["%Y-%m-%dT%H:%M", "%Y-%m-%dT%H:%M:%S%.f"]
                    .iter()
                    .find_map(|format| NaiveDateTime::parse_from_str(deadline, format).ok())
                    .map(|d| d.and_utc())
            })
            .filter(|d| {
                (1..=9999).contains(&d.year()) && d.timestamp_subsec_nanos() < 1_000_000_000
            });
        if parsed.is_none() {
            errors.add("deadline", "请输入有效的申报截止时间（UTC）");
        }
        parsed
    };
    let announcement = input.announcement.trim();
    if announcement.chars().count() > 4000 {
        errors.add("announcement", "首页说明最多 4000 个字符");
    }
    match (errors.is_empty(), start_date, end_date) {
        (true, Some(start_date), Some(end_date)) => Ok(NewAcademicYear {
            name: name.into(),
            start_date,
            end_date,
            deadline,
            is_active: false,
            announcement: (!announcement.is_empty()).then(|| announcement.into()),
        }),
        _ => Err(errors),
    }
}

pub fn validate_class_code(code: &str) -> Result<(), ValidationErrors> {
    let mut errors = ValidationErrors::new();
    if code.trim().is_empty() || code.chars().count() > 256 || code.chars().any(char::is_control) {
        errors.add(
            "class_access_code",
            "班级口令须为 1-256 个字符，不能全为空白或包含控制字符",
        );
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
