use axum_test::TestServer;
use chrono::NaiveDate;
use serde_json::json;
use sqlx::sqlite::SqlitePoolOptions;
use tempfile::tempdir;
use zongce_web::{
    auth::hash_secret,
    db::{AcademicYearRepo, NewAcademicYear, SettingsRepo, migrate},
    state::AppState,
    storage::AttachmentStorage,
};

async fn server() -> (TestServer, sqlx::SqlitePool, tempfile::TempDir) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("pool");
    migrate(&pool).await.expect("migrate");
    AcademicYearRepo::insert(
        &pool,
        &NewAcademicYear {
            name: "2025-2026学年".to_owned(),
            start_date: NaiveDate::from_ymd_opt(2025, 8, 31).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2026, 8, 28).unwrap(),
            deadline: None,
            is_active: true,
            announcement: Some("请提交清晰材料".to_owned()),
        },
    )
    .await
    .expect("year");
    SettingsRepo::set(
        &pool,
        "class_access_code_hash",
        &hash_secret("class-code").expect("hash"),
    )
    .await
    .expect("setting");
    let dir = tempdir().expect("tempdir");
    let app = zongce_web::routes::build_router(AppState {
        db: pool.clone(),
        storage: AttachmentStorage::new(dir.path()),
    });
    let mut server = TestServer::new(app).expect("server");
    server.save_cookies();
    (server, pool, dir)
}

fn hidden_csrf(html: &str) -> String {
    html.split("name=\"csrf_token\" value=\"")
        .nth(1)
        .and_then(|value| value.split('"').next())
        .expect("csrf token")
        .to_owned()
}

fn submission_no(html: &str) -> String {
    html.split("class=\"submission-number\">")
        .nth(1)
        .and_then(|value| value.split('<').next())
        .expect("submission number")
        .to_owned()
}

fn edit_code(html: &str) -> String {
    html.split("id=\"edit-code\">")
        .nth(1)
        .and_then(|value| value.split('<').next())
        .expect("edit code")
        .to_owned()
}

async fn create_submission(server: &TestServer, student_name: &str) -> (String, String) {
    let home = server.get("/").await;
    let home_html = home.text();
    let access_csrf = hidden_csrf(&home_html);
    server
        .post("/access")
        .form(&json!({"csrf_token": access_csrf, "access_code": "class-code"}))
        .await;
    let form = server.get("/submit").await;
    let form_html = form.text();
    let submit_csrf = hidden_csrf(&form_html);
    let response = server
        .post("/submit")
        .multipart(
            axum_test::multipart::MultipartForm::new()
                .add_text("csrf_token", submit_csrf)
                .add_text("has_result", "yes")
                .add_text("student_name", student_name)
                .add_text("student_no", "20250001")
                .add_text("result_name", "竞赛一等奖")
                .add_text("obtained_date", "2026-04-01")
                .add_text("category", "academic_competition")
                .add_text("competition_name", "全国大学生竞赛")
                .add_text("competition_type", "A")
                .add_text("level", "国家")
                .add_text("award_level", "一等奖")
                .add_part(
                    "attachments",
                    axum_test::multipart::Part::bytes(b"proof".to_vec())
                        .file_name("proof.pdf")
                        .mime_type("application/pdf"),
                ),
        )
        .await;
    assert_eq!(response.status_code(), 303);
    let location = response.headers()["location"].to_str().unwrap().to_owned();
    let success = server.get(&location).await;
    assert_eq!(success.status_code(), 200);
    (submission_no(&success.text()), edit_code(&success.text()))
}

async fn query_submission(server: &TestServer, submission_no: &str, edit_code: &str) -> u16 {
    let query = server.get("/query").await;
    assert_eq!(query.status_code(), 200);
    let csrf = hidden_csrf(&query.text());
    let response = server
        .post("/query")
        .form(&json!({
            "csrf_token": csrf,
            "submission_no": submission_no,
            "edit_code": edit_code,
        }))
        .await;
    response.status_code().as_u16()
}

