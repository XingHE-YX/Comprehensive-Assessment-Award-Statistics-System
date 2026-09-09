use std::{collections::BTreeMap, str::FromStr};

use axum::{
    Form,
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use chrono::{NaiveDate, Utc};
use serde_json::{Map, Value};
use tower_sessions::Session;

use crate::{
    auth::{
        establish_receipt_session, establish_student_session, generate_csrf_token,
        receipt_edit_code, receipt_submission_no, student_year_id, verify_csrf_token,
    },
    db::{AcademicYearRepo, DeclarationRepo, SettingsRepo, SubmissionRepo},
    domain::{Category, SubmissionStatus},
    error::AppError,
    state::AppState,
    storage::UploadInput,
    validation::{SubmissionInput, ValidationErrors, validate_declaration, validate_submission},
};

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
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
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
        if values.get("no_result_confirm").map(String::as_str) != Some("yes") {
            errors.add("no_result_confirm", "请确认本学年暂无成果材料");
        }
        if !errors.is_empty() {
            return render_submit_error(&session, year, values, errors).await;
        }
        DeclarationRepo::upsert(
            &state.db,
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
        return Ok(Redirect::to("/success/declaration").into_response());
    }

    let category = match values
        .get("category")
        .and_then(|value| Category::from_str(value).ok())
    {
        Some(category) => category,
        None => {
            let mut errors = ValidationErrors::new();
            errors.add("category", "成果类别为必填项");
            return render_submit_error(&session, year, values, errors).await;
        }
    };
    let obtained_date = match values
        .get("obtained_date")
        .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
    {
        Some(value) => value,
        None => {
            let mut errors = ValidationErrors::new();
            errors.add("obtained_date", "取得日期格式无效");
            return render_submit_error(&session, year, values, errors).await;
        }
    };
    let category_data = values
        .iter()
        .filter(|(key, _)| {
            !matches!(
                key.as_str(),
                "csrf_token"
                    | "has_result"
                    | "student_name"
                    | "student_no"
                    | "result_name"
                    | "obtained_date"
                    | "category"
                    | "detail"
                    | "remark"
                    | "no_result_confirm"
            )
        })
        .map(|(key, value)| (key.clone(), Value::String(value.clone())))
        .collect::<Map<String, Value>>();
    let submission_input = SubmissionInput {
        student_name: values.get("student_name").cloned().unwrap_or_default(),
        student_no: values.get("student_no").cloned().unwrap_or_default(),
        result_name: values.get("result_name").cloned().unwrap_or_default(),
        obtained_date,
        detail: optional_value(&values, "detail"),
        remark: optional_value(&values, "remark"),
        category,
        category_data: Value::Object(category_data),
    };
    let validated = match validate_submission(submission_input, &year, &uploads) {
        Ok(value) => value,
        Err(errors) => return render_submit_error(&session, year, values, errors).await,
    };
    let edit_code = crate::auth::generate_edit_code();
    let edit_code_hash = crate::auth::hash_secret(&edit_code).map_err(|_| AppError::BadRequest)?;

    let mut transaction = state.db.begin().await?;
    let sequence: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(CAST(substr(submission_no, 8) AS INTEGER)), 0) + 1 FROM submissions WHERE academic_year_id = ?",
    )
    .bind(year.id)
    .fetch_one(&mut *transaction)
    .await?;
    let submission_no = crate::auth::generate_submission_no(&year, sequence as u64);
    let stored = match state
        .storage
        .save_many(&year.name, &submission_no, 0, uploads)
        .await
    {
        Ok(stored) => stored,
        Err(error) => {
            let _ = transaction.rollback().await;
            return Err(AppError::Storage(error));
        }
    };
    let now = Utc::now();
    let category_json =
        serde_json::to_string(&validated.category_data).map_err(|_| AppError::BadRequest)?;
    let insert = sqlx::query(
        "INSERT INTO submissions (submission_no, academic_year_id, student_name, student_no, category, result_name, obtained_date, detail, remark, category_data, edit_code_hash, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&submission_no)
    .bind(year.id)
    .bind(&validated.student_name)
    .bind(&validated.student_no)
    .bind(validated.category.as_str())
    .bind(&validated.result_name)
    .bind(validated.obtained_date)
    .bind(&validated.detail)
    .bind(&validated.remark)
    .bind(category_json)
    .bind(edit_code_hash)
    .bind(now)
    .bind(now)
    .execute(&mut *transaction)
    .await;
    let submission_id = match insert {
        Ok(result) => result.last_insert_rowid(),
        Err(error) => {
            for item in &stored {
                let _ = state
                    .storage
                    .remove_for_submission(&year.name, &submission_no, &item.stored_name)
                    .await;
            }
            return Err(AppError::Database(error));
        }
    };
    for item in &stored {
        if let Err(error) = sqlx::query(
            "INSERT INTO attachments (submission_id, original_name, stored_name, mime_type, file_size, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(submission_id)
        .bind(&item.original_name)
        .bind(&item.stored_name)
        .bind(&item.mime_type)
        .bind(item.file_size)
        .bind(now)
        .execute(&mut *transaction)
        .await
        {
            let _ = transaction.rollback().await;
            for saved in &stored {
                let _ = state.storage.remove_for_submission(&year.name, &submission_no, &saved.stored_name).await;
            }
            return Err(AppError::Database(error));
        }
    }
    if let Err(error) = transaction.commit().await {
        for saved in &stored {
            let _ = state
                .storage
                .remove_for_submission(&year.name, &submission_no, &saved.stored_name)
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
    let Some(receipt_no) = receipt_submission_no(&session).await? else {
        return Err(AppError::NotFound);
    };
    if receipt_no != submission_no {
        return Err(AppError::NotFound);
    }
    let Some(submission) = SubmissionRepo::find_by_no(&state.db, &submission_no).await? else {
        return Err(AppError::NotFound);
    };
    let Some(edit_code) = receipt_edit_code(&session).await? else {
        return Err(AppError::NotFound);
    };
    let html = views::success(submission.submission_no, submission.result_name, edit_code)
        .map_err(|_| AppError::Template)?;
    session
        .remove::<String>(crate::auth::RECEIPT_SUBMISSION_NO_KEY)
        .await
        .map_err(crate::auth::AuthError::Session)?;
    session
        .remove::<String>(crate::auth::RECEIPT_EDIT_CODE_KEY)
        .await
        .map_err(crate::auth::AuthError::Session)?;
    session
        .remove::<chrono::DateTime<Utc>>(crate::auth::RECEIPT_EXPIRES_AT_KEY)
        .await
        .map_err(crate::auth::AuthError::Session)?;
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

async fn parse_multipart(
    multipart: &mut Multipart,
) -> Result<(BTreeMap<String, String>, Vec<UploadInput>, String), AppError> {
    let mut values = BTreeMap::new();
    let mut uploads = Vec::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::Multipart)?
    {
        let name = field.name().unwrap_or_default().to_owned();
        if field.file_name().is_some() {
            let original_name = field.file_name().unwrap_or_default().to_owned();
            let mime_type = field
                .content_type()
                .unwrap_or("application/octet-stream")
                .to_owned();
            let bytes = field
                .bytes()
                .await
                .map_err(|_| AppError::Multipart)?
                .to_vec();
            uploads.push(UploadInput::new(original_name, mime_type, bytes));
        } else {
            values.insert(name, field.text().await.map_err(|_| AppError::Multipart)?);
        }
    }
    let csrf_token = values.get("csrf_token").cloned().unwrap_or_default();
    Ok((values, uploads, csrf_token))
}

fn optional_value(values: &BTreeMap<String, String>, key: &str) -> Option<String> {
    values
        .get(key)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
