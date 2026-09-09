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
    export_url: String,
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
    let export_url = filtered_export_url(&filter);
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
        export_url,
    }
    .render()
    .map(Html)
    .map_err(|_| AppError::Template)
}

fn filtered_export_url(filter: &SubmissionFilter) -> String {
    // Explicit year (including empty = all) keeps the download aligned with the
    // displayed table even if an administrator activates another year meanwhile.
    let values = BTreeMap::from([
        (
            "academic_year_id".into(),
            filter
                .academic_year_id
                .map(|id| id.to_string())
                .unwrap_or_default(),
        ),
        ("name".into(), filter.name.clone().unwrap_or_default()),
        (
            "student_no".into(),
            filter.student_no.clone().unwrap_or_default(),
        ),
        (
            "category".into(),
            filter
                .category
                .map(|c| c.as_str().to_owned())
                .unwrap_or_default(),
        ),
        (
            "status".into(),
            filter
                .status
                .map(|s| s.as_str().to_owned())
                .unwrap_or_default(),
        ),
    ]);
    format!("/admin/export.xlsx?{}", encode_query(&values))
}

pub(super) fn encode_query(values: &BTreeMap<String, String>) -> String {
    [
        "academic_year_id",
        "name",
        "student_no",
        "category",
        "status",
    ]
    .into_iter()
    .filter_map(|key| {
        values
            .get(key)
            .map(|value| format!("{key}={}", urlencoding::encode(value)))
    })
    .collect::<Vec<_>>()
    .join("&")
}