fn edit_form(csrf: &str) -> axum_test::multipart::MultipartForm {
    axum_test::multipart::MultipartForm::new()
        .add_text("csrf_token", csrf)
        .add_text("student_name", "测试学生")
        .add_text("student_no", "TEST-001")
        .add_text("result_name", "补充后的成果")
        .add_text("obtained_date", "2026-04-02")
        .add_text("category", "academic_competition")
        .add_text("competition_name", "测试竞赛")
        .add_text("competition_type", "A")
        .add_text("level", "国家")
        .add_text("award_level", "一等奖")
}

fn proof(name: &str) -> axum_test::multipart::Part {
    axum_test::multipart::Part::bytes(b"additional proof".to_vec())
        .file_name(name.to_owned())
        .mime_type("application/pdf")
}

#[tokio::test]
async fn review_changes_after_loading_the_record_prevent_stale_student_writes() {
    use zongce_web::{
        db::SubmissionRepo,
        domain::SubmissionStatus,
        validation::{SubmissionInput, validate_student_update},
    };
    let (server, pool, dir) = server().await;
    let (number, _code) = create_submission(&server, "测试学生").await;
    let original = SubmissionRepo::find_by_no(&pool, &number)
        .await
        .unwrap()
        .unwrap();
    let year = AcademicYearRepo::find_by_id(&pool, original.academic_year_id)
        .await
        .unwrap()
        .unwrap();
    let input = validate_student_update(
        SubmissionInput {
            student_name: original.student_name.clone(),
            student_no: original.student_no.clone(),
            category: original.category,
            category_data: original.category_data.clone(),
            result_name: "不应保存的修改".to_owned(),
            obtained_date: original.obtained_date,
            detail: None,
            remark: None,
        },
        &year,
        &[],
        1,
    )
    .unwrap();
    SubmissionRepo::update_review(
        &pool,
        original.id,
        SubmissionStatus::Approved,
        Some("审核完成"),
        Some(3.5),
    )
    .await
    .unwrap();
    let state = AppState {
        db: pool.clone(),
        storage: AttachmentStorage::new(dir.path()),
    };
    let result =
        zongce_web::services::submissions::update_student(&state, &original, &year, &input, vec![])
            .await;
    assert!(matches!(
        result,
        Err(zongce_web::error::AppError::Forbidden)
    ));
    let unchanged = SubmissionRepo::find_by_no(&pool, &number)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unchanged.result_name, original.result_name);
    assert_eq!(unchanged.status, SubmissionStatus::Approved);
    assert_eq!(unchanged.approved_score, Some(3.5));
}

#[tokio::test]
async fn optional_sports_choice_can_remain_empty_when_editing() {
    let (server, _pool, _dir) = server().await;
    let (number, code) = create_submission(&server, "测试学生").await;
    assert_eq!(query_submission(&server, &number, &code).await, 303);
    let detail = server.get(&format!("/query/{number}")).await;
    let response = server
        .post(&format!("/query/{number}/update"))
        .multipart(
            edit_form(&hidden_csrf(&detail.text()))
                .add_text("category", "sports_arts_competition")
                .add_text("has_award_level", "yes")
                .add_text("is_seu_sports_meet", ""),
        )
        .await;
    assert_eq!(response.status_code(), 303, "{}", response.text());
}

#[tokio::test]
async fn wrong_edit_code_is_rejected_without_revealing_the_submission() {
    let (server, _pool, _dir) = server().await;
    let (submission_no, _edit_code) = create_submission(&server, "张三").await;
    let query = server.get("/query").await;
    assert_eq!(query.status_code(), 200);
    let response = server
        .post("/query")
        .form(&json!({
            "csrf_token": hidden_csrf(&query.text()),
            "submission_no": submission_no,
            "edit_code": "WRONGCODE",
        }))
        .await;
    assert_eq!(response.status_code(), 401);
    assert!(response.text().contains("申报编号或修改码不正确"));
}

