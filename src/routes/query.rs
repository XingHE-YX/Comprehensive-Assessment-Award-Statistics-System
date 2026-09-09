use std::collections::BTreeMap;

use axum::{
    Form,
    extract::{Multipart, Path, State},
    http::{StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use tokio::io::AsyncReadExt;
use tower_sessions::Session;

use super::{
    submission_form::{parse_multipart, submission_input},
    views,
};
use crate::{
    auth::{self, AuthError, generate_csrf_token, verify_csrf_token, verify_student_session},
    db::{AcademicYearRepo, AttachmentRepo, SubmissionRepo},
    domain::Submission,
    error::AppError,
    services,
    state::AppState,
    storage::StorageError,
    validation::{ValidationErrors, validate_student_update},
};

#[derive(Deserialize)]
pub struct QueryForm {
    #[serde(default)]
    csrf_token: String,
    #[serde(default)]
    submission_no: String,
    #[serde(default)]
    edit_code: String,
}

pub async fn query_get(session: Session) -> Result<Response, AppError> {
    let html = views::query(generate_csrf_token(&session).await?, String::new(), false)
        .map_err(|_| AppError::Template)?;
    Ok(Html(html).into_response())
}

pub async fn query_post(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<QueryForm>,
) -> Result<Response, AppError> {
    if !verify_csrf_token(&session, &form.csrf_token).await? {
        return Err(AppError::BadRequest);
    }
    let submission_no = form.submission_no.trim();
    match auth::verify_student_access(&state.db, submission_no, form.edit_code.trim()).await {
        Ok(submission) => {
            auth::establish_verified_student_session(&session, submission.id).await?;
            Ok(Redirect::to(&format!("/query/{}", submission.submission_no)).into_response())
        }
        Err(AuthError::InvalidCredentials) => {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let html = views::query(
                generate_csrf_token(&session).await?,
                submission_no.to_owned(),
                true,
            )
            .map_err(|_| AppError::Template)?;
            Ok((StatusCode::UNAUTHORIZED, Html(html)).into_response())
        }
        Err(error) => Err(error.into()),
    }
}

async fn verified_submission(
    state: &AppState,
    session: &Session,
    submission_no: &str,
) -> Result<Submission, AppError> {
    let submission = SubmissionRepo::find_by_no(&state.db, submission_no)
        .await?
        .ok_or(AppError::NotFound)?;
    verify_student_session(session, submission.id).await?;
    Ok(submission)
}

pub async fn detail(
    State(state): State<AppState>,
    session: Session,
    Path(submission_no): Path<String>,
) -> Result<Response, AppError> {
    let submission = verified_submission(&state, &session, &submission_no).await?;
    render_detail(
        &state,
        &session,
        submission,
        None,
        ValidationErrors::new(),
        StatusCode::OK,
    )
    .await
}

async fn render_detail(
    state: &AppState,
    session: &Session,
    submission: Submission,
    values: Option<BTreeMap<String, String>>,
    errors: ValidationErrors,
    status: StatusCode,
) -> Result<Response, AppError> {
    let year = AcademicYearRepo::find_by_id(&state.db, submission.academic_year_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let attachments = AttachmentRepo::list_for_submission(&state.db, submission.id).await?;
    let html = views::detail(
        year,
        submission,
        attachments,
        generate_csrf_token(session).await?,
        values,
        errors,
    )
    .map_err(|_| AppError::Template)?;
    Ok((status, Html(html)).into_response())
}

pub async fn update(
    State(state): State<AppState>,
    session: Session,
    Path(submission_no): Path<String>,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    let submission = verified_submission(&state, &session, &submission_no).await?;
    if !submission.status.can_student_edit() {
        return Err(AppError::Forbidden);
    }
    let (values, uploads, csrf) = parse_multipart(&mut multipart).await?;
    if !verify_csrf_token(&session, &csrf).await? {
        return Err(AppError::BadRequest);
    }
    let year = AcademicYearRepo::find_by_id(&state.db, submission.academic_year_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let existing = AttachmentRepo::list_for_submission(&state.db, submission.id).await?;
    let validated = submission_input(&values)
        .and_then(|input| validate_student_update(input, &year, &uploads, existing.len()));
    let result = match validated {
        Ok(input) => {
            services::submissions::update_student(&state, &submission, &year, &input, uploads).await
        }
        Err(errors) => Err(AppError::Validation(errors)),
    };
    match result {
        Ok(()) => {
            tracing::info!(submission_no = %submission_no, status = "pending", "student submission updated");
            Ok(Redirect::to(&format!("/query/{submission_no}")).into_response())
        }
        Err(AppError::Validation(errors)) => {
            render_detail(
                &state,
                &session,
                submission,
                Some(values),
                errors,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(error) => Err(error),
    }
}

pub async fn attachment(
    State(state): State<AppState>,
    session: Session,
    Path((submission_no, id)): Path<(String, i64)>,
) -> Result<Response, AppError> {
    let submission = SubmissionRepo::find_by_no(&state.db, &submission_no)
        .await?
        .ok_or(AppError::NotFound)?;
    match auth::require_admin(&session).await {
        Ok(_) => {}
        Err(AuthError::MissingSession) => verify_student_session(&session, submission.id).await?,
        Err(error) => return Err(error.into()),
    }
    let attachment = AttachmentRepo::find_by_id(&state.db, id)
        .await?
        .filter(|attachment| attachment.submission_id == submission.id)
        .ok_or(AppError::NotFound)?;
    let mut file = state.storage.open(&attachment.stored_name).await?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .await
        .map_err(StorageError::Io)?;
    let filename: String = attachment
        .original_name
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '/' | '\\'))
        .collect();
    let disposition = format!(
        "inline; filename=\"attachment\"; filename*=UTF-8''{}",
        urlencoding::encode(&filename)
    );
    Ok((
        [
            (header::CONTENT_TYPE, attachment.mime_type),
            (header::CONTENT_DISPOSITION, disposition),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
        ],
        bytes,
    )
        .into_response())
}
