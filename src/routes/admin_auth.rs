use std::{sync::Arc, time::Duration};

use askama::Template;
use axum::{
    Extension, Form,
    extract::{Request, rejection::FormRejection},
    http::{Method, StatusCode},
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    auth::{self, AdminCredentials, AuthError},
    error::AppError,
};

#[derive(Template)]
#[template(path = "admin/login.html")]
struct LoginTemplate {
    csrf_token: String,
    username: String,
    invalid: bool,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct LoginForm {
    csrf_token: String,
    username: String,
    password: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct LogoutForm {
    csrf_token: String,
}

pub async fn login_get(session: Session) -> Result<Response, AppError> {
    match auth::require_admin(&session).await {
        Ok(_) => return Ok(Redirect::to("/admin").into_response()),
        Err(AuthError::MissingSession) => {}
        Err(error) => return Err(error.into()),
    }
    render_login(&session, String::new(), false).await
}

async fn render_login(
    session: &Session,
    username: String,
    invalid: bool,
) -> Result<Response, AppError> {
    let html = LoginTemplate {
        csrf_token: auth::generate_csrf_token(session).await?,
        username,
        invalid,
    }
    .render()
    .map_err(|_| AppError::Template)?;
    Ok((
        if invalid {
            StatusCode::UNAUTHORIZED
        } else {
            StatusCode::OK
        },
        Html(html),
    )
        .into_response())
}

pub async fn login_post(
    Extension(credentials): Extension<Option<Arc<AdminCredentials>>>,
    session: Session,
    form: Result<Form<LoginForm>, FormRejection>,
) -> Result<Response, AppError> {
    let Form(form) = form.map_err(|_| AppError::BadRequest)?;
    if !auth::verify_csrf_token(&session, &form.csrf_token).await? {
        return Err(AppError::BadRequest);
    }
    let username = form.username.trim().to_owned();
    let valid = match credentials {
        Some(credentials) => credentials.verify(username.clone(), form.password).await?,
        None => false,
    };
    if !valid {
        tokio::time::sleep(Duration::from_millis(100)).await;
        return render_login(&session, username, true).await;
    }
    auth::establish_admin_session(&session).await?;
    Ok(Redirect::to("/admin").into_response())
}

pub async fn logout(
    session: Session,
    form: Result<Form<LogoutForm>, FormRejection>,
) -> Result<Response, AppError> {
    let Form(form) = form.map_err(|_| AppError::BadRequest)?;
    if !auth::verify_csrf_token(&session, &form.csrf_token).await? {
        return Err(AppError::BadRequest);
    }
    session.flush().await.map_err(AuthError::from)?;
    Ok(Redirect::to("/admin/login").into_response())
}

/// All protected admin routes run this before extractors or database access.
pub async fn require_admin(
    session: Session,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    match auth::require_admin(&session).await {
        Ok(_) => Ok(next.run(request).await),
        Err(AuthError::MissingSession)
            if matches!(*request.method(), Method::GET | Method::HEAD) =>
        {
            Ok(Redirect::to("/admin/login").into_response())
        }
        Err(error) => Err(error.into()),
    }
}
