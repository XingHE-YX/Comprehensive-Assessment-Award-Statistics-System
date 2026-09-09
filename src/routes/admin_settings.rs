use askama::Template;
use axum::{
    Form,
    extract::{
        Path, Query, State,
        rejection::{FormRejection, PathRejection, QueryRejection},
    },
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    auth,
    db::AcademicYearRepo,
    domain::AcademicYear,
    error::AppError,
    services::settings::{self, SettingsError},
    state::AppState,
    validation::{ValidationErrors, settings::AcademicYearInput},
};

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct SettingsQuery {
    edit: Option<i64>,
    saved: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct YearForm {
    csrf_token: String,
    #[serde(flatten)]
    fields: AcademicYearInput,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct CsrfForm {
    csrf_token: String,
}

// Never derive Debug or pass a submitted secret to a template.
#[derive(Default, Deserialize)]
#[serde(default)]
pub struct ClassCodeForm {
    csrf_token: String,
    class_access_code: String,
}

#[derive(Default)]
struct Editor {
    id: Option<i64>,
    fields: AcademicYearInput,
    errors: ValidationErrors,
}
impl Editor {
    fn action(&self) -> String {
        self.id
            .map(|id| format!("/admin/years/{id}"))
            .unwrap_or_else(|| "/admin/years".into())
    }
}

#[derive(Template)]
#[template(path = "admin/settings.html")]
struct SettingsTemplate {
    csrf_token: String,
    years: Vec<AcademicYear>,
    editor: Editor,
    code_errors: ValidationErrors,
    notice: String,
    saved: &'static str,
}

async fn render(
    state: &AppState,
    session: &Session,
    editor: Editor,
    code_errors: ValidationErrors,
    notice: String,
    saved: &'static str,
    status: StatusCode,
) -> Result<Response, AppError> {
    let html = SettingsTemplate {
        csrf_token: auth::generate_csrf_token(session).await?,
        years: AcademicYearRepo::list(&state.db).await?,
        editor,
        code_errors,
        notice,
        saved,
    }
    .render()
    .map_err(|_| AppError::Template)?;
    Ok((status, Html(html)).into_response())
}

pub async fn page(
    State(state): State<AppState>,
    session: Session,
    query: Result<Query<SettingsQuery>, QueryRejection>,
) -> Result<Response, AppError> {
    let Query(query) = query.map_err(|_| AppError::BadRequest)?;
    let editor = if let Some(id) = query.edit {
        let year = AcademicYearRepo::find_by_id(&state.db, id)
            .await?
            .ok_or(AppError::NotFound)?;
        Editor {
            id: Some(id),
            fields: AcademicYearInput::from(&year),
            errors: ValidationErrors::new(),
        }
    } else {
        Editor::default()
    };
    let saved = match query.saved.as_str() {
        "year" => "学年设置已保存。可在下方激活学年，或返回申报列表。",
        "active" => "当前学年已切换。学生下次进入时将使用新学年，历史申报仍可查看。",
        "code" => "班级口令已更新。新的口令立即用于验证，请妥善保存。",
        _ => "",
    };
    render(
        &state,
        &session,
        editor,
        ValidationErrors::new(),
        String::new(),
        saved,
        StatusCode::OK,
    )
    .await
}

async fn check_csrf(session: &Session, token: &str) -> Result<(), AppError> {
    if auth::verify_csrf_token(session, token).await? {
        Ok(())
    } else {
        Err(AppError::BadRequest)
    }
}

pub async fn create(
    State(state): State<AppState>,
    session: Session,
    form: Result<Form<YearForm>, FormRejection>,
) -> Result<Response, AppError> {
    let Form(form) = form.map_err(|_| AppError::BadRequest)?;
    save(&state, &session, None, form).await
}

pub async fn update(
    State(state): State<AppState>,
    session: Session,
    id: Result<Path<i64>, PathRejection>,
    form: Result<Form<YearForm>, FormRejection>,
) -> Result<Response, AppError> {
    let Path(id) = id.map_err(|_| AppError::BadRequest)?;
    let Form(form) = form.map_err(|_| AppError::BadRequest)?;
    save(&state, &session, Some(id), form).await
}

async fn save(
    state: &AppState,
    session: &Session,
    id: Option<i64>,
    form: YearForm,
) -> Result<Response, AppError> {
    check_csrf(session, &form.csrf_token).await?;
    if let Some(id) = id
        && AcademicYearRepo::find_by_id(&state.db, id).await?.is_none()
    {
        return Err(AppError::NotFound);
    }
    match settings::save_year(&state.db, id, &form.fields).await {
        Ok(()) => Ok(Redirect::to("/admin/settings?saved=year").into_response()),
        Err(error) => {
            settings_error(
                state,
                session,
                Editor {
                    id,
                    fields: form.fields,
                    errors: ValidationErrors::new(),
                },
                false,
                error,
            )
            .await
        }
    }
}

pub async fn activate(
    State(state): State<AppState>,
    session: Session,
    id: Result<Path<i64>, PathRejection>,
    form: Result<Form<CsrfForm>, FormRejection>,
) -> Result<Response, AppError> {
    let Path(id) = id.map_err(|_| AppError::BadRequest)?;
    let Form(form) = form.map_err(|_| AppError::BadRequest)?;
    check_csrf(&session, &form.csrf_token).await?;
    match settings::activate_year(&state.db, id).await {
        Ok(()) => Ok(Redirect::to("/admin/settings?saved=active").into_response()),
        Err(error) => settings_error(&state, &session, Editor::default(), false, error).await,
    }
}

pub async fn class_code(
    State(state): State<AppState>,
    session: Session,
    form: Result<Form<ClassCodeForm>, FormRejection>,
) -> Result<Response, AppError> {
    let Form(form) = form.map_err(|_| AppError::BadRequest)?;
    check_csrf(&session, &form.csrf_token).await?;
    match settings::replace_class_code(&state.db, form.class_access_code).await {
        Ok(()) => Ok(Redirect::to("/admin/settings?saved=code").into_response()),
        Err(error) => settings_error(&state, &session, Editor::default(), true, error).await,
    }
}

async fn settings_error(
    state: &AppState,
    session: &Session,
    mut editor: Editor,
    code_form: bool,
    error: SettingsError,
) -> Result<Response, AppError> {
    let mut code_errors = ValidationErrors::new();
    let (notice, status) = match error {
        SettingsError::Internal(error) => return Err(error),
        SettingsError::Conflict => (
            "设置正在被其他操作更新，本次修改未保存，请稍后重试。".into(),
            StatusCode::CONFLICT,
        ),
        SettingsError::Invalid(errors) => {
            if code_form {
                code_errors = errors;
            } else {
                editor.errors = errors;
            }
            (String::new(), StatusCode::UNPROCESSABLE_ENTITY)
        }
    };
    render(state, session, editor, code_errors, notice, "", status).await
}
