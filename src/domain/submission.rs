use chrono::{DateTime, NaiveDate, Utc};
use serde_json::Value;

use super::Category;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmissionStatus {
    Pending,
    Approved,
    NeedsRevision,
    Rejected,
}

impl SubmissionStatus {
    pub const fn can_student_edit(self) -> bool {
        matches!(self, Self::Pending | Self::NeedsRevision)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::NeedsRevision => "needs_revision",
            Self::Rejected => "rejected",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Pending => "待审核",
            Self::Approved => "已通过",
            Self::NeedsRevision => "需补充材料",
            Self::Rejected => "不予认定",
        }
    }
}

impl std::fmt::Display for SubmissionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SubmissionStatus {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pending" => Ok(Self::Pending),
            "approved" => Ok(Self::Approved),
            "needs_revision" => Ok(Self::NeedsRevision),
            "rejected" => Ok(Self::Rejected),
            _ => Err(format!("unknown submission status: {value}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Submission {
    pub id: i64,
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
    pub status: SubmissionStatus,
    pub review_note: Option<String>,
    pub approved_score: Option<f64>,
    pub edit_code_hash: String,
    pub student_modified_after_review: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
