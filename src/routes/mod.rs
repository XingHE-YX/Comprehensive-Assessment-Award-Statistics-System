mod admin;
mod admin_auth;
mod admin_export;
mod admin_settings;
mod admin_submissions;
mod fields;
mod query;
mod security;
mod student;
mod submission_form;
mod views;

use axum::{Router, extract::DefaultBodyLimit, routing::get};
use tower_http::services::ServeDir;

use crate::{auth, state::AppState};

/// Student-only development/test entry point; administrator login is disabled.
/// Production startup must use build_router_with_config with validated configuration.
pub fn build_router(state: AppState) -> Router {
    let session_layer =
        auth::session_layer(&[42_u8; 64], false).expect("static development session key is valid");
    router(
        state,
        session_layer,
        None,
        std::sync::Arc::new(
            auth::EditCodeVault::new(&[42_u8; 64]).expect("static development key is valid"),
        ),
        115_343_360,
    )
}

/// Configured session, administrator credentials and body limit for application startup.
pub fn build_router_with_config(
    state: AppState,
    config: &crate::config::Config,
) -> Result<Router, auth::AuthError> {
    Ok(router(
        state,
        auth::session_layer(&config.session_secret, config.cookie_secure)?,
        Some(std::sync::Arc::new(auth::AdminCredentials::new(
            config.admin_username.clone(),
            config.admin_password_hash.clone(),
        ))),
        std::sync::Arc::new(auth::EditCodeVault::new(&config.session_secret)?),
        config.max_body_bytes,
    ))
}

fn router(
    state: AppState,
    session_layer: tower_sessions::SessionManagerLayer<
        tower_sessions::MemoryStore,
        tower_sessions::service::PrivateCookie,
    >,
    credentials: Option<std::sync::Arc<auth::AdminCredentials>>,
    vault: std::sync::Arc<auth::EditCodeVault>,
    max_body_bytes: usize,
) -> Router {
    let protected = Router::new()
        .route("/admin", get(admin::dashboard))
        .route("/admin/export.xlsx", get(admin_export::download))
        .route("/admin/logout", axum::routing::post(admin_auth::logout))
        .route("/admin/submissions/{id}", get(admin_submissions::detail))
        .route(
            "/admin/submissions/{id}/edit-code/reset",
            axum::routing::post(admin_submissions::reset_edit_code),
        )
        .route(
            "/admin/submissions/{id}/review",
            axum::routing::post(admin_submissions::review),
        )
        .route("/admin/settings", get(admin_settings::page))
        .route("/admin/years", axum::routing::post(admin_settings::create))
        .route(
            "/admin/years/{id}",
            axum::routing::post(admin_settings::update),
        )
        .route(
            "/admin/years/{id}/activate",
            axum::routing::post(admin_settings::activate),
        )
        .route(
            "/admin/settings/class-code",
            axum::routing::post(admin_settings::class_code),
        )
        .route_layer(axum::middleware::from_fn(admin_auth::require_admin));
    Router::new()
        .merge(protected)
        .route(
            "/admin/login",
            get(admin_auth::login_get).post(admin_auth::login_post),
        )
        .route("/", get(student::home))
        .route("/healthz", get(security::health))
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
        .fallback(|| async { crate::error::AppError::NotFound })
        .layer(DefaultBodyLimit::max(max_body_bytes))
        .layer(axum::Extension(credentials))
        .layer(axum::Extension(vault))
        .layer(session_layer)
        .layer(axum::middleware::from_fn_with_state(
            max_body_bytes,
            security::enforce_declared_limit,
        ))
        .layer(axum::middleware::from_fn(security::observe))
        .with_state(state)
}
