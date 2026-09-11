use axum_test::{
    TestServer,
    multipart::{MultipartForm, Part},
};
use serde_json::{Value, json};
#[path = "support/roster.rs"]
mod roster;
use zongce_web::{
    auth::{generate_edit_code, hash_secret, verify_secret},
    config::Config,
    db::{AcademicYearRepo, SettingsRepo, SubmissionFilter, SubmissionRepo},
    state::AppState,
    storage::AttachmentStorage,
};

struct Fixture {
    server: TestServer,
    student: TestServer,
    state: AppState,
    _dir: tempfile::TempDir,
    token: String,
    code: String,
    year_id: i64,
}

fn csrf(html: &str) -> String {
    html.split("name=\"csrf_token\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .into()
}

async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let mut state =
        AppState::initialize(&format!("sqlite:{}", dir.path().join("test.db").display()))
            .await
            .unwrap();
    state.storage = AttachmentStorage::new(dir.path().join("uploads"));
    roster::seed(
        &state.db,
        &[
            ("设置流程测试", "SETTINGS-TEST"),
            ("上传竞态测试", "RACE-TEST"),
        ],
    )
    .await;
    let password = generate_edit_code();
    let code = generate_edit_code();
    SettingsRepo::set(
        &state.db,
        "class_access_code_hash",
        &hash_secret(&code).unwrap(),
    )
    .await
    .unwrap();
    let config = Config {
        app_env: "development".into(),
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        admin_username: "test-admin".into(),
        admin_password_hash: hash_secret(&password).unwrap(),
        session_secret: uuid::Uuid::new_v4().as_bytes().repeat(4),
        database_url: "sqlite::memory:".into(),
        upload_dir: dir.path().into(),
        cookie_secure: false,
        max_body_bytes: 115_343_360,
    };
    let router = zongce_web::routes::build_router_with_config(state.clone(), &config).unwrap();
    let mut server = TestServer::new(router.clone()).unwrap();
    let mut student = TestServer::new_with_config(
        router,
        axum_test::TestServerConfig {
            transport: Some(axum_test::Transport::HttpRandomPort),
            ..Default::default()
        },
    )
    .unwrap();
    server.save_cookies();
    student.save_cookies();
    let token = csrf(&server.get("/admin/login").await.text());
    assert_eq!(
        server
            .post("/admin/login")
            .form(&json!({"csrf_token":token,"username":"test-admin","password":password}))
            .await
            .status_code(),
        303
    );
    let token = csrf(&server.get("/admin").await.text());
    let year_id = AcademicYearRepo::current(&state.db)
        .await
        .unwrap()
        .unwrap()
        .id;
    Fixture {
        server,
        student,
        state,
        _dir: dir,
        token,
        code,
        year_id,
    }
}

fn year_form(token: &str, name: &str) -> Value {
    json!({"csrf_token":token,"name":name,"start_date":"2025-08-31","end_date":"2026-08-28","deadline":"","announcement":"请提交清晰材料"})
}

