use crate::{
    db::{DeclarationRepo, SubmissionFilter, SubmissionRepo},
    domain::{StudentDeclaration, Submission, SubmissionStatus},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardRowKind {
    Submission,
    Declaration,
}

pub enum DashboardRow {
    Submission(Box<Submission>),
    Declaration(StudentDeclaration),
}

impl DashboardRow {
    pub fn record_key(&self) -> String {
        match self {
            Self::Submission(value) => format!("s:{}", value.id),
            Self::Declaration(value) => format!("d:{}", value.id),
        }
    }
    pub const fn kind(&self) -> DashboardRowKind {
        match self {
            Self::Submission(_) => DashboardRowKind::Submission,
            Self::Declaration(_) => DashboardRowKind::Declaration,
        }
    }

    pub fn student_name(&self) -> &str {
        match self {
            Self::Submission(value) => &value.student_name,
            Self::Declaration(value) => &value.student_name,
        }
    }

    pub fn student_no(&self) -> &str {
        match self {
            Self::Submission(value) => &value.student_no,
            Self::Declaration(value) => &value.student_no,
        }
    }

    pub fn submission_no(&self) -> Option<&str> {
        match self {
            Self::Submission(value) => Some(&value.submission_no),
            Self::Declaration(_) => None,
        }
    }

    pub fn result_name(&self) -> Option<&str> {
        match self {
            Self::Submission(value) => Some(&value.result_name),
            Self::Declaration(_) => None,
        }
    }

    pub fn category_label(&self) -> Option<&'static str> {
        match self {
            Self::Submission(value) => Some(value.category.label()),
            Self::Declaration(_) => None,
        }
    }

    pub fn status_label(&self) -> &'static str {
        match self {
            Self::Submission(value) => value.status.label(),
            Self::Declaration(_) => "无申报材料",
        }
    }

    pub fn status_key(&self) -> Option<&'static str> {
        match self {
            Self::Submission(value) => Some(value.status.as_str()),
            Self::Declaration(_) => None,
        }
    }

    pub fn score_label(&self) -> Option<String> {
        match self {
            Self::Submission(value) => Some(
                value
                    .approved_score
                    .map_or_else(|| "未核定".to_owned(), |score| score.to_string()),
            ),
            Self::Declaration(_) => None,
        }
    }

    pub fn student_modified_after_review(&self) -> bool {
        match self {
            Self::Submission(value) => value.student_modified_after_review,
            Self::Declaration(_) => false,
        }
    }

    pub fn created_at_rfc3339(&self) -> String {
        self.created_at().to_rfc3339()
    }

    pub fn created_at_label(&self) -> String {
        self.created_at().format("%Y-%m-%d %H:%M UTC").to_string()
    }

    pub fn detail_url(&self) -> Option<String> {
        match self {
            Self::Submission(value) => Some(format!("/admin/submissions/{}", value.id)),
            Self::Declaration(_) => None,
        }
    }

    fn created_at(&self) -> &chrono::DateTime<chrono::Utc> {
        match self {
            Self::Submission(value) => &value.created_at,
            Self::Declaration(value) => &value.created_at,
        }
    }

    fn id(&self) -> i64 {
        match self {
            Self::Submission(value) => value.id,
            Self::Declaration(value) => value.id,
        }
    }

    fn kind_rank(&self) -> u8 {
        match self {
            Self::Submission(_) => 0,
            Self::Declaration(_) => 1,
        }
    }
}

pub struct DashboardData {
    pub submissions: Vec<Submission>,
    pub rows: Vec<DashboardRow>,
    pub status_counts: Vec<(SubmissionStatus, usize)>,
    pub declaration_count: i64,
    pub approved_total: String,
}

impl DashboardData {
    pub async fn load(
        pool: &sqlx::SqlitePool,
        filter: &SubmissionFilter,
    ) -> Result<Self, sqlx::Error> {
        let submissions = SubmissionRepo::list(pool, filter).await?;
        let status_counts = [
            SubmissionStatus::Pending,
            SubmissionStatus::Approved,
            SubmissionStatus::NeedsRevision,
            SubmissionStatus::Rejected,
        ]
        .into_iter()
        .map(|status| {
            (
                status,
                submissions.iter().filter(|s| s.status == status).count(),
            )
        })
        .collect();
        let approved_total: f64 = submissions
            .iter()
            .filter(|s| s.status == SubmissionStatus::Approved)
            .filter_map(|s| s.approved_score)
            // f64::sum uses negative zero for an empty iterator. Start at +0 for display.
            .fold(0.0, |total, score| total + score);
        let declarations = DeclarationRepo::list(pool, filter).await?;
        let declaration_count = declarations.len() as i64;
        let mut rows: Vec<_> = submissions
            .iter()
            .cloned()
            .map(|submission| DashboardRow::Submission(Box::new(submission)))
            .chain(declarations.into_iter().map(DashboardRow::Declaration))
            .collect();
        rows.sort_by(|left, right| {
            right
                .created_at()
                .cmp(left.created_at())
                .then_with(|| left.kind_rank().cmp(&right.kind_rank()))
                .then_with(|| right.id().cmp(&left.id()))
        });
        Ok(Self {
            submissions,
            rows,
            status_counts,
            declaration_count,
            approved_total: format!("{approved_total:.2}"),
        })
    }
}
