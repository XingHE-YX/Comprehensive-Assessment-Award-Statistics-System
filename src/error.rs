use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Auth(#[from] crate::auth::AuthError),
    #[error(transparent)]
    Storage(#[from] crate::storage::StorageError),
    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),
    #[error("数据库操作失败")]
    Database(#[from] sqlx::Error),
    #[error("请求无效")]
    BadRequest,
    #[error("资源不存在")]
    NotFound,
    #[error("无权访问")]
    Forbidden,
    #[error("记录已更新，请刷新后重试")]
    Conflict,
    #[error("表单校验失败")]
    Validation(crate::validation::ValidationErrors),
    #[error("模板渲染失败")]
    Template,
    #[error("请求体无法解析")]
    Multipart,
    #[error("请求内容过大")]
    PayloadTooLarge,
    #[error("导出失败，请稍后重试")]
    Export,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::Auth(crate::auth::AuthError::InvalidCredentials) => StatusCode::UNAUTHORIZED,
            Self::Auth(crate::auth::AuthError::MissingSession) | Self::Forbidden => {
                StatusCode::FORBIDDEN
            }
            Self::Storage(crate::storage::StorageError::InvalidPath)
            | Self::Storage(crate::storage::StorageError::UnsupportedType) => {
                StatusCode::BAD_REQUEST
            }
            Self::Storage(crate::storage::StorageError::InvalidSize)
            | Self::Storage(crate::storage::StorageError::TooManyAttachments)
            | Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::Storage(crate::storage::StorageError::Io(ref error))
                if error.kind() == std::io::ErrorKind::NotFound =>
            {
                StatusCode::NOT_FOUND
            }
            Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::Conflict => StatusCode::CONFLICT,
            Self::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Multipart => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        if matches!(self, Self::Storage(_)) {
            tracing::warn!(event = "storage_failure", status = status.as_u16());
        } else if status.is_server_error() {
            // Error sources can contain SQL, file paths and submitted values.
            tracing::error!(event = "application_error", status = status.as_u16());
        }
        if matches!(self, Self::Export) {
            return error_page(status, "导出失败，请稍后重试");
        }
        safe_error_page(status)
    }
}

impl From<axum::extract::rejection::FormRejection> for AppError {
    fn from(error: axum::extract::rejection::FormRejection) -> Self {
        if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
            Self::PayloadTooLarge
        } else {
            Self::BadRequest
        }
    }
}

impl From<axum::extract::multipart::MultipartError> for AppError {
    fn from(error: axum::extract::multipart::MultipartError) -> Self {
        tracing::warn!(event = "upload_failed", status = error.status().as_u16());
        if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
            Self::PayloadTooLarge
        } else {
            Self::Multipart
        }
    }
}

#[derive(Template)]
#[template(path = "errors/error.html")]
struct ErrorTemplate<'a> {
    status: u16,
    message: &'a str,
}

pub(crate) fn safe_error_page(status: StatusCode) -> Response {
    let message = match status.as_u16() {
        400 | 405 | 415 => "请求无效，请检查填写内容后重试",
        401 => "凭据不正确，请重试",
        403 => "无权访问，请重新验证身份",
        404 => "页面不存在或资源已不可用",
        409 => "记录已更新，请刷新页面后重试",
        413 => "请求内容过大，请减少附件数量或文件大小",
        422 => "表单校验失败，请检查填写内容",
        _ => "服务暂时不可用，请稍后重试",
    };
    error_page(status, message)
}

fn error_page(status: StatusCode, message: &str) -> Response {
    let html = ErrorTemplate {status: status.as_u16(), message}.render()
        .unwrap_or_else(|_| "<!doctype html><html lang=\"zh-CN\"><meta charset=\"utf-8\"><title>服务暂时不可用</title><p>服务暂时不可用，请稍后重试</p></html>".into());
    (
        status,
        [(axum::http::header::CACHE_CONTROL, "no-store")],
        Html(html),
    )
        .into_response()
}
