use axum_test::{TestServer, multipart::MultipartForm};
use serde_json::json;
use zongce_web::{auth, config::Config, db::AcademicYearRepo, state::AppState};

fn csrf(html: &str) -> String {
    html.split("name=\"csrf_token\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .into()
}

async fn fixture() -> (TestServer, AppState, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut state = AppState::initialize("sqlite::memory:").await.unwrap();
    state.storage = zongce_web::storage::AttachmentStorage::new(dir.path());
    let password = auth::generate_edit_code();
    let config = Config {
        app_env: "development".into(),
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        admin_username: "test-admin".into(),
        admin_password_hash: auth::hash_secret(&password).unwrap(),
        session_secret: uuid::Uuid::new_v4().as_bytes().repeat(4),
        database_url: "sqlite::memory:".into(),
        upload_dir: std::env::temp_dir(),
        cookie_secure: false,
        max_body_bytes: 115_343_360,
    };
    let mut server = TestServer::new(
        zongce_web::routes::build_router_with_config(state.clone(), &config).unwrap(),
    )
    .unwrap();
    server.save_cookies();
    let token = csrf(&server.get("/admin/login").await.text());
    server
        .post("/admin/login")
        .form(&json!({"csrf_token":token,"username":"test-admin","password":password}))
        .await
        .assert_status_see_other();
    let token = csrf(&server.get("/admin/settings").await.text());
    (server, state, token, dir)
}

#[tokio::test]
async fn creation_requires_prepared_roster_and_empty_startup_stays_empty() {
    let (server, state, token, _dir) = fixture().await;
    assert!(AcademicYearRepo::list(&state.db).await.unwrap().is_empty());
    server.post("/admin/years").form(&json!({"csrf_token":token,"name":"2026-2027本地测试","start_date":"2026-08-31","end_date":"2027-08-30"})).await.assert_status_unprocessable_entity();
    let response = server
        .post("/admin/roster/prepare")
        .multipart(
            MultipartForm::new()
                .add_text("csrf_token", token.clone())
                .add_text(
                    "roster_text",
                    "姓名\t学号\n测试学生\t000001\n测试学生\t000001",
                ),
        )
        .await;
    response.assert_status_see_other();
    let html = server.get("/admin/settings").await.text();
    let draft = html
        .split("name=\"roster_draft_id\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    server.post("/admin/years").form(&json!({"csrf_token":token,"name":"2026-2027本地测试","start_date":"2026-08-31","end_date":"2027-08-30","roster_draft_id":draft})).await.assert_status_see_other();
    let years = AcademicYearRepo::list(&state.db).await.unwrap();
    assert_eq!(years.len(), 1);
    assert!(!years[0].is_active);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM academic_year_students")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

async fn create_year(server: &TestServer, state: &AppState, token: &str, name: &str) -> i64 {
    server
        .post("/admin/roster/prepare")
        .multipart(
            MultipartForm::new()
                .add_text("csrf_token", token)
                .add_text("roster_text", "姓名,学号\n测试学生,000001"),
        )
        .await
        .assert_status_see_other();
    let html = server.get("/admin/settings").await.text();
    let draft = html
        .split("name=\"roster_draft_id\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    server.post("/admin/years").form(&json!({"csrf_token":token,"name":name,"start_date":"2026-08-31","end_date":"2027-08-30","roster_draft_id":draft})).await.assert_status_see_other();
    AcademicYearRepo::list(&state.db)
        .await
        .unwrap()
        .into_iter()
        .find(|year| year.name == name)
        .unwrap()
        .id
}

async fn activate(server: &TestServer, state: &AppState, token: &str, id: i64) {
    server
        .post(&format!("/admin/years/{id}/activate"))
        .form(&json!({"csrf_token":token}))
        .await
        .assert_status_see_other();
    let code = auth::generate_edit_code();
    zongce_web::db::SettingsRepo::set(
        &state.db,
        "class_access_code_hash",
        &auth::hash_secret(&code).unwrap(),
    )
    .await
    .unwrap();
    server
        .post("/access")
        .form(&json!({"csrf_token":token,"access_code":code}))
        .await
        .assert_status_see_other();
}

fn declaration(token: &str, name: &str, number: &str) -> MultipartForm {
    MultipartForm::new()
        .add_text("csrf_token", token)
        .add_text("student_name", name)
        .add_text("student_no", number)
        .add_text("has_result", "no")
        .add_text("no_result_confirm", "yes")
}

fn result_form(token: &str, name: &str, number: &str) -> MultipartForm {
    MultipartForm::new()
        .add_text("csrf_token", token)
        .add_text("student_name", name)
        .add_text("student_no", number)
        .add_text("has_result", "yes")
        .add_text("result_name", "回收测试成果")
        .add_text("obtained_date", "2026-09-01")
        .add_text("category", "academic_competition")
        .add_text("competition_name", "测试竞赛")
        .add_text("competition_type", "A")
        .add_text("level", "国家")
        .add_text("award_level", "一等奖")
        .add_part(
            "attachments",
            axum_test::multipart::Part::bytes(b"%PDF-1.4 proof".to_vec())
                .file_name("proof.pdf")
                .mime_type("application/pdf"),
        )
}

#[tokio::test]
async fn roster_checks_both_branches_updates_year_scope_and_atomic_append() {
    let (server, state, token, _dir) = fixture().await;
    let first = create_year(&server, &state, &token, "第一学年").await;
    activate(&server, &state, &token, first).await;
    for (name, number) in [
        ("错误姓名", "000001"),
        ("测试学生", "1"),
        ("测试学生", "unknown"),
    ] {
        server
            .post("/submit")
            .multipart(declaration(&token, name, number))
            .await
            .assert_status_unprocessable_entity();
        server
            .post("/submit")
            .multipart(result_form(&token, name, number))
            .await
            .assert_status_unprocessable_entity();
    }
    server
        .post("/submit")
        .multipart(declaration(&token, "测试学生", "000001"))
        .await
        .assert_status_see_other();
    let append = |text: &str| {
        MultipartForm::new()
            .add_text("csrf_token", token.clone())
            .add_text("roster_text", text.to_owned())
    };
    server
        .post(&format!("/admin/years/{first}/students"))
        .multipart(append("姓名,学号\n补录学生,000000\n冲突姓名,000001"))
        .await
        .assert_status_unprocessable_entity();
    assert_eq!(
        zongce_web::db::RosterRepo::list(&state.db, first)
            .await
            .unwrap()
            .len(),
        1
    );
    server
        .post(&format!("/admin/years/{first}/students"))
        .multipart(append("姓名,学号\n补录学生,000000\n测试学生,000001"))
        .await
        .assert_status_see_other();
    server
        .post("/submit")
        .multipart(declaration(&token, "补录学生", "000000"))
        .await
        .assert_status_see_other();
    let response = server
        .post("/submit")
        .multipart(result_form(&token, "测试学生", "000001"))
        .await;
    response.assert_status_see_other();
    let receipt = server
        .get(response.headers()["location"].to_str().unwrap())
        .await
        .text();
    let code = receipt
        .split("id=\"edit-code\">")
        .nth(1)
        .unwrap()
        .split('<')
        .next()
        .unwrap();
    let submission = zongce_web::db::SubmissionRepo::list(&state.db, &Default::default())
        .await
        .unwrap()
        .remove(0);
    server
        .post("/query")
        .form(
            &json!({"csrf_token":token,"submission_no":submission.submission_no,"edit_code":code}),
        )
        .await
        .assert_status_see_other();
    server
        .post(&format!("/query/{}/update", submission.submission_no))
        .multipart(result_form(&token, "冒用姓名", "000001"))
        .await
        .assert_status_unprocessable_entity();
    let second = create_year(&server, &state, &token, "第二学年").await;
    activate(&server, &state, &token, second).await;
    server
        .post("/submit")
        .multipart(declaration(&token, "补录学生", "000000"))
        .await
        .assert_status_unprocessable_entity();
    assert_eq!(
        zongce_web::db::SubmissionRepo::find_by_id(&state.db, submission.id)
            .await
            .unwrap()
            .unwrap()
            .student_name,
        "测试学生"
    );
}

#[tokio::test]
async fn all_statuses_and_declarations_recycle_atomically_excluding_scores_files_and_export() {
    use calamine::Reader;
    let (server, state, token, _dir) = fixture().await;
    let year = create_year(&server, &state, &token, "回收测试学年").await;
    activate(&server, &state, &token, year).await;
    for status in ["pending", "approved", "needs_revision", "rejected"] {
        server
            .post("/submit")
            .multipart(result_form(&token, "测试学生", "000001"))
            .await
            .assert_status_see_other();
        let submission = zongce_web::db::SubmissionRepo::list(&state.db, &Default::default())
            .await
            .unwrap()
            .remove(0);
        server
            .post(&format!("/admin/submissions/{}/review", submission.id))
            .form(&json!({"csrf_token":token,"status":status,"approved_score":"2.50"}))
            .await
            .assert_status_see_other();
    }
    server
        .post("/submit")
        .multipart(declaration(&token, "测试学生", "000001"))
        .await
        .assert_status_see_other();
    let submissions = zongce_web::db::SubmissionRepo::list(&state.db, &Default::default())
        .await
        .unwrap();
    let declaration_id: i64 = sqlx::query_scalar("SELECT id FROM student_declarations")
        .fetch_one(&state.db)
        .await
        .unwrap();
    let first = &submissions[0];
    let attachment = zongce_web::db::AttachmentRepo::list_for_submission(&state.db, first.id)
        .await
        .unwrap()
        .remove(0);
    let file_url = format!(
        "/submissions/{}/attachments/{}",
        first.submission_no, attachment.id
    );
    server.get(&file_url).await.assert_status_ok();
    let mut selected = std::collections::BTreeMap::from([
        ("csrf_token".to_owned(), token.clone()),
        ("confirm_delete".to_owned(), "yes".to_owned()),
    ]);
    for submission in &submissions {
        selected.insert(format!("selected:s:{}", submission.id), "yes".into());
    }
    selected.insert(format!("selected:d:{declaration_id}"), "yes".into());
    let mut invalid = selected.clone();
    invalid.insert("selected:s:999999".into(), "yes".into());
    server
        .post("/admin/records/delete")
        .form(&invalid)
        .await
        .assert_status_conflict();
    assert_eq!(
        zongce_web::db::SubmissionRepo::list(&state.db, &Default::default())
            .await
            .unwrap()
            .len(),
        4
    );
    server
        .post("/admin/records/delete/confirm")
        .form(&selected)
        .await
        .assert_status_ok();
    server
        .post("/admin/records/delete")
        .form(&selected)
        .await
        .assert_status_see_other();
    let data = zongce_web::services::admin::DashboardData::load(&state.db, &Default::default())
        .await
        .unwrap();
    assert!(data.rows.is_empty());
    assert_eq!(data.approved_total, "0.00");
    server.get(&file_url).await.assert_status_not_found();
    server
        .get(&format!("/admin/submissions/{}", first.id))
        .await
        .assert_status_not_found();
    assert!(state.storage.open(&attachment.stored_name).await.is_ok());
    let export = server.get("/admin/export.xlsx").await;
    export.assert_status_ok();
    let mut workbook: calamine::Xlsx<_> =
        calamine::Xlsx::new(std::io::Cursor::new(export.as_bytes())).unwrap();
    assert_eq!(workbook.worksheet_range("申报明细").unwrap().height(), 1);
    assert_eq!(workbook.worksheet_range("学生汇总").unwrap().height(), 1);
    server
        .post("/admin/records/restore")
        .form(&selected)
        .await
        .assert_status_see_other();
    let data = zongce_web::services::admin::DashboardData::load(&state.db, &Default::default())
        .await
        .unwrap();
    assert_eq!(data.rows.len(), 5);
    assert_eq!(data.approved_total, "2.50");
    server.get(&file_url).await.assert_status_ok();
}

#[tokio::test]
async fn delete_only_selected_year_keeps_other_year_and_allows_same_name_recreation() {
    let (server, state, token, _dir) = fixture().await;
    let retained_id = create_year(&server, &state, &token, "2025-2026待激活").await;
    let retained = AcademicYearRepo::find_by_id(&state.db, retained_id)
        .await
        .unwrap()
        .unwrap();
    let old = create_year(&server, &state, &token, "2026-2027本地测试").await;
    activate(&server, &state, &token, old).await;
    server
        .post("/submit")
        .multipart(result_form(&token, "测试学生", "000001"))
        .await
        .assert_status_see_other();
    server
        .post("/submit")
        .multipart(declaration(&token, "测试学生", "000001"))
        .await
        .assert_status_see_other();
    let submission = zongce_web::db::SubmissionRepo::list(&state.db, &Default::default())
        .await
        .unwrap()
        .remove(0);
    let attachment = zongce_web::db::AttachmentRepo::list_for_submission(&state.db, submission.id)
        .await
        .unwrap()
        .remove(0);
    let url = format!("/admin/years/{old}/delete");
    server.get(&url).await.assert_status_ok();
    server
        .post(&url)
        .form(&json!({"csrf_token":token,"confirm_delete":"yes","confirm_name":"错误学年"}))
        .await
        .assert_status_conflict();
    assert!(
        AcademicYearRepo::find_by_id(&state.db, old)
            .await
            .unwrap()
            .is_some()
    );
    server
        .post(&url)
        .form(
            &json!({"csrf_token":token,"confirm_delete":"yes","confirm_name":"2026-2027本地测试"}),
        )
        .await
        .assert_status_see_other();
    assert!(
        AcademicYearRepo::current(&state.db)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        AcademicYearRepo::find_by_id(&state.db, retained_id)
            .await
            .unwrap()
            .unwrap(),
        retained
    );
    assert!(state.storage.open(&attachment.stored_name).await.is_err());
    let id = create_year(&server, &state, &token, "2026-2027本地测试").await;
    assert!(id > old);
    assert!(
        zongce_web::db::SubmissionRepo::list(&state.db, &Default::default())
            .await
            .unwrap()
            .is_empty()
    );
    let violations = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&state.db)
        .await
        .unwrap();
    assert!(violations.is_empty());
}

#[tokio::test]
async fn new_admin_routes_require_authentication_csrf_and_explicit_delete_confirmation() {
    let (server, state, token, _dir) = fixture().await;
    let anonymous = TestServer::new(zongce_web::routes::build_router(state.clone())).unwrap();
    for url in [
        "/admin/roster/prepare",
        "/admin/years/1/students",
        "/admin/years/1/delete",
        "/admin/records/delete",
        "/admin/records/delete/confirm",
        "/admin/records/restore",
    ] {
        anonymous.post(url).await.assert_status_forbidden();
        server
            .post(url)
            .form(&json!({"csrf_token":"invalid"}))
            .await
            .assert_status_bad_request();
    }
    anonymous
        .get("/admin/roster/template.xlsx")
        .await
        .assert_status_see_other();
    server
        .get("/admin/roster/template.xlsx")
        .await
        .assert_status_ok();
    let id = create_year(&server, &state, &token, "确认测试").await;
    server
        .post(&format!("/admin/years/{id}/delete"))
        .form(&json!({"csrf_token":token,"confirm_name":"确认测试"}))
        .await
        .assert_status_bad_request();
    assert!(
        AcademicYearRepo::find_by_id(&state.db, id)
            .await
            .unwrap()
            .is_some()
    );
}

#[test]
fn roster_template_parser_preserves_text_ids_and_rejects_ambiguous_input() {
    use zongce_web::services::roster;
    let parsed = roster::parse(
        Some("名单.csv"),
        "\u{feff}学号,姓名\n00001,测试学生\n00001,测试学生".as_bytes(),
    )
    .unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].student_no, "00001");
    for text in [
        "姓名,学号",
        "姓名,学号\n甲,0001\n乙,0001",
        "姓名,学号\n甲,",
        "没有表头",
    ] {
        assert!(roster::parse(None, text.as_bytes()).is_err());
    }
    let mut workbook = rust_xlsxwriter::Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet
        .write_string(0, 0, "姓名")
        .unwrap()
        .write_string(0, 1, "学号")
        .unwrap();
    sheet
        .write_string(1, 0, "测试学生")
        .unwrap()
        .write_string(1, 1, "000001")
        .unwrap();
    assert_eq!(
        roster::parse(Some("名单.xlsx"), &workbook.save_to_buffer().unwrap()).unwrap()[0]
            .student_no,
        "000001"
    );
    workbook
        .worksheet_from_index(0)
        .unwrap()
        .write_number(1, 1, 123.0)
        .unwrap();
    assert!(roster::parse(Some("名单.xlsx"), &workbook.save_to_buffer().unwrap()).is_err());
}
