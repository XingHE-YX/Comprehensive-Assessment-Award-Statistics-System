mod fields;
mod query;
mod student;
mod submission_form;
mod views;

use axum::{Router, extract::DefaultBodyLimit, routing::get};
use tower_http::services::ServeDir;

use crate::{auth, state::AppState};

pub fn build_router(state: AppState) -> Router {
    let session_layer =
        auth::session_layer(&[42_u8; 64], false).expect("static development session key is valid");
    Router::new()
        .route("/", get(student::home))
        .route("/access", axum::routing::post(student::access))
        .route(
            "/submit",
            get(student::submit_get).post(student::submit_post),
        )
        .route("/success/{submission_no}", get(student::success))
        .route("/query", get(query::query_get).post(query::query_post))
        .route("/query/{submission_no}", get(query::detail))
        .route(
            "/query/{submission_no}/update",
            axum::routing::post(query::update),
        )
        .route(
            "/submissions/{submission_no}/attachments/{id}",
            get(query::attachment),
        )
        .layer(axum::middleware::map_response(
            |mut response: axum::response::Response| async move {
                response.headers_mut().insert(
                    axum::http::header::CACHE_CONTROL,
                    axum::http::HeaderValue::from_static("no-store"),
                );
                response
            },
        ))
        .nest_service("/static", ServeDir::new("static"))
        .layer(DefaultBodyLimit::max(115_343_360))
        .layer(session_layer)
        .with_state(state)
}
