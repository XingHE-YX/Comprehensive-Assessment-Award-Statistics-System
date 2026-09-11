use crate::{
    auth,
    db::{AcademicYearRepo, RosterRepo, RosterStudent},
    domain::AcademicYear,
    error::AppError,
    services::roster,
    state::AppState,
};
use askama::Template;
use axum::{
    extract::{Multipart, Path, State},
    http::{StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct RosterDraft {
    pub id: String,
    pub students: Vec<RosterStudent>,
    expires_at: i64,
}

pub(super) async fn draft(session: &Session) -> Result<Option<RosterDraft>, AppError> {
    Ok(session
        .get::<RosterDraft>("roster_draft")
        .await
        .map_err(auth::AuthError::from)?
        .filter(|draft| draft.expires_at > chrono::Utc::now().timestamp()))
}

pub(super) async fn clear_draft(session: &Session) -> Result<(), AppError> {
    session
        .remove::<RosterDraft>("roster_draft")
        .await
        .map_err(auth::AuthError::from)?;
    Ok(())
}

async fn read_roster(
    session: &Session,
    mut multipart: Multipart,
) -> Result<Vec<RosterStudent>, AppError> {
    let mut token = String::new();
    let mut text = None;
    let mut file = None;
    let mut seen = std::collections::BTreeSet::new();
    while let Some(mut field) = multipart.next_field().await? {
        let key = field.name().unwrap_or_default().to_owned();
        if !seen.insert(key.clone()) {
            return Err(AppError::BadRequest);
        }
        let filename = field.file_name().unwrap_or_default().to_owned();
        let mut bytes = Vec::new();
        while let Some(chunk) = field.chunk().await? {
            if bytes.len() + chunk.len() > roster::MAX_ROSTER_BYTES {
                return Err(AppError::PayloadTooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
        match key.as_str() {
            "csrf_token" => token = String::from_utf8(bytes).map_err(|_| AppError::BadRequest)?,
            "roster_text" => {
                if !bytes.iter().all(u8::is_ascii_whitespace) {
                    text = Some(bytes);
                }
            }
            "roster_file" => {
                if !filename.is_empty() {
                    file = Some((filename, bytes));
                }
            }
            _ => return Err(AppError::BadRequest),
        }
    }
    if !auth::verify_csrf_token(session, &token).await? {
        return Err(AppError::BadRequest);
    }
    if text.is_some() && file.is_some() {
        let mut errors = crate::validation::ValidationErrors::new();
        errors.add("roster", "请选择上传文件或粘贴名单其中一种方式");
        return Err(AppError::Validation(errors));
    }
    let (name, bytes) = file
        .map(|(name, bytes)| (Some(name), bytes))
        .unwrap_or((None, text.unwrap_or_default()));
    tokio::task::spawn_blocking(move || roster::parse(name.as_deref(), &bytes))
        .await
        .map_err(|_| AppError::BadRequest)?
}

pub async fn prepare(
    State(state): State<AppState>,
    session: Session,
    multipart: Multipart,
) -> Result<Response, AppError> {
    match read_roster(&session, multipart).await {
        Ok(students) => {
            let draft = RosterDraft {
                id: uuid::Uuid::new_v4().to_string(),
                students,
                expires_at: chrono::Utc::now().timestamp() + 3600,
            };
            session
                .insert("roster_draft", draft)
                .await
                .map_err(auth::AuthError::from)?;
            Ok(Redirect::to("/admin/settings?saved=roster#year-form").into_response())
        }
        Err(AppError::Validation(errors)) => {
            super::admin_settings::roster_error(&state, &session, errors).await
        }
        Err(error) => Err(error),
    }
}

#[derive(Template)]
#[template(path = "admin/roster.html")]
struct RosterTemplate {
    csrf_token: String,
    year: AcademicYear,
    students: Vec<RosterStudent>,
    errors: crate::validation::ValidationErrors,
    saved: bool,
}

async fn render(
    state: &AppState,
    session: &Session,
    id: i64,
    errors: crate::validation::ValidationErrors,
    saved: bool,
) -> Result<Response, AppError> {
    let status = if errors.is_empty() {
        StatusCode::OK
    } else {
        StatusCode::UNPROCESSABLE_ENTITY
    };
    let html = RosterTemplate {
        csrf_token: auth::generate_csrf_token(session).await?,
        year: AcademicYearRepo::find_by_id(&state.db, id)
            .await?
            .ok_or(AppError::NotFound)?,
        students: RosterRepo::list(&state.db, id).await?,
        errors,
        saved,
    }
    .render()
    .map_err(|_| AppError::Template)?;
    Ok((status, Html(html)).into_response())
}

pub async fn page(
    State(state): State<AppState>,
    session: Session,
    id: Result<Path<i64>, axum::extract::rejection::PathRejection>,
    axum::extract::Query(query): axum::extract::Query<std::collections::BTreeMap<String, String>>,
) -> Result<Response, AppError> {
    let Path(id) = id.map_err(|_| AppError::BadRequest)?;
    render(
        &state,
        &session,
        id,
        crate::validation::ValidationErrors::new(),
        query.get("saved").is_some_and(|v| v == "1"),
    )
    .await
}

pub async fn append(
    State(state): State<AppState>,
    session: Session,
    id: Result<Path<i64>, axum::extract::rejection::PathRejection>,
    multipart: Multipart,
) -> Result<Response, AppError> {
    let Path(id) = id.map_err(|_| AppError::BadRequest)?;
    let result = match read_roster(&session, multipart).await {
        Ok(students) => roster::append(&state.db, id, &students).await,
        Err(error) => Err(error),
    };
    match result {
        Ok(_) => Ok(Redirect::to(&format!("/admin/years/{id}/students?saved=1")).into_response()),
        Err(AppError::Validation(errors)) => render(&state, &session, id, errors, false).await,
        Err(error) => Err(error),
    }
}

pub async fn template() -> Result<Response, AppError> {
    Ok((
        [
            (
                header::CONTENT_TYPE,
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            ),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=student-roster-template.xlsx",
            ),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        roster::template()?,
    )
        .into_response())
}
