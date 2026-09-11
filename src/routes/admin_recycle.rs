use crate::{
    auth,
    db::{
        AcademicYearRepo, RecordKey, RecycleRecord, RecycleRepo, YearDeletionRepo,
        YearDeletionSummary,
    },
    domain::AcademicYear,
    error::AppError,
    state::AppState,
};
use askama::Template;
use axum::{
    Form,
    extract::{
        Path, Query, State,
        rejection::{FormRejection, PathRejection, QueryRejection},
    },
    response::{Html, IntoResponse, Redirect, Response},
};
use std::collections::BTreeMap;
use tower_sessions::Session;

#[derive(Template)]
#[template(path = "admin/recycle.html")]
struct RecycleTemplate {
    csrf_token: String,
    rows: Vec<RecycleRecord>,
    years: Vec<AcademicYear>,
    year_id: Option<i64>,
    confirm: bool,
    saved: bool,
}
impl RecycleTemplate {
    fn selected(&self, id: &i64) -> bool {
        self.year_id == Some(*id)
    }
}

fn keys(values: &BTreeMap<String, String>) -> Result<Vec<RecordKey>, AppError> {
    let mut keys = std::collections::BTreeSet::new();
    for (name, value) in values {
        if let Some(key) = name.strip_prefix("selected:") {
            if value != "yes" {
                return Err(AppError::BadRequest);
            }
            keys.insert(RecordKey::parse(key)?);
        }
    }
    if keys.is_empty() || keys.len() > 500 {
        return Err(AppError::BadRequest);
    }
    Ok(keys.into_iter().collect())
}

async fn check_csrf(session: &Session, values: &BTreeMap<String, String>) -> Result<(), AppError> {
    if !auth::verify_csrf_token(session, values.get("csrf_token").map_or("", String::as_str))
        .await?
    {
        return Err(AppError::BadRequest);
    }
    Ok(())
}

pub async fn page(
    State(state): State<AppState>,
    session: Session,
    query: Result<Query<BTreeMap<String, String>>, QueryRejection>,
) -> Result<Html<String>, AppError> {
    let Query(query) = query.map_err(|_| AppError::BadRequest)?;
    let year_id = query
        .get("academic_year_id")
        .filter(|v| !v.is_empty())
        .map(|v| v.parse::<i64>().map_err(|_| AppError::BadRequest))
        .transpose()?;
    RecycleTemplate {
        csrf_token: auth::generate_csrf_token(&session).await?,
        rows: RecycleRepo::list(&state.db, true, year_id).await?,
        years: AcademicYearRepo::list(&state.db).await?,
        year_id,
        confirm: false,
        saved: query.contains_key("saved"),
    }
    .render()
    .map(Html)
    .map_err(|_| AppError::Template)
}

pub async fn confirm(
    State(state): State<AppState>,
    session: Session,
    form: Result<Form<BTreeMap<String, String>>, FormRejection>,
) -> Result<Html<String>, AppError> {
    let Form(values) = form.map_err(AppError::from)?;
    check_csrf(&session, &values).await?;
    let selected = keys(&values)?;
    let rows: Vec<_> = RecycleRepo::list(&state.db, false, None)
        .await?
        .into_iter()
        .filter(|row| selected.iter().any(|key| key.value() == row.key))
        .collect();
    if rows.len() != selected.len() {
        return Err(AppError::Conflict);
    }
    RecycleTemplate {
        csrf_token: auth::generate_csrf_token(&session).await?,
        rows,
        years: Vec::new(),
        year_id: None,
        confirm: true,
        saved: false,
    }
    .render()
    .map(Html)
    .map_err(|_| AppError::Template)
}

pub async fn delete(
    State(state): State<AppState>,
    session: Session,
    form: Result<Form<BTreeMap<String, String>>, FormRejection>,
) -> Result<Response, AppError> {
    let Form(values) = form.map_err(AppError::from)?;
    check_csrf(&session, &values).await?;
    if values.get("confirm_delete").map(String::as_str) != Some("yes") {
        return Err(AppError::BadRequest);
    }
    let selected = keys(&values)?;
    RecycleRepo::change(&state.db, &selected, false).await?;
    tracing::info!(event = "records_deleted", count = selected.len());
    Ok(Redirect::to("/admin/recycle?saved=1").into_response())
}

pub async fn restore(
    State(state): State<AppState>,
    session: Session,
    form: Result<Form<BTreeMap<String, String>>, FormRejection>,
) -> Result<Response, AppError> {
    let Form(values) = form.map_err(AppError::from)?;
    check_csrf(&session, &values).await?;
    let selected = keys(&values)?;
    RecycleRepo::change(&state.db, &selected, true).await?;
    tracing::info!(event = "records_restored", count = selected.len());
    Ok(Redirect::to("/admin/recycle?saved=1").into_response())
}

#[derive(Template)]
#[template(path = "admin/delete_year.html")]
struct DeleteYearTemplate {
    csrf_token: String,
    year: YearDeletionSummary,
}

pub async fn year_page(
    State(state): State<AppState>,
    session: Session,
    id: Result<Path<i64>, PathRejection>,
) -> Result<Html<String>, AppError> {
    let Path(id) = id.map_err(|_| AppError::BadRequest)?;
    DeleteYearTemplate {
        csrf_token: auth::generate_csrf_token(&session).await?,
        year: YearDeletionRepo::summary(&state.db, id).await?,
    }
    .render()
    .map(Html)
    .map_err(|_| AppError::Template)
}

pub async fn delete_year(
    State(state): State<AppState>,
    session: Session,
    id: Result<Path<i64>, PathRejection>,
    form: Result<Form<BTreeMap<String, String>>, FormRejection>,
) -> Result<Response, AppError> {
    let Path(id) = id.map_err(|_| AppError::BadRequest)?;
    let Form(values) = form.map_err(AppError::from)?;
    check_csrf(&session, &values).await?;
    if values.get("confirm_delete").map(String::as_str) != Some("yes") {
        return Err(AppError::BadRequest);
    }
    YearDeletionRepo::delete(
        &state.db,
        id,
        values.get("confirm_name").map_or("", String::as_str),
    )
    .await?;
    crate::services::recycle::cleanup(&state).await?;
    tracing::info!(event = "academic_year_deleted", academic_year_id = id);
    Ok(Redirect::to("/admin/settings?saved=deleted").into_response())
}
