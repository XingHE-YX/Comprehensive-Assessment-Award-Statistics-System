use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("forbidden")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("internal error")]
    Internal,
}

impl AppError {
    #[must_use]
    pub const fn forbidden() -> Self {
        Self::Forbidden
    }

    #[must_use]
    pub const fn not_found() -> Self {
        Self::NotFound
    }

    #[must_use]
    pub const fn internal() -> Self {
        Self::Internal
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            Self::Forbidden => render_error(StatusCode::FORBIDDEN, ForbiddenTemplate),
            Self::NotFound => render_error(StatusCode::NOT_FOUND, NotFoundTemplate),
            Self::Internal => {
                tracing::error!(error_kind = "internal", "请求处理失败");
                render_error(StatusCode::INTERNAL_SERVER_ERROR, InternalErrorTemplate)
            }
        }
    }
}

fn render_error<T: Template>(status: StatusCode, template: T) -> Response {
    match template.render() {
        Ok(body) => (status, Html(body)).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html("系统暂时无法处理请求，请稍后重试。"),
        )
            .into_response(),
    }
}

#[derive(Template)]
#[template(path = "errors/403.html")]
struct ForbiddenTemplate;

#[derive(Template)]
#[template(path = "errors/404.html")]
struct NotFoundTemplate;

#[derive(Template)]
#[template(path = "errors/500.html")]
struct InternalErrorTemplate;
