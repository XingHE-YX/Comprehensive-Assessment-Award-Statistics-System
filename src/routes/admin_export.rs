use std::collections::BTreeMap;

use axum::{
    extract::{Query, State, rejection::QueryRejection},
    http::{HeaderValue, header},
    response::{IntoResponse, Redirect, Response},
};

use crate::{error::AppError, services::export, state::AppState};

pub async fn download(
    State(state): State<AppState>,
    values: Result<Query<BTreeMap<String, String>>, QueryRejection>,
) -> Result<Response, AppError> {
    let Query(values) = values.map_err(|_| AppError::BadRequest)?;
    let Some(download) = export::prepare(&state.db, &values).await? else {
        return Ok(
            Redirect::to(&format!("/admin?{}", super::admin::encode_query(&values)))
                .into_response(),
        );
    };
    let disposition = HeaderValue::from_str(&format!(
        "attachment; filename=\"assessment-export.xlsx\"; filename*=UTF-8''{}",
        urlencoding::encode(&download.filename),
    ))
    .map_err(|_| AppError::Export)?;
    Ok((
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static(
                    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                ),
            ),
            (header::CONTENT_DISPOSITION, disposition),
            (
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ),
        ],
        download.bytes,
    )
        .into_response())
}
