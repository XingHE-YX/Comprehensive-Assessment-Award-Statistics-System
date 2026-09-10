use askama::Template;
use axum::extract::rejection::{FormRejection, PathRejection, QueryRejection};
use axum::{
    Form,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    auth,
    db::{AcademicYearRepo, AttachmentRepo, SubmissionRepo},
    domain::{AcademicYear, Attachment, Submission, SubmissionStatus},
    error::AppError,
    state::AppState,
    validation::{ValidationErrors, admin::validate_review},
};

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct ReviewForm {
    csrf_token: String,
    status: String,
    review_note: String,
    approved_score: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct DetailQuery {
    saved: String,
    code_reset: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct ResetForm {
    csrf_token: String,
    confirm_reset: String,
    edit_code_version: Option<i64>,
}

struct AttachmentView {
    attachment: Attachment,
    available: bool,
    image: bool,
}

#[derive(Template)]
#[template(path = "admin/submission.html")]
struct DetailTemplate {
    csrf_token: String,
    year: AcademicYear,
    submission: Submission,
    category_fields: Vec<(&'static str, String)>,
    attachments: Vec<AttachmentView>,
    form: ReviewForm,
    errors: ValidationErrors,
    saved: bool,
    code_reset: bool,
    edit_code: Option<String>,
}
impl DetailTemplate {
    fn statuses(&self) -> [SubmissionStatus; 4] {
        [
            SubmissionStatus::Pending,
            SubmissionStatus::Approved,
            SubmissionStatus::NeedsRevision,
            SubmissionStatus::Rejected,
        ]
    }
}

async fn load(state: &AppState, id: i64) -> Result<Submission, AppError> {
    SubmissionRepo::find_by_id(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)
}

pub async fn detail(
    State(state): State<AppState>,
    axum::Extension(vault): axum::Extension<std::sync::Arc<auth::EditCodeVault>>,
    session: Session,
    id: Result<Path<i64>, PathRejection>,
    query: Result<Query<DetailQuery>, QueryRejection>,
) -> Result<Response, AppError> {
    let Path(id) = id.map_err(|_| AppError::BadRequest)?;
    let Query(query) = query.map_err(|_| AppError::BadRequest)?;
    let submission = load(&state, id).await?;
    render(
        &state,
        &vault,
        &session,
        submission,
        None,
        ValidationErrors::new(),
        query,
    )
    .await
}

async fn render(
    state: &AppState,
    vault: &auth::EditCodeVault,
    session: &Session,
    submission: Submission,
    form: Option<ReviewForm>,
    errors: ValidationErrors,
    query: DetailQuery,
) -> Result<Response, AppError> {
    let year = AcademicYearRepo::find_by_id(&state.db, submission.academic_year_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let mut attachments = Vec::new();
    for attachment in AttachmentRepo::list_for_submission(&state.db, submission.id).await? {
        let available = state.storage.open(&attachment.stored_name).await.is_ok();
        let image = matches!(attachment.mime_type.as_str(), "image/jpeg" | "image/png");
        attachments.push(AttachmentView {
            attachment,
            available,
            image,
        });
    }
    let (_, category_fields) = super::views::submission_values(&submission);
    let form = form.unwrap_or_else(|| ReviewForm {
        csrf_token: String::new(),
        status: submission.status.as_str().into(),
        review_note: submission.review_note.clone().unwrap_or_default(),
        approved_score: submission
            .approved_score
            .map(|s| format!("{s:.2}"))
            .unwrap_or_default(),
    });
    let status = if errors.is_empty() {
        StatusCode::OK
    } else {
        StatusCode::UNPROCESSABLE_ENTITY
    };
    let html = DetailTemplate {
        edit_code: crate::services::submissions::recover_edit_code(vault, &submission),
        csrf_token: auth::generate_csrf_token(session).await?,
        year,
        submission,
        category_fields,
        attachments,
        form,
        errors,
        saved: query.saved == "1",
        code_reset: query.code_reset == "1",
    }
    .render()
    .map_err(|_| AppError::Template)?;
    Ok((status, Html(html)).into_response())
}

pub async fn review(
    State(state): State<AppState>,
    axum::Extension(vault): axum::Extension<std::sync::Arc<auth::EditCodeVault>>,
    session: Session,
    id: Result<Path<i64>, PathRejection>,
    form: Result<Form<ReviewForm>, FormRejection>,
) -> Result<Response, AppError> {
    let Path(id) = id.map_err(|_| AppError::BadRequest)?;
    let Form(form) = form.map_err(AppError::from)?;
    if !auth::verify_csrf_token(&session, &form.csrf_token).await? {
        return Err(AppError::BadRequest);
    }
    let submission = load(&state, id).await?;
    let validated = match validate_review(&form.status, &form.review_note, &form.approved_score) {
        Ok(value) => value,
        Err(errors) => {
            return render(
                &state,
                &vault,
                &session,
                submission,
                Some(form),
                errors,
                DetailQuery::default(),
            )
            .await;
        }
    };
    SubmissionRepo::update_review(
        &state.db,
        id,
        validated.status,
        validated.review_note.as_deref(),
        validated.approved_score,
    )
    .await?;
    tracing::info!(
        submission_id = id,
        status = validated.status.as_str(),
        "admin review saved"
    );
    Ok(Redirect::to(&format!("/admin/submissions/{id}?saved=1")).into_response())
}

pub async fn reset_edit_code(
    State(state): State<AppState>,
    axum::Extension(vault): axum::Extension<std::sync::Arc<auth::EditCodeVault>>,
    session: Session,
    id: Result<Path<i64>, PathRejection>,
    form: Result<Form<ResetForm>, FormRejection>,
) -> Result<Response, AppError> {
    let Path(id) = id.map_err(|_| AppError::BadRequest)?;
    let Form(form) = form.map_err(AppError::from)?;
    if !auth::verify_csrf_token(&session, &form.csrf_token).await? || form.confirm_reset != "yes" {
        return Err(AppError::BadRequest);
    }
    let version = form
        .edit_code_version
        .filter(|version| *version >= 0)
        .ok_or(AppError::BadRequest)?;
    let submission = load(&state, id).await?;
    crate::services::submissions::reset_edit_code(&state, &vault, &submission, version).await?;
    Ok(Redirect::to(&format!(
        "/admin/submissions/{id}?code_reset=1#edit-code-panel"
    ))
    .into_response())
}