#[tokio::test]
async fn valid_credentials_open_detail_and_session_cannot_cross_submissions() {
    let (server, _pool, _dir) = server().await;
    let (first_no, first_code) = create_submission(&server, "张三").await;
    let (second_no, _second_code) = create_submission(&server, "李四").await;
    assert_eq!(
        server
            .get(&format!("/query/{first_no}"))
            .await
            .status_code(),
        403
    );
    assert_eq!(query_submission(&server, &first_no, &first_code).await, 303);
    let first = server.get(&format!("/query/{first_no}")).await;
    assert_eq!(first.status_code(), 200);
    assert!(first.text().contains("待审核"));
    let mismatch = server.get(&format!("/query/{second_no}")).await;
    assert_eq!(mismatch.status_code(), 403);
    let denied_update = server
        .post(&format!("/query/{second_no}/update"))
        .multipart(edit_form(&hidden_csrf(&first.text())))
        .await;
    assert_eq!(denied_update.status_code(), 403);
}

#[tokio::test]
async fn pending_and_needs_revision_can_be_resubmitted_with_review_note_preserved() {
    let (server, pool, _dir) = server().await;
    let (submission_no, edit_code) = create_submission(&server, "张三").await;
    assert_eq!(
        query_submission(&server, &submission_no, &edit_code).await,
        303
    );
    let detail = server.get(&format!("/query/{submission_no}")).await;
    assert!(detail.text().contains("编辑申报"));

    sqlx::query(
        "UPDATE submissions SET status = 'needs_revision', review_note = '请补充清晰材料' WHERE submission_no = ?",
    )
    .bind(&submission_no)
    .execute(&pool)
    .await
    .unwrap();
    let detail = server.get(&format!("/query/{submission_no}")).await;
    let update_csrf = hidden_csrf(&detail.text());
    assert!(detail.text().contains("请补充清晰材料"));
    let response = server
        .post(&format!("/query/{submission_no}/update"))
        .multipart(
            axum_test::multipart::MultipartForm::new()
                .add_text("csrf_token", update_csrf)
                .add_text("student_name", "张三（已更新）")
                .add_text("student_no", "20250001")
                .add_text("result_name", "竞赛一等奖（补充材料）")
                .add_text("obtained_date", "2026-04-02")
                .add_text("category", "academic_competition")
                .add_text("competition_name", "全国大学生竞赛")
                .add_text("competition_type", "A")
                .add_text("level", "国家")
                .add_text("award_level", "一等奖")
                .add_part(
                    "attachments",
                    axum_test::multipart::Part::bytes(b"proof-2".to_vec())
                        .file_name("proof-2.pdf")
                        .mime_type("application/pdf"),
                ),
        )
        .await;
    assert_eq!(response.status_code(), 303);
    let updated = sqlx::query_as::<_, (String, String, i64, i64)>(
        "SELECT status, review_note, student_modified_after_review, (SELECT COUNT(*) FROM attachments WHERE submission_id = submissions.id) FROM submissions WHERE submission_no = ?",
    )
    .bind(&submission_no)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(updated.0, "pending");
    assert_eq!(updated.1, "请补充清晰材料");
    assert_eq!(updated.2, 1);
    assert_eq!(updated.3, 2);
}

#[tokio::test]
async fn approved_and_rejected_submissions_are_read_only() {
    for status in ["approved", "rejected"] {
        let (server, pool, _dir) = server().await;
        let (submission_no, edit_code) = create_submission(&server, "张三").await;
        sqlx::query("UPDATE submissions SET status = ? WHERE submission_no = ?")
            .bind(status)
            .bind(&submission_no)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            query_submission(&server, &submission_no, &edit_code).await,
            303
        );
        let detail = server.get(&format!("/query/{submission_no}")).await;
        assert!(!detail.text().contains("编辑申报"));
        let response = server
            .post(&format!("/query/{submission_no}/update"))
            .multipart(
                axum_test::multipart::MultipartForm::new()
                    .add_text("csrf_token", hidden_csrf(&detail.text())),
            )
            .await;
        assert_eq!(response.status_code(), 403, "status {status}");
    }
}

