mod student;
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
        .nest_service("/static", ServeDir::new("static"))
        .layer(DefaultBodyLimit::max(115_343_360))
        .layer(session_layer)
        .with_state(state)
}
