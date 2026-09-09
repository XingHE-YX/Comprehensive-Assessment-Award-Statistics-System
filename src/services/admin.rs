use crate::{
    db::{DeclarationRepo, SubmissionFilter, SubmissionRepo},
    domain::{Submission, SubmissionStatus},
};

pub struct DashboardData {
    pub submissions: Vec<Submission>,
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
        let declaration_count = DeclarationRepo::count(pool, filter).await?;
        Ok(Self {
            submissions,
            status_counts,
            declaration_count,
            approved_total: format!("{approved_total:.2}"),
        })
    }
}
