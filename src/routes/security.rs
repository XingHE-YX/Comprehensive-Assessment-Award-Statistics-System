use crate::{
    error::{AppError, safe_error_page},
    state::AppState,
};
use axum::{
    extract::{MatchedPath, Request, State},
    http::{HeaderValue, header},
    middleware::Next,
    response::Response,
};
use tracing::Instrument;

pub(super) async fn enforce_declared_limit(
    State(limit): State<usize>,
    request: Request,
    next: Next,
) -> Response {
    // Reject declared oversized bodies before authentication/extraction. Requests
    // without Content-Length remain bounded while streaming by DefaultBodyLimit.
    if request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|size| size > limit as u64)
    {
        return safe_error_page(axum::http::StatusCode::PAYLOAD_TOO_LARGE);
    }
    next.run(request).await
}

pub(super) async fn health(State(state): State<AppState>) -> Result<&'static str, AppError> {
    crate::db::readiness(&state.db).await?;
    Ok("ok")
}

pub(super) async fn observe(request: Request, next: Next) -> Response {
    // Ignore caller-supplied ids; matched templates omit secret path/query values.
    let request_id = uuid::Uuid::new_v4().to_string();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or("unmatched")
        .to_owned();
    let span = tracing::info_span!("request", request_id = %request_id, route = %route);
    async move {
        let mut response = next.run(request).await;
        let status = response.status();
        let is_html = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("text/html"));
        // Extractors/static services can return framework text before a handler.
        if (status.is_client_error() || status.is_server_error()) && !is_html {
            response = safe_error_page(status);
        }
        if let Ok(value) = HeaderValue::from_str(&request_id) {
            response.headers_mut().insert("x-request-id", value);
        }
        response.headers_mut().insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
        response.headers_mut().insert(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        );
        tracing::info!(event = "request_complete", status = status.as_u16());
        response
    }
    .instrument(span)
    .await
}
