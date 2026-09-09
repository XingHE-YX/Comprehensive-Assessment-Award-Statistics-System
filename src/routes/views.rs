use crate::{domain::AcademicYear, validation::ValidationErrors};
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
    csrf_token: String,
    values: BTreeMap<String, String>,
    errors: ValidationErrors,
}
impl SubmitTemplate {
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
        csrf_token,
        values,
        errors,
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