#[tokio::test]
async fn attachments_require_the_matching_verified_student_session() {
    let (server, pool, dir) = server().await;
    let (submission_no, edit_code) = create_submission(&server, "张三").await;
    let attachment_id: i64 = sqlx::query_scalar("SELECT id FROM attachments LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        query_submission(&server, &submission_no, &edit_code).await,
        303
    );
    let valid = server
        .get(&format!(
            "/submissions/{submission_no}/attachments/{attachment_id}"
        ))
        .await;
    assert_eq!(valid.status_code(), 200);
    assert_eq!(valid.as_bytes().as_ref(), b"proof");
    assert_eq!(valid.headers()["cache-control"], "no-store");
    assert_eq!(valid.headers()["content-type"], "application/pdf");
    assert_eq!(valid.headers()["x-content-type-options"], "nosniff");
    assert!(
        valid.headers()["content-disposition"]
            .to_str()
            .unwrap()
            .contains("filename*=UTF-8''proof.pdf")
    );

    let (second_no, second_code) = create_submission(&server, "另一位测试学生").await;
    assert_eq!(
        query_submission(&server, &second_no, &second_code).await,
        303
    );
    assert_eq!(
        server
            .get(&format!(
                "/submissions/{submission_no}/attachments/{attachment_id}"
            ))
            .await
            .status_code(),
        403
    );
    assert_eq!(
        server
            .get(&format!(
                "/submissions/{second_no}/attachments/{attachment_id}"
            ))
            .await
            .status_code(),
        404
    );

    let app = zongce_web::routes::build_router(AppState {
        db: pool,
        storage: AttachmentStorage::new(dir.path()),
    });
    let anonymous = TestServer::new(app).unwrap();
    let denied = anonymous
        .get(&format!(
            "/submissions/{submission_no}/attachments/{attachment_id}"
        ))
        .await;
    assert_eq!(denied.status_code(), 403);
}

#[tokio::test]
async fn pending_update_keeps_existing_files_and_uses_its_original_year_after_deadline() {
    let (server, pool, _dir) = server().await;
    let (number, code) = create_submission(&server, "测试学生").await;
    assert_eq!(
        query_submission(&server, &format!(" {number} "), &format!(" {code} ")).await,
        303
    );
    sqlx::query("UPDATE academic_years SET is_active = 0, deadline = '2000-01-01T00:00:00Z'")
        .execute(&pool)
        .await
        .unwrap();
    let original = zongce_web::db::SubmissionRepo::find_by_no(&pool, &number)
        .await
        .unwrap()
        .unwrap();
    let detail = server.get(&format!("/query/{number}")).await;
    let update = server
        .post(&format!("/query/{number}/update"))
        .multipart(edit_form(&hidden_csrf(&detail.text())).add_part(
            "attachments",
            axum_test::multipart::Part::bytes(Vec::new()).file_name(""),
        ))
        .await;
    assert_eq!(update.status_code(), 303, "{}", update.text());
    let updated = zongce_web::db::SubmissionRepo::find_by_no(&pool, &number)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.result_name, "补充后的成果");
    assert_eq!(updated.edit_code_hash, original.edit_code_hash);
    assert_eq!(updated.academic_year_id, original.academic_year_id);
    assert!(updated.updated_at >= original.updated_at);
    assert_eq!(
        zongce_web::db::AttachmentRepo::list_for_submission(&pool, updated.id)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn invalid_fields_and_csrf_cannot_modify_the_record() {
    let (server, pool, _dir) = server().await;
    let (number, code) = create_submission(&server, "测试学生").await;
    let bad_query = server
        .post("/query")
        .form(&json!({"submission_no": number, "edit_code": code}))
        .await;
    assert_eq!(bad_query.status_code(), 400);
    assert_eq!(query_submission(&server, &number, &code).await, 303);
    let detail = server.get(&format!("/query/{number}")).await;
    let csrf = hidden_csrf(&detail.text());
    let original = zongce_web::db::SubmissionRepo::find_by_no(&pool, &number)
        .await
        .unwrap()
        .unwrap();
    let bad_csrf = server
        .post(&format!("/query/{number}/update"))
        .multipart(edit_form("invalid"))
        .await;
    assert_eq!(bad_csrf.status_code(), 400);
    for (key, value) in [
        ("obtained_date", "2024-01-01"),
        ("student_name", ""),
        ("competition_name", ""),
        ("category", "invalid"),
    ] {
        let invalid = server
            .post(&format!("/query/{number}/update"))
            .multipart(edit_form(&csrf).add_text(key, value))
            .await;
        assert_eq!(invalid.status_code(), 422);
        assert!(invalid.text().contains("aria-invalid=\"true\""));
        assert!(invalid.text().contains("补充后的成果"));
    }
    let invalid_upload = server
        .post(&format!("/query/{number}/update"))
        .multipart(edit_form(&csrf).add_part("attachments", proof("invalid.exe")))
        .await;
    assert_eq!(invalid_upload.status_code(), 422);
    let unchanged = zongce_web::db::SubmissionRepo::find_by_no(&pool, &number)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unchanged, original);
}

