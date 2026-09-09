use crate::{
    domain::{AcademicYear, Attachment, Submission},
    validation::ValidationErrors,
};
use askama::Template;
use std::collections::BTreeMap;

#[derive(Template)]
#[template(path = "student/home.html")]
struct HomeTemplate {
    year: Option<AcademicYear>,
    csrf_token: String,
    class_name: String,
    site_title: String,
    error: Option<String>,
}

#[derive(Template)]
#[template(path = "student/submit.html")]
struct SubmitTemplate {
    year: AcademicYear,
    form: SubmissionForm,
}

struct SubmissionForm {
    csrf_token: String,
    values: BTreeMap<String, String>,
    errors: ValidationErrors,
    action: String,
    editing: bool,
}
impl SubmissionForm {
    fn value(&self, key: &str) -> &str {
        self.values.get(key).map(String::as_str).unwrap_or("")
    }
    fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
    fn selected(&self, key: &str, value: &str) -> bool {
        self.value(key) == value
    }
    fn checked(&self, key: &str, value: &str) -> bool {
        let current = self.value(key);
        (current.is_empty() && value == "yes") || current == value
    }
    fn category_sections(&self) -> Vec<super::fields::CategorySection> {
        super::fields::category_sections()
    }
    fn active_field(&self, category: &str, field: &super::fields::Field) -> bool {
        self.value("category") == category
            && (field.when_key.is_empty() || self.value(field.when_key) == field.when_value)
    }
}
#[derive(Template)]
#[template(path = "student/success.html")]
struct SuccessTemplate {
    submission_no: String,
    result_name: String,
    edit_code: String,
}
#[derive(Template)]
#[template(path = "student/declaration.html")]
struct DeclarationTemplate;

pub fn home(
    year: Option<AcademicYear>,
    csrf_token: String,
    class_name: String,
    site_title: String,
    error: Option<String>,
) -> Result<String, askama::Error> {
    HomeTemplate {
        year,
        csrf_token,
        class_name,
        site_title,
        error,
    }
    .render()
}
pub fn submit(
    year: AcademicYear,
    csrf_token: String,
    values: BTreeMap<String, String>,
    errors: ValidationErrors,
) -> Result<String, askama::Error> {
    SubmitTemplate {
        year,
        form: SubmissionForm {
            csrf_token,
            values,
            errors,
            action: "/submit".to_owned(),
            editing: false,
        },
    }
    .render()
}
pub fn success(
    submission_no: String,
    result_name: String,
    edit_code: String,
) -> Result<String, askama::Error> {
    SuccessTemplate {
        submission_no,
        result_name,
        edit_code,
    }
    .render()
}
pub fn declaration() -> Result<String, askama::Error> {
    DeclarationTemplate.render()
}

#[derive(Template)]
#[template(path = "student/query.html")]
struct QueryTemplate {
    csrf_token: String,
    submission_no: String,
    invalid: bool,
}

pub fn query(
    csrf_token: String,
    submission_no: String,
    invalid: bool,
) -> Result<String, askama::Error> {
    QueryTemplate {
        csrf_token,
        submission_no,
        invalid,
    }
    .render()
}

#[derive(Template)]
#[template(path = "student/detail.html")]
struct DetailTemplate {
    year: AcademicYear,
    submission: Submission,
    attachments: Vec<Attachment>,
    category_fields: Vec<(&'static str, String)>,
    form: SubmissionForm,
}

pub fn detail(
    year: AcademicYear,
    submission: Submission,
    attachments: Vec<Attachment>,
    csrf_token: String,
    values: Option<BTreeMap<String, String>>,
    errors: ValidationErrors,
) -> Result<String, askama::Error> {
    let (mut stored_values, category_fields) = submission_values(&submission);
    stored_values.extend([
        ("student_name".to_owned(), submission.student_name.clone()),
        ("student_no".to_owned(), submission.student_no.clone()),
        ("result_name".to_owned(), submission.result_name.clone()),
        (
            "obtained_date".to_owned(),
            submission.obtained_date.to_string(),
        ),
        (
            "category".to_owned(),
            submission.category.as_str().to_owned(),
        ),
        (
            "detail".to_owned(),
            submission.detail.clone().unwrap_or_default(),
        ),
        (
            "remark".to_owned(),
            submission.remark.clone().unwrap_or_default(),
        ),
    ]);
    let form = SubmissionForm {
        csrf_token,
        values: values.unwrap_or(stored_values),
        errors,
        action: format!("/query/{}/update", submission.submission_no),
        editing: true,
    };
    DetailTemplate {
        year,
        submission,
        attachments,
        category_fields,
        form,
    }
    .render()
}

pub(super) fn submission_values(
    submission: &Submission,
) -> (BTreeMap<String, String>, Vec<(&'static str, String)>) {
    let mut stored_values = BTreeMap::new();
    if let Some(data) = submission.category_data.as_object() {
        for (key, value) in data {
            let text = match value {
                serde_json::Value::String(value) => value.clone(),
                serde_json::Value::Bool(value) => if *value { "yes" } else { "no" }.to_owned(),
                serde_json::Value::Number(value) => value.to_string(),
                _ => continue,
            };
            let text = match (key.as_str(), text.as_str()) {
                ("has_award_level" | "is_seu_sports_meet" | "is_scholarship", "是") => "yes",
                ("has_award_level" | "is_seu_sports_meet" | "is_scholarship", "否") => "no",
                ("is_scholarship", "不确定") => "uncertain",
                ("nature", "学术论文") => "academic",
                ("nature", "非学术文章") => "non_academic",
                ("certificate_type", "计算机") => "computer",
                ("certificate_type", "其他资格证书") => "other",
                _ => &text,
            }
            .to_owned();
            stored_values.insert(key.clone(), text);
        }
    }
    // Legacy award keys accepted by validation must remain visible at the source
    // linked from an export, and editable through the canonical form field.
    if submission.category == crate::domain::Category::AcademicCompetition
        && stored_values
            .get("other_award")
            .is_none_or(|value| value.trim().is_empty())
        && let Some(value) = ["award_detail", "actual_award_rank"]
            .into_iter()
            .filter_map(|key| stored_values.get(key))
            .find(|value| !value.trim().is_empty())
            .cloned()
    {
        stored_values.insert("other_award".into(), value);
    }
    let mut category_fields = Vec::new();
    for section in super::fields::category_sections() {
        if section.category != submission.category {
            continue;
        }
        for field in section.fields {
            if !field.when_key.is_empty()
                && stored_values.get(field.when_key).map(String::as_str) != Some(field.when_value)
            {
                continue;
            }
            if let Some(value) = stored_values
                .get(field.key)
                .filter(|value| !value.is_empty())
            {
                let label = field
                    .options
                    .iter()
                    .find(|(key, _)| *key == value)
                    .map_or(value.as_str(), |(_, label)| *label);
                category_fields.push((field.label, label.to_owned()));
            }
        }
    }
    (stored_values, category_fields)
}