async fn create(f: &Fixture, name: &str) -> i64 {
    f.server
        .post("/admin/roster/prepare")
        .multipart(
            MultipartForm::new()
                .add_text("csrf_token", f.token.clone())
                .add_text(
                    "roster_text",
                    "姓名,学号\n设置流程测试,SETTINGS-TEST\n上传竞态测试,RACE-TEST",
                ),
        )
        .await
        .assert_status_see_other();
    let html = f.server.get("/admin/settings").await.text();
    let draft = html
        .split("name=\"roster_draft_id\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let mut fields = year_form(&f.token, name);
    fields["roster_draft_id"] = json!(draft);
    let response = f.server.post("/admin/years").form(&fields).await;
    assert_eq!(response.status_code(), 303, "{}", response.text());
    AcademicYearRepo::list(&f.state.db)
        .await
        .unwrap()
        .into_iter()
        .find(|y| y.name == name)
        .unwrap()
        .id
}

async fn enter(f: &Fixture, code: &str) {
    let token = csrf(&f.student.get("/").await.text());
    assert_eq!(
        f.student
            .post("/access")
            .form(&json!({"csrf_token":token,"access_code":code}))
            .await
            .status_code(),
        303
    );
}

async fn submit(f: &Fixture, declaration: bool, date: &str) -> axum_test::TestResponse {
    let token = csrf(&f.student.get("/submit").await.text());
    let mut form = MultipartForm::new()
        .add_text("csrf_token", token)
        .add_text("student_name", "设置流程测试")
        .add_text("student_no", "SETTINGS-TEST")
        .add_text("has_result", if declaration { "no" } else { "yes" })
        .add_text("no_result_confirm", "yes");
    if !declaration {
        form = form
            .add_text("result_name", "设置测试成果")
            .add_text("obtained_date", date.to_owned())
            .add_text("category", "academic_competition")
            .add_text("competition_name", "测试竞赛")
            .add_text("competition_type", "A")
            .add_text("level", "国家")
            .add_text("award_level", "一等奖")
            .add_part(
                "attachments",
                Part::bytes(b"%PDF-1.4 test".to_vec())
                    .file_name("proof.pdf")
                    .mime_type("application/pdf"),
            );
    }
    f.student.post("/submit").multipart(form).await
}

#[tokio::test]
async fn settings_require_admin_and_csrf_before_mutation() {
    let f = fixture().await;
    let read = f.student.get("/admin/settings").await;
    assert_eq!(read.status_code(), 303);
    assert_eq!(read.headers()["location"], "/admin/login");
    enter(&f, &f.code).await;
    for path in [
        "/admin/years",
        "/admin/years/1",
        "/admin/years/1/activate",
        "/admin/settings/class-code",
    ] {
        assert_eq!(f.student.post(path).await.status_code(), 403);
        assert_eq!(
            f.server
                .post(path)
                .form(&json!({"csrf_token":"invalid"}))
                .await
                .status_code(),
            400
        );
    }
    assert_eq!(AcademicYearRepo::list(&f.state.db).await.unwrap().len(), 1);
    let response = f.server.get("/admin/settings").await;
    assert_eq!(response.status_code(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert!(!response.text().contains("argon2"));
}

#[tokio::test]
async fn create_is_inactive_and_edit_updates_home_without_restart() {
    let f = fixture().await;
    let id = create(&f, "新学年").await;
    assert_eq!(
        AcademicYearRepo::current(&f.state.db)
            .await
            .unwrap()
            .unwrap()
            .id,
        f.year_id
    );
    let mut form = year_form(&f.token, "已修改学年");
    form["deadline"] = json!("2099-09-09T10:30");
    form["announcement"] = json!("说明 <script>不可执行</script>\n第二行");
    let response = f
        .server
        .post(&format!("/admin/years/{}", f.year_id))
        .form(&form)
        .await;
    assert_eq!(response.status_code(), 303);
    let home = f.student.get("/").await.text();
    assert!(home.contains("已修改学年"));
    assert!(home.contains("2099-09-09"));
    assert!(!home.contains("<script>不可执行</script>"));
    let year = AcademicYearRepo::current(&f.state.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        year.deadline.unwrap().to_rfc3339(),
        "2099-09-09T10:30:00+00:00"
    );
    assert_eq!(year.id, f.year_id);
    // Editing an inactive year cannot activate it even with a forged field.
    form["name"] = json!("新学年修改");
    form["is_active"] = json!("true");
    assert_eq!(
        f.server
            .post(&format!("/admin/years/{id}"))
            .form(&form)
            .await
            .status_code(),
        303
    );
    assert!(
        !AcademicYearRepo::find_by_id(&f.state.db, id)
            .await
            .unwrap()
            .unwrap()
            .is_active
    );
}

#[tokio::test]
async fn invalid_years_and_duplicate_names_return_inline_errors_without_writes() {
    let f = fixture().await;
    f.server
        .post("/admin/roster/prepare")
        .multipart(
            MultipartForm::new()
                .add_text("csrf_token", f.token.clone())
                .add_text("roster_text", "姓名,学号\n设置流程测试,SETTINGS-TEST"),
        )
        .await
        .assert_status_see_other();
    let html = f.server.get("/admin/settings").await.text();
    let draft = html
        .split("name=\"roster_draft_id\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    for (field, value) in [
        ("name", "".into()),
        ("name", "年".repeat(41)),
        ("start_date", "2026-02-30".into()),
        ("start_date", "2026-8-1".into()),
        ("end_date", "2025-08-30".into()),
        ("deadline", "not-a-time".into()),
        ("deadline", "2026-02-30T10:00".into()),
        ("announcement", "文".repeat(4001)),
    ] {
        let mut form = year_form(&f.token, "保留输入");
        form["roster_draft_id"] = json!(draft);
        form[field] = json!(value);
        let response = f.server.post("/admin/years").form(&form).await;
        assert_eq!(
            response.status_code(),
            422,
            "field {field}: {}",
            response.text()
        );
        assert!(response.text().contains("aria-invalid=\"true\""));
        assert!(response.text().contains("role=\"alert\""));
        assert!(!response.text().contains("sqlx"));
    }
    let mut duplicate_fields = year_form(&f.token, "2025-2026学年");
    duplicate_fields["roster_draft_id"] = json!(draft);
    let duplicate = f.server.post("/admin/years").form(&duplicate_fields).await;
    assert_eq!(duplicate.status_code(), 422);
    assert!(duplicate.text().contains("学年名称已存在"));
    let id = create(&f, "另一学年").await;
    let original = AcademicYearRepo::current(&f.state.db)
        .await
        .unwrap()
        .unwrap();
    let duplicate = f
        .server
        .post(&format!("/admin/years/{}", f.year_id))
        .form(&year_form(&f.token, "另一学年"))
        .await;
    assert_eq!(duplicate.status_code(), 422);
    assert_eq!(
        AcademicYearRepo::current(&f.state.db)
            .await
            .unwrap()
            .unwrap(),
        original
    );
    let mut same_day = year_form(&f.token, "同日学年");
    same_day["end_date"] = json!("2025-08-31");
    assert_eq!(
        f.server
            .post(&format!("/admin/years/{id}"))
            .form(&same_day)
            .await
            .status_code(),
        303
    );
}

#[tokio::test]
async fn activation_is_atomic_repeatable_and_invalidates_old_year_access() {
    let f = fixture().await;
    enter(&f, &f.code).await;
    let id = create(&f, "下一学年").await;
    for _ in 0..2 {
        let response = f
            .server
            .post(&format!("/admin/years/{id}/activate"))
            .form(&json!({"csrf_token":f.token}))
            .await;
        assert_eq!(response.status_code(), 303);
        let years = AcademicYearRepo::list(&f.state.db).await.unwrap();
        assert_eq!(years.iter().filter(|y| y.is_active).count(), 1);
        assert_eq!(years.iter().find(|y| y.is_active).unwrap().id, id);
    }
    assert_eq!(f.student.get("/submit").await.status_code(), 303);
    let before = AcademicYearRepo::list(&f.state.db).await.unwrap();
    assert_eq!(
        f.server
            .post("/admin/years/999999/activate")
            .form(&json!({"csrf_token":f.token}))
            .await
            .status_code(),
        404
    );
    assert_eq!(AcademicYearRepo::list(&f.state.db).await.unwrap(), before);
    let first = f
        .server
        .post(&format!("/admin/years/{}/activate", f.year_id))
        .form(&json!({"csrf_token":f.token}));
    let second = f
        .server
        .post(&format!("/admin/years/{id}/activate"))
        .form(&json!({"csrf_token":f.token}));
    let (first, second) = tokio::join!(first, second);
    assert_eq!(first.status_code(), 303);
    assert_eq!(second.status_code(), 303);
    assert_eq!(
        AcademicYearRepo::list(&f.state.db)
            .await
            .unwrap()
            .iter()
            .filter(|y| y.is_active)
            .count(),
        1
    );
}

#[tokio::test]
async fn activation_lock_conflict_is_inline_and_preserves_current_year() {
    let f = fixture().await;
    let id = create(&f, "待激活学年").await;
    let lock = f.state.db.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let response = f
        .server
        .post(&format!("/admin/years/{id}/activate"))
        .form(&json!({"csrf_token":f.token}))
        .await;
    assert_eq!(response.status_code(), 409);
    assert!(response.text().contains("role=\"alert\""));
    assert!(!response.text().contains("database is locked"));
    lock.rollback().await.unwrap();
    assert_eq!(
        AcademicYearRepo::current(&f.state.db)
            .await
            .unwrap()
            .unwrap()
            .id,
        f.year_id
    );
}

#[tokio::test]
async fn deadline_changes_apply_to_results_and_declarations_and_can_be_cleared() {
    let f = fixture().await;
    enter(&f, &f.code).await;
    let mut form = year_form(&f.token, "2025-2026学年");
    form["deadline"] = json!("2000-01-01T00:00");
    assert_eq!(
        f.server
            .post(&format!("/admin/years/{}", f.year_id))
            .form(&form)
            .await
            .status_code(),
        303
    );
    for declaration in [false, true] {
        let response = submit(&f, declaration, "2026-04-02").await;
        assert_eq!(response.status_code(), 422, "{}", response.text());
        assert!(response.text().contains("截止"));
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM student_declarations")
        .fetch_one(&f.state.db)
        .await
        .unwrap();
    assert_eq!(count, 0);
    form["deadline"] = json!("");
    form["start_date"] = json!("2026-05-01");
    assert_eq!(
        f.server
            .post(&format!("/admin/years/{}", f.year_id))
            .form(&form)
            .await
            .status_code(),
        303
    );
    assert_eq!(submit(&f, false, "2026-04-02").await.status_code(), 422);
    assert_eq!(submit(&f, true, "").await.status_code(), 303);
    assert_eq!(submit(&f, false, "2026-05-01").await.status_code(), 303);
}

#[tokio::test]
async fn historical_records_and_attachments_survive_switch_and_rename() {
    let f = fixture().await;
    enter(&f, &f.code).await;
    let response = submit(&f, false, "2026-04-02").await;
    assert_eq!(response.status_code(), 303);
    let receipt = f
        .student
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
    let old = SubmissionRepo::list(&f.state.db, &SubmissionFilter::default())
        .await
        .unwrap()
        .remove(0);
    let token = csrf(&f.student.get("/query").await.text());
    assert_eq!(
        f.student
            .post("/query")
            .form(&json!({"csrf_token":token,"submission_no":old.submission_no,"edit_code":code}))
            .await
            .status_code(),
        303
    );
    assert_eq!(
        f.server
            .post(&format!("/admin/years/{}", f.year_id))
            .form(&year_form(&f.token, "历史重命名"))
            .await
            .status_code(),
        303
    );
    let id = create(&f, "同结束年份的新学年").await;
    assert_eq!(
        f.server
            .post(&format!("/admin/years/{id}/activate"))
            .form(&json!({"csrf_token":f.token}))
            .await
            .status_code(),
        303
    );
    let detail = f
        .student
        .get(&format!("/query/{}", old.submission_no))
        .await;
    assert_eq!(detail.status_code(), 200);
    assert!(detail.text().contains("设置测试成果"));
    let attachments = zongce_web::db::AttachmentRepo::list_for_submission(&f.state.db, old.id)
        .await
        .unwrap();
    assert_eq!(
        f.student
            .get(&format!(
                "/submissions/{}/attachments/{}",
                old.submission_no, attachments[0].id
            ))
            .await
            .status_code(),
        200
    );
    assert_eq!(
        f.server
            .get(&format!("/admin/submissions/{}", old.id))
            .await
            .status_code(),
        200
    );
    assert!(
        f.server
            .get(&format!("/admin?academic_year_id={}", f.year_id))
            .await
            .text()
            .contains(&old.submission_no)
    );
    assert_eq!(
        SubmissionRepo::find_by_id(&f.state.db, old.id)
            .await
            .unwrap()
            .unwrap(),
        old
    );
    // Two distinct academic years can end in the same calendar year; numbering stays global.
    enter(&f, &f.code).await;
    let next = submit(&f, false, "2026-04-02").await;
    assert_eq!(next.status_code(), 303, "{}", next.text());
    assert_ne!(
        next.headers()["location"],
        format!("/success/{}", old.submission_no)
    );
}

#[tokio::test]
async fn class_code_replacement_is_hashed_immediate_and_never_echoed() {
    let f = fixture().await;
    let code = format!(" {} ", generate_edit_code());
    let response = f
        .server
        .post("/admin/settings/class-code")
        .form(&json!({"csrf_token":f.token,"class_access_code":code}))
        .await;
    assert_eq!(response.status_code(), 303);
    let hash = SettingsRepo::get(&f.state.db, "class_access_code_hash")
        .await
        .unwrap()
        .unwrap();
    assert!(verify_secret(&hash, &code));
    assert!(!verify_secret(&hash, &f.code));
    let page = f.server.get("/admin/settings").await.text();
    assert!(!page.contains(code.trim()));
    assert!(!page.contains(&hash));
    let token = csrf(&f.student.get("/").await.text());
    assert_eq!(
        f.student
            .post("/access")
            .form(&json!({"csrf_token":token,"access_code":f.code}))
            .await
            .status_code(),
        400
    );
    enter(&f, &code).await;
    for invalid in ["   ".into(), "x".repeat(257)] {
        let response = f
            .server
            .post("/admin/settings/class-code")
            .form(&json!({"csrf_token":f.token,"class_access_code":invalid}))
            .await;
        assert_eq!(response.status_code(), 422);
        assert!(response.text().contains("aria-invalid=\"true\""));
        assert!(!response.text().contains(&format!("value=\"{invalid}\"")));
        assert_eq!(
            SettingsRepo::get(&f.state.db, "class_access_code_hash")
                .await
                .unwrap()
                .unwrap(),
            hash
        );
    }
}

#[tokio::test]
async fn missing_years_and_malformed_settings_inputs_are_safe() {
    let f = fixture().await;
    for path in [
        "/admin/years/not-a-number",
        "/admin/years/not-a-number/activate",
    ] {
        let response = f
            .server
            .post(path)
            .form(&json!({"csrf_token":f.token}))
            .await;
        assert_eq!(response.status_code(), 400);
        assert!(response.text().contains("请求无效"));
    }
    assert_eq!(
        f.server
            .post("/admin/years/999999")
            .form(&year_form(&f.token, "不存在"))
            .await
            .status_code(),
        404
    );
    assert_eq!(
        f.server
            .post("/admin/years")
            .content_type("application/x-www-form-urlencoded")
            .text("name=a&name=b")
            .await
            .status_code(),
        400
    );
    sqlx::query("DELETE FROM academic_years")
        .execute(&f.state.db)
        .await
        .unwrap();
    let page = f.server.get("/admin/settings").await;
    assert_eq!(page.status_code(), 200);
    assert!(page.text().contains("暂无学年"));
    assert!(f.student.get("/").await.text().contains("申报暂未开放"));
    let id = create(&f, "首个学年").await;
    assert!(
        AcademicYearRepo::current(&f.state.db)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        f.server
            .post(&format!("/admin/years/{id}/activate"))
            .form(&json!({"csrf_token":f.token}))
            .await
            .status_code(),
        303
    );
    assert!(f.student.get("/").await.text().contains("首个学年"));
}

#[tokio::test]
async fn display_names_are_independent_of_attachment_directories() {
    let f = fixture().await;
    let id = create(&f, "2025/2026学年").await;
    assert_eq!(
        f.server
            .post(&format!("/admin/years/{id}/activate"))
            .form(&json!({"csrf_token":f.token}))
            .await
            .status_code(),
        303
    );
    enter(&f, &f.code).await;
    let response = submit(&f, false, "2026-04-02").await;
    assert_eq!(response.status_code(), 303);
    let receipt = f
        .student
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
    let old = SubmissionRepo::list(&f.state.db, &SubmissionFilter::default())
        .await
        .unwrap()
        .remove(0);
    let token = csrf(&f.student.get("/query").await.text());
    assert_eq!(
        f.student
            .post("/query")
            .form(&json!({"csrf_token":token,"submission_no":old.submission_no,"edit_code":code}))
            .await
            .status_code(),
        303
    );
    assert_eq!(
        f.server
            .post(&format!("/admin/years/{id}"))
            .form(&year_form(&f.token, ".."))
            .await
            .status_code(),
        303
    );
    assert_eq!(submit(&f, false, "2026-04-02").await.status_code(), 303);
    let token = csrf(
        &f.student
            .get(&format!("/query/{}", old.submission_no))
            .await
            .text(),
    );
    let updated = f
        .student
        .post(&format!("/query/{}/update", old.submission_no))
        .multipart(
            MultipartForm::new()
                .add_text("csrf_token", token)
                .add_text("student_name", "设置流程测试")
                .add_text("student_no", "SETTINGS-TEST")
                .add_text("result_name", "改名后补充")
                .add_text("obtained_date", "2026-04-02")
                .add_text("category", "academic_competition")
                .add_text("competition_name", "测试竞赛")
                .add_text("competition_type", "A")
                .add_text("level", "国家")
                .add_text("award_level", "一等奖")
                .add_part(
                    "attachments",
                    Part::bytes(b"%PDF-1.4 extra".to_vec())
                        .file_name("extra.pdf")
                        .mime_type("application/pdf"),
                ),
        )
        .await;
    assert_eq!(updated.status_code(), 303);
    let files = zongce_web::db::AttachmentRepo::list_for_submission(&f.state.db, old.id)
        .await
        .unwrap();
    assert_eq!(files.len(), 2);
    let cold_storage = AttachmentStorage::new(f.state.storage.root());
    for file in files {
        assert!(cold_storage.open(&file.stored_name).await.is_ok());
    }
}

#[tokio::test]
async fn settings_changed_during_body_upload_are_rechecked_before_persistence() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    for declaration in [false, true] {
        for change in ["activation", "deadline", "date-range"] {
            // Declarations have no achievement date, so that restriction doesn't apply.
            if declaration && change == "date-range" {
                continue;
            }
            let f = fixture().await;
            let home = f.student.get("/").await;
            let cookie = home.headers()["set-cookie"]
                .to_str()
                .unwrap()
                .split(';')
                .next()
                .unwrap()
                .to_owned();
            enter(&f, &f.code).await;
            let token = csrf(&f.student.get("/submit").await.text());
            let mut body = String::new();
            for (name, value) in [
                ("csrf_token", token.as_str()),
                ("has_result", if declaration { "no" } else { "yes" }),
                ("no_result_confirm", "yes"),
                ("student_name", "上传竞态测试"),
                ("student_no", "RACE-TEST"),
                ("result_name", "竞赛"),
                ("obtained_date", "2026-04-02"),
                ("category", "academic_competition"),
                ("competition_name", "测试竞赛"),
                ("competition_type", "A"),
                ("level", "国家"),
                ("award_level", "一等奖"),
            ] {
                body.push_str(&format!("--settings-boundary\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"));
            }
            if !declaration {
                body.push_str("--settings-boundary\r\nContent-Disposition: form-data; name=\"attachments\"; filename=\"proof.pdf\"\r\nContent-Type: application/pdf\r\n\r\n%PDF-1.4 test\r\n");
            }
            body.push_str("--settings-boundary--\r\n");
            let url = f.student.server_address().unwrap();
            let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", url.port().unwrap()))
                .await
                .unwrap();
            stream.write_all(format!("POST /submit HTTP/1.1\r\nHost: localhost\r\nCookie: {cookie}\r\nContent-Type: multipart/form-data; boundary=settings-boundary\r\nContent-Length: {}\r\nExpect: 100-continue\r\nConnection: close\r\n\r\n",body.len()).as_bytes()).await.unwrap();
            // Hyper emits 100 only when the handler polls the body, after the initial year lookup.
            let interim = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                let mut bytes = Vec::new();
                while !bytes.ends_with(b"\r\n\r\n") {
                    bytes.push(stream.read_u8().await.unwrap());
                }
                String::from_utf8(bytes).unwrap()
            })
            .await
            .unwrap();
            assert!(interim.starts_with("HTTP/1.1 100"));
            if change == "activation" {
                let id = create(&f, "上传期间切换").await;
                assert_eq!(
                    f.server
                        .post(&format!("/admin/years/{id}/activate"))
                        .form(&json!({"csrf_token":f.token}))
                        .await
                        .status_code(),
                    303
                );
            } else {
                let mut form = year_form(&f.token, "2025-2026学年");
                if change == "deadline" {
                    form["deadline"] = json!("2000-01-01T00:00");
                } else {
                    form["start_date"] = json!("2026-05-01");
                }
                assert_eq!(
                    f.server
                        .post(&format!("/admin/years/{}", f.year_id))
                        .form(&form)
                        .await
                        .status_code(),
                    303
                );
            }
            stream.write_all(body.as_bytes()).await.unwrap();
            let mut response = String::new();
            tokio::time::timeout(
                std::time::Duration::from_secs(5),
                stream.read_to_string(&mut response),
            )
            .await
            .unwrap()
            .unwrap();
            if change == "activation" {
                assert!(response.starts_with("HTTP/1.1 303"));
                assert!(
                    response.to_lowercase().contains("location: /\r\n"),
                    "stale-year submission was accepted"
                );
            } else {
                assert!(
                    response.starts_with("HTTP/1.1 422"),
                    "stale settings accepted: {change}"
                );
            }
            for table in ["submissions", "student_declarations"] {
                let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
                    .fetch_one(&f.state.db)
                    .await
                    .unwrap();
                assert_eq!(count, 0);
            }
        }
    }
}
