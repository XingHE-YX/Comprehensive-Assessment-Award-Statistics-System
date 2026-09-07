use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
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
            | Self::Storage(crate::storage::StorageError::TooManyAttachments) => {
                StatusCode::PAYLOAD_TOO_LARGE
            }
            Self::Storage(crate::storage::StorageError::Io(ref error))
                if error.kind() == std::io::ErrorKind::NotFound =>
            {
                StatusCode::NOT_FOUND
            }
            Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, self.to_string()).into_response()
    }
}
