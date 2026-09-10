use std::collections::BTreeMap;

use axum::{
    Form,
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use chrono::Utc;
use tower_sessions::Session;

use crate::{
    auth::{
        establish_receipt_session, establish_student_session, generate_csrf_token, student_year_id,
        verify_csrf_token,
    },
    db::{
        AcademicYearRepo, AttachmentRepo, DeclarationRepo, NewSubmission, SettingsRepo,
        SubmissionRepo,
    },
    domain::SubmissionStatus,
    error::AppError,
    state::AppState,
    storage::UploadInput,
    validation::{ValidationErrors, validate_declaration, validate_submission},
};

use super::submission_form::{parse_multipart, submission_input};
use super::views;

pub async fn home(State(state): State<AppState>, session: Session) -> Result<Response, AppError> {
    let year = AcademicYearRepo::current(&state.db).await?;
    let csrf_token = generate_csrf_token(&session).await?;
    let class_name = SettingsRepo::get(&state.db, "class_name")
        .await?
        .unwrap_or_else(|| "综测成果申报".to_owned());
    let site_title = SettingsRepo::get(&state.db, "site_title")
        .await?
        .unwrap_or_else(|| "综测成果申报".to_owned());
    let html = views::home(year, csrf_token, class_name, site_title, None)
        .map_err(|_| AppError::Template)?;
    Ok(Html(html).into_response())
}

pub async fn access(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<BTreeMap<String, String>>,
) -> Result<Response, AppError> {
    let csrf = form
        .get("csrf_token")
        .map(String::as_str)
        .unwrap_or_default();
    if !verify_csrf_token(&session, csrf).await? {
        return Err(AppError::BadRequest);
    }
    let year = AcademicYearRepo::current(&state.db).await?;
    let Some(year) = year else {
        return Ok(Redirect::to("/").into_response());
    };
    let code = form
        .get("access_code")
        .map(String::as_str)
        .unwrap_or_default();
    let hash = SettingsRepo::get(&state.db, "class_access_code_hash").await?;
    let valid = hash
        .as_deref()
        .is_some_and(|value| crate::auth::verify_secret(value, code));
    if !valid {
        tracing::info!(event = "class_access_failed", status = 400);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let csrf_token = generate_csrf_token(&session).await?;
        let class_name = SettingsRepo::get(&state.db, "class_name")
            .await?
            .unwrap_or_else(|| "综测成果申报".to_owned());
        let site_title = SettingsRepo::get(&state.db, "site_title")
            .await?
            .unwrap_or_else(|| "综测成果申报".to_owned());
        let html = views::home(
            Some(year),
            csrf_token,
            class_name,
            site_title,
            Some("班级口令不正确，请重试。".to_owned()),
        )
        .map_err(|_| AppError::Template)?;
        return Ok((StatusCode::BAD_REQUEST, Html(html)).into_response());
    }
    establish_student_session(&session, year.id).await?;
    Ok(Redirect::to("/submit").into_response())
}

pub async fn submit_get(
    State(state): State<AppState>,
    session: Session,
) -> Result<Response, AppError> {
    let Some(year_id) = student_year_id(&session).await? else {
        return Ok(Redirect::to("/").into_response());
    };
    let Some(year) = AcademicYearRepo::current(&state.db).await? else {
        return Ok(Redirect::to("/").into_response());
    };
    if year.id != year_id {
        return Ok(Redirect::to("/").into_response());
    }
    let csrf_token = generate_csrf_token(&session).await?;
    let html = views::submit(year, csrf_token, BTreeMap::new(), ValidationErrors::new())
        .map_err(|_| AppError::Template)?;
    Ok(Html(html).into_response())
}

pub async fn submit_post(
    State(state): State<AppState>,
    axum::Extension(vault): axum::Extension<std::sync::Arc<crate::auth::EditCodeVault>>,
    session: Session,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    let Some(year_id) = student_year_id(&session).await? else {
        return Ok(Redirect::to("/").into_response());
    };
    let Some(year) = AcademicYearRepo::current(&state.db).await? else {
        return Ok(Redirect::to("/").into_response());
    };
    if year.id != year_id {
        return Ok(Redirect::to("/").into_response());
    }

    let (values, uploads, csrf_token): (BTreeMap<String, String>, Vec<UploadInput>, String) =
        parse_multipart(&mut multipart).await?;
    if !verify_csrf_token(&session, &csrf_token).await? {
        return Err(AppError::BadRequest);
    }

    // A request may spend time uploading while the administrator changes settings.
    // Re-read under the write lock; both results and declarations use this snapshot.
    let mut transaction = state.db.begin_with("BEGIN IMMEDIATE").await?;
    let Some(year) = AcademicYearRepo::current_in_transaction(&mut transaction).await? else {
        return Ok(Redirect::to("/").into_response());
    };
    if year.id != year_id {
        return Ok(Redirect::to("/").into_response());
    }

    let has_result = values
        .get("has_result")
        .map(String::as_str)
        .unwrap_or("yes");
    if has_result == "no" {
        let mut errors = match validate_declaration(
            values
                .get("student_name")
                .map(String::as_str)
                .unwrap_or_default(),
            values
                .get("student_no")
                .map(String::as_str)
                .unwrap_or_default(),
        ) {
            Ok(()) => ValidationErrors::new(),
            Err(errors) => errors,
        };
        if let Err(deadline_errors) = crate::validation::validate_deadline(&year, Utc::now()) {
            errors.extend(deadline_errors);
        }
        if values.get("no_result_confirm").map(String::as_str) != Some("yes") {
            errors.add("no_result_confirm", "请确认本学年暂无成果材料");
        }
        if !errors.is_empty() {
            return render_submit_error(&session, year, values, errors).await;
        }
        DeclarationRepo::upsert_in_transaction(
            &mut transaction,
            year.id,
            values
                .get("student_name")
                .map(String::as_str)
                .unwrap_or_default()
                .trim(),
            values
                .get("student_no")
                .map(String::as_str)
                .unwrap_or_default()
                .trim(),
        )
        .await?;
        transaction.commit().await?;
        return Ok(Redirect::to("/success/declaration").into_response());
    }

    let submission_input = match submission_input(&values) {
        Ok(input) => input,
        Err(errors) => return render_submit_error(&session, year, values, errors).await,
    };
    let validated = match validate_submission(submission_input, &year, &uploads) {
        Ok(value) => value,
        Err(errors) => return render_submit_error(&session, year, values, errors).await,
    };
    let edit_code = crate::auth::generate_edit_code();
    let edit_code_hash = crate::auth::hash_secret(&edit_code).map_err(|_| AppError::BadRequest)?;

    let submission_no = SubmissionRepo::next_number(&mut transaction, &year).await?;
    let ciphertext = vault.encrypt(&submission_no, &edit_code);
    let year_directory = crate::storage::academic_year_directory(year.id);
    let stored = match state
        .storage
        .save_many(&year_directory, &submission_no, 0, uploads)
        .await
    {
        Ok(stored) => stored,
        Err(error) => {
            tracing::warn!(event = "upload_failed", status = "rejected");
            let _ = transaction.rollback().await;
            return Err(AppError::Storage(error));
        }
    };
    let insert = SubmissionRepo::insert_in_transaction(
        &mut transaction,
        &NewSubmission {
            submission_no: submission_no.clone(),
            academic_year_id: year.id,
            student_name: validated.student_name,
            student_no: validated.student_no,
            category: validated.category,
            result_name: validated.result_name,
            obtained_date: validated.obtained_date,
            detail: validated.detail,
            remark: validated.remark,
            category_data: validated.category_data,
            edit_code_hash,
        },
        &ciphertext,
    )
    .await;
    let submission_id = match insert {
        Ok(id) => id,
        Err(error) => {
            for item in &stored {
                let _ = state
                    .storage
                    .remove_for_submission(&year_directory, &submission_no, &item.stored_name)
                    .await;
            }
            return Err(AppError::Database(error));
        }
    };
    for item in &stored {
        if let Err(error) =
            AttachmentRepo::insert_stored(&mut transaction, submission_id, item).await
        {
            let _ = transaction.rollback().await;
            for saved in &stored {
                let _ = state
                    .storage
                    .remove_for_submission(&year_directory, &submission_no, &saved.stored_name)
                    .await;
            }
            return Err(AppError::Database(error));
        }
    }
    if let Err(error) = transaction.commit().await {
        for saved in &stored {
            let _ = state
                .storage
                .remove_for_submission(&year_directory, &submission_no, &saved.stored_name)
                .await;
        }
        return Err(AppError::Database(error));
    }
    establish_receipt_session(&session, &submission_no, &edit_code).await?;
    tracing::info!(submission_no = %submission_no, status = %SubmissionStatus::Pending, "student submission created");
    Ok(Redirect::to(&format!("/success/{submission_no}")).into_response())
}

pub async fn success(
    State(state): State<AppState>,
    session: Session,
    Path(submission_no): Path<String>,
) -> Result<Response, AppError> {
    if submission_no == "declaration" {
        let html = views::declaration().map_err(|_| AppError::Template)?;
        return Ok(Html(html).into_response());
    }
    let Some(receipt) = crate::auth::take_receipt(&session).await? else {
        return Err(AppError::NotFound);
    };
    if receipt.submission_no != submission_no {
        return Err(AppError::NotFound);
    }
    let Some(submission) = SubmissionRepo::find_by_no(&state.db, &submission_no).await? else {
        return Err(AppError::NotFound);
    };
    if receipt.version != submission.edit_code_version
        || !crate::auth::verify_secret(&submission.edit_code_hash, &receipt.edit_code)
    {
        return Err(AppError::NotFound);
    }
    let html = views::success(
        submission.submission_no,
        submission.result_name,
        receipt.edit_code,
    )
    .map_err(|_| AppError::Template)?;
    Ok(Html(html).into_response())
}

async fn render_submit_error(
    session: &Session,
    year: crate::domain::AcademicYear,
    values: BTreeMap<String, String>,
    errors: ValidationErrors,
) -> Result<Response, AppError> {
    let csrf_token = generate_csrf_token(session).await?;
    let html = views::submit(year, csrf_token, values, errors).map_err(|_| AppError::Template)?;
    Ok((StatusCode::UNPROCESSABLE_ENTITY, Html(html)).into_response())
}
