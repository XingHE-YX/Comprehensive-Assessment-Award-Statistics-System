use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudentDeclaration {
    pub id: i64,
    pub academic_year_id: i64,
    pub student_name: String,
    pub student_no: String,
    pub has_submission_material: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
