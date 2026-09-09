use std::collections::BTreeMap;

use askama::Template;
use axum::{
    extract::{Query, State, rejection::QueryRejection},
    response::Html,
};
use tower_sessions::Session;

use crate::{
    auth,
    db::{AcademicYearRepo, SubmissionFilter},
    domain::{AcademicYear, Category, SubmissionStatus},
    error::AppError,
    services::admin::DashboardData,
    state::AppState,
    validation::admin::dashboard_filters,
};

#[derive(Template)]
#[template(path = "admin/index.html")]
struct DashboardTemplate {
    csrf_token: String,
    years: Vec<AcademicYear>,
    active_year_name: String,
    filter: SubmissionFilter,
    invalid_filters: bool,
    data: DashboardData,
}
impl DashboardTemplate {
    fn categories(&self) -> &'static [Category] {
        Category::all()
    }
    fn statuses(&self) -> [SubmissionStatus; 4] {
        [
            SubmissionStatus::Pending,
            SubmissionStatus::Approved,
            SubmissionStatus::NeedsRevision,
            SubmissionStatus::Rejected,
        ]
    }
    fn year_selected(&self, id: &i64) -> bool {
        self.filter.academic_year_id == Some(*id)
    }
    fn category_selected(&self, category: &Category) -> bool {
        self.filter.category == Some(*category)
    }
    fn status_selected(&self, status: &SubmissionStatus) -> bool {
        self.filter.status == Some(*status)
    }
}

pub async fn dashboard(
    State(state): State<AppState>,
    session: Session,
    values: Result<Query<BTreeMap<String, String>>, QueryRejection>,
) -> Result<Html<String>, AppError> {
    let Query(values) = values.map_err(|_| AppError::BadRequest)?;
    let years = AcademicYearRepo::list(&state.db).await?;
    let (filter, invalid_filters) = dashboard_filters(&values, &years);
    let data = DashboardData::load(&state.db, &filter).await?;
    let active_year_name = years
        .iter()
        .find(|y| y.is_active)
        .map(|y| y.name.clone())
        .unwrap_or_else(|| "暂无开放学年".into());
    DashboardTemplate {
        csrf_token: auth::generate_csrf_token(&session).await?,
        years,
        active_year_name,
        filter,
        invalid_filters,
        data,
    }
    .render()
    .map(Html)
    .map_err(|_| AppError::Template)
}