#[tokio::test]
async fn updating_counts_old_and_new_attachments_together() {
    let (server, pool, _dir) = server().await;
    let (number, code) = create_submission(&server, "测试学生").await;
    assert_eq!(query_submission(&server, &number, &code).await, 303);
    let detail = server.get(&format!("/query/{number}")).await;
    let csrf = hidden_csrf(&detail.text());
    let mut form = edit_form(&csrf);
    for _ in 0..9 {
        form = form.add_part("attachments", proof("extra.pdf"));
    }
    assert_eq!(
        server
            .post(&format!("/query/{number}/update"))
            .multipart(form)
            .await
            .status_code(),
        303
    );
    let before = zongce_web::db::SubmissionRepo::find_by_no(&pool, &number)
        .await
        .unwrap()
        .unwrap();
    let overflow = server
        .post(&format!("/query/{number}/update"))
        .multipart(edit_form(&csrf).add_part("attachments", proof("eleventh.pdf")))
        .await;
    assert_eq!(overflow.status_code(), 422);
    assert_eq!(
        zongce_web::db::AttachmentRepo::list_for_submission(&pool, before.id)
            .await
            .unwrap()
            .len(),
        10
    );
    assert_eq!(
        zongce_web::db::SubmissionRepo::find_by_no(&pool, &number)
            .await
            .unwrap()
            .unwrap(),
        before
    );
}

#[tokio::test]
async fn attachment_insert_failure_rolls_back_fields_and_removes_new_files() {
    let (server, pool, dir) = server().await;
    let (number, code) = create_submission(&server, "测试学生").await;
    assert_eq!(query_submission(&server, &number, &code).await, 303);
    let detail = server.get(&format!("/query/{number}")).await;
    let before = zongce_web::db::SubmissionRepo::find_by_no(&pool, &number)
        .await
        .unwrap()
        .unwrap();
    sqlx::query("CREATE TRIGGER fail_attachment BEFORE INSERT ON attachments BEGIN SELECT RAISE(ABORT, 'test-only failure'); END;")
        .execute(&pool).await.unwrap();
    let failed = server
        .post(&format!("/query/{number}/update"))
        .multipart(
            edit_form(&hidden_csrf(&detail.text())).add_part("attachments", proof("extra.pdf")),
        )
        .await;
    assert_eq!(failed.status_code(), 500);
    assert!(!failed.text().contains("test-only failure"));
    assert_eq!(
        zongce_web::db::SubmissionRepo::find_by_no(&pool, &number)
            .await
            .unwrap()
            .unwrap(),
        before
    );
    assert_eq!(
        zongce_web::db::AttachmentRepo::list_for_submission(&pool, before.id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        std::fs::read_dir(dir.path().join("2025-2026学年").join(number))
            .unwrap()
            .count(),
        1
    );
}

#[tokio::test]
async fn nonexistent_number_and_wrong_code_share_the_same_error_and_do_not_echo_codes() {
    let (server, _pool, _dir) = server().await;
    let (number, code) = create_submission(&server, "测试学生").await;
    let query = server.get("/query").await;
    let csrf = hidden_csrf(&query.text());
    for (number, code) in [
        (number, "incorrect-test-code".to_owned()),
        ("ZC2026-999999".to_owned(), code),
    ] {
        let denied = server
            .post("/query")
            .form(&json!({"csrf_token": csrf, "submission_no": number, "edit_code": code}))
            .await;
        assert_eq!(denied.status_code(), 401);
        assert!(denied.text().contains("申报编号或修改码不正确"));
        assert!(!denied.text().contains(&code));
    }
}
