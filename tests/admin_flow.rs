use axum_test::TestServer;
use zongce_web::state::AppState;

#[tokio::test]
async fn anonymous_admin_reads_redirect_and_writes_are_forbidden() {
    let state = AppState::initialize("sqlite::memory:").await.unwrap();
    let server = TestServer::new(zongce_web::routes::build_router(state)).unwrap();
    for path in [
        "/admin",
        "/admin/submissions/1",
        "/admin/submissions/not-a-number",
    ] {
        let response = server.get(path).await;
        assert_eq!(response.status_code(), 303);
        assert_eq!(response.headers()["location"], "/admin/login");
    }
    for path in ["/admin/logout", "/admin/submissions/1/review"] {
        assert_eq!(server.post(path).await.status_code(), 403);
    }
}

use chrono::NaiveDate;
use serde_json::json;
use zongce_web::{
    auth::{generate_edit_code, hash_secret},
    config::Config,
    db::{
        AcademicYearRepo, AttachmentRepo, DeclarationRepo, NewAcademicYear, NewAttachment,
        NewSubmission, SettingsRepo, SubmissionRepo,
    },
    domain::{Category, Submission, SubmissionStatus},
    storage::{AttachmentStorage, UploadInput},
};

struct Fixture {
    server: TestServer,
    state: AppState,
    _dir: tempfile::TempDir,
    password: String,
    year_id: i64,
}

async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let mut state = AppState::initialize("sqlite::memory:").await.unwrap();
    state.storage = AttachmentStorage::new(dir.path());
    let password = generate_edit_code();
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
    let mut server = TestServer::new(
        zongce_web::routes::build_router_with_config(state.clone(), &config).unwrap(),
    )
    .unwrap();
    server.save_cookies();
    let year_id = AcademicYearRepo::current(&state.db)
        .await
        .unwrap()
        .unwrap()
        .id;
    Fixture {
        server,
        state,
        _dir: dir,
        password,
        year_id,
    }
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

async fn login(f: &Fixture) -> String {
    let response = f.server.get("/admin/login").await;
    assert_eq!(response.status_code(), 200);
    let old_csrf = csrf(&response.text());
    let result = f
        .server
        .post("/admin/login")
        .form(&json!({"csrf_token":old_csrf,"username":"test-admin","password":f.password}))
        .await;
    assert_eq!(result.status_code(), 303);
    assert_eq!(result.headers()["location"], "/admin");
    let dashboard = f.server.get("/admin").await;
    assert_eq!(dashboard.status_code(), 200);
    assert_eq!(dashboard.headers()["cache-control"], "no-store");
    let token = csrf(&dashboard.text());
    assert_ne!(token, old_csrf, "privilege change must rotate CSRF");
    token
}

async fn record(
    f: &Fixture,
    seq: u32,
    year_id: i64,
    name: &str,
    category: Category,
    status: SubmissionStatus,
    score: Option<f64>,
) -> Submission {
    let submission = SubmissionRepo::insert(&f.state.db, &NewSubmission {
        submission_no: format!("ZC2026-{seq:06}"), academic_year_id: year_id,
        student_name: name.into(), student_no: format!("TEST-{seq}"), category,
        result_name: format!("测试成果{seq}<script>不可执行</script>"),
        obtained_date: NaiveDate::from_ymd_opt(2026,4,2).unwrap(),
        detail: Some("测试说明".into()), remark: Some("学生备注".into()),
        category_data: json!({"competition_name":"测试竞赛","competition_type":"A","level":"国家","award_level":"一等奖"}),
        edit_code_hash: hash_secret(&generate_edit_code()).unwrap(),
    }).await.unwrap();
    SubmissionRepo::update_review(&f.state.db, submission.id, status, None, score)
        .await
        .unwrap();
    submission
}

#[tokio::test]
async fn login_uses_generic_failure_csrf_and_logout_revokes_access() {
    let f = fixture().await;
    let token = csrf(&f.server.get("/admin/login").await.text());
    for (username, password) in [
        ("unknown", f.password.as_str()),
        ("test-admin", "invalid-test-value"),
    ] {
        let response = f
            .server
            .post("/admin/login")
            .form(&json!({"csrf_token":token,"username":username,"password":password}))
            .await;
        assert_eq!(response.status_code(), 401);
        assert!(response.text().contains("用户名或密码错误"));
        assert!(!response.text().contains(password));
        assert!(!response.text().contains("argon2"));
    }
    assert_eq!(
        f.server
            .post("/admin/login")
            .form(&json!({"username":"test-admin","password":f.password}))
            .await
            .status_code(),
        400
    );
    let token = login(&f).await;
    assert_eq!(f.server.get("/admin/login").await.status_code(), 303);
    assert_eq!(
        f.server
            .post("/admin/logout")
            .form(&json!({"csrf_token":"invalid"}))
            .await
            .status_code(),
        400
    );
    assert_eq!(f.server.get("/admin").await.status_code(), 200);
    assert_eq!(
        f.server
            .post("/admin/logout")
            .form(&json!({"csrf_token":token}))
            .await
            .status_code(),
        303
    );
    assert_eq!(f.server.get("/admin").await.status_code(), 303);
    assert_eq!(
        f.server
            .post("/admin/submissions/1/review")
            .await
            .status_code(),
        403
    );
}

#[tokio::test]
async fn student_class_and_verified_sessions_cannot_review() {
    let f = fixture().await;
    let class_code = generate_edit_code();
    SettingsRepo::set(
        &f.state.db,
        "class_access_code_hash",
        &hash_secret(&class_code).unwrap(),
    )
    .await
    .unwrap();
    let token = csrf(&f.server.get("/").await.text());
    assert_eq!(
        f.server
            .post("/access")
            .form(&json!({"csrf_token":token,"access_code":class_code}))
            .await
            .status_code(),
        303
    );
    assert_eq!(f.server.get("/admin").await.status_code(), 303);
    let submission = record(
        &f,
        1,
        f.year_id,
        "测试学生",
        Category::AcademicCompetition,
        SubmissionStatus::Pending,
        None,
    )
    .await;
    let edit_code = generate_edit_code();
    sqlx::query("UPDATE submissions SET edit_code_hash = ? WHERE id = ?")
        .bind(hash_secret(&edit_code).unwrap())
        .bind(submission.id)
        .execute(&f.state.db)
        .await
        .unwrap();
    assert_eq!(f.server.post("/query").form(&json!({"csrf_token":token,"submission_no":submission.submission_no,"edit_code":edit_code})).await.status_code(),303);
    assert_eq!(
        f.server
            .get(&format!("/admin/submissions/{}", submission.id))
            .await
            .status_code(),
        303
    );
    assert_eq!(
        f.server
            .post(&format!("/admin/submissions/{}/review", submission.id))
            .form(&json!({"csrf_token":token,"status":"approved","approved_score":"9"}))
            .await
            .status_code(),
        403
    );
    assert_eq!(
        SubmissionRepo::find_by_id(&f.state.db, submission.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        SubmissionStatus::Pending
    );
}

#[tokio::test]
async fn dashboard_filters_sort_and_counts_use_selected_records() {
    let f = fixture().await;
    let history = AcademicYearRepo::insert(
        &f.state.db,
        &NewAcademicYear {
            name: "历史测试学年".into(),
            start_date: NaiveDate::from_ymd_opt(2024, 9, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2025, 8, 31).unwrap(),
            deadline: None,
            is_active: false,
            announcement: None,
        },
    )
    .await
    .unwrap();
    let first = record(
        &f,
        1,
        f.year_id,
        "甲测试",
        Category::AcademicCompetition,
        SubmissionStatus::Approved,
        Some(1.25),
    )
    .await;
    let second = record(
        &f,
        2,
        f.year_id,
        "乙测试",
        Category::Patent,
        SubmissionStatus::Pending,
        None,
    )
    .await;
    record(
        &f,
        3,
        history.id,
        "历史测试",
        Category::AcademicCompetition,
        SubmissionStatus::Approved,
        Some(90.0),
    )
    .await;
    record(
        &f,
        4,
        f.year_id,
        "百分%测试",
        Category::AcademicCompetition,
        SubmissionStatus::NeedsRevision,
        None,
    )
    .await;
    record(
        &f,
        5,
        f.year_id,
        "拒绝测试",
        Category::AcademicCompetition,
        SubmissionStatus::Rejected,
        Some(5.0),
    )
    .await;
    DeclarationRepo::upsert(&f.state.db, f.year_id, "甲测试", "TEST-1")
        .await
        .unwrap();
    DeclarationRepo::upsert(&f.state.db, history.id, "历史测试", "TEST-3")
        .await
        .unwrap();
    login(&f).await;
    let html = f.server.get("/admin").await.text();
    assert!(html.find(&second.submission_no).unwrap() < html.find(&first.submission_no).unwrap());
    assert!(!html.contains("ZC2026-000003"));
    for expected in [
        "id=\"count-total\">4<",
        "id=\"count-approved\">1<",
        "id=\"count-pending\">1<",
        "id=\"count-needs_revision\">1<",
        "id=\"count-rejected\">1<",
        "id=\"count-declarations\">1<",
        "id=\"approved-total\">1.25<",
    ] {
        assert!(html.contains(expected), "{expected}");
    }
    for (query, expected) in [
        ("name=甲", "ZC2026-000001"),
        ("student_no=TEST-2", "ZC2026-000002"),
        ("category=patent", "ZC2026-000002"),
        ("status=needs_revision", "ZC2026-000004"),
        ("name=%25", "ZC2026-000004"),
    ] {
        let filtered = f.server.get(&format!("/admin?{query}")).await;
        assert_eq!(filtered.status_code(), 200);
        assert!(filtered.text().contains(expected));
        assert!(filtered.text().contains("id=\"count-total\">1<"));
    }
    let combined = f
        .server
        .get("/admin?name=甲&student_no=TEST-2&category=patent&status=approved")
        .await
        .text();
    assert!(combined.contains("没有符合筛选条件的申报"));
    assert!(combined.contains("id=\"count-total\">0<"));
    let history_html = f
        .server
        .get(&format!("/admin?academic_year_id={}", history.id))
        .await
        .text();
    assert!(history_html.contains("ZC2026-000003"));
    assert!(history_html.contains("id=\"approved-total\">90.00<"));
    assert!(
        f.server
            .get("/admin?academic_year_id=")
            .await
            .text()
            .contains("id=\"count-total\">5<")
    );
    assert!(
        f.server
            .get("/admin?name=%27%20OR%201=1--")
            .await
            .text()
            .contains("id=\"count-total\">0<")
    );
    for invalid in [
        "category=bogus",
        "status=bogus",
        "academic_year_id=abc",
        "academic_year_id=9999",
    ] {
        let response = f.server.get(&format!("/admin?{invalid}")).await;
        assert_eq!(response.status_code(), 200);
        assert!(response.text().contains("已忽略无效筛选条件"));
        assert!(response.text().contains("id=\"count-total\">4<"));
    }
}

#[tokio::test]
async fn reviews_validate_atomically_and_are_visible_to_students() {
    let f = fixture().await;
    let s = record(
        &f,
        1,
        f.year_id,
        "审核测试",
        Category::AcademicCompetition,
        SubmissionStatus::Pending,
        None,
    )
    .await;
    let token = login(&f).await;
    let path = format!("/admin/submissions/{}/review", s.id);
    assert_eq!(
        f.server
            .post(&path)
            .form(&json!({"status":"approved","approved_score":"1"}))
            .await
            .status_code(),
        400
    );
    for (status, score, note) in [
        ("bad", "1", ""),
        ("approved", "", ""),
        ("approved", "-1", ""),
        ("approved", "1.234", ""),
        ("approved", "NaN", ""),
        ("approved", "inf", ""),
        ("approved", "1e2", ""),
        ("pending", "", "x"),
    ] {
        let note = if note == "x" {
            "字".repeat(4001)
        } else {
            note.into()
        };
        let response = f.server.post(&path).form(&json!({"csrf_token":token,"status":status,"approved_score":score,"review_note":note})).await;
        assert_eq!(response.status_code(), 422, "{status} {score}");
        assert!(response.text().contains("aria-invalid=\"true\""));
        let stored = SubmissionRepo::find_by_id(&f.state.db, s.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, SubmissionStatus::Pending);
        assert_eq!(stored.approved_score, None);
        assert_eq!(stored.review_note, None);
    }
    for (status, score, expected) in [
        ("needs_revision", "", SubmissionStatus::NeedsRevision),
        ("approved", "0", SubmissionStatus::Approved),
        ("rejected", "0", SubmissionStatus::Rejected),
        ("pending", "", SubmissionStatus::Pending),
        ("approved", "2.35", SubmissionStatus::Approved),
    ] {
        let response = f.server.post(&path).form(&json!({"csrf_token":token,"status":status,"approved_score":score,"review_note":"  <script>测试备注</script>  "})).await;
        assert_eq!(response.status_code(), 303);
        let detail = f
            .server
            .get(response.headers()["location"].to_str().unwrap())
            .await;
        assert!(detail.text().contains("审核已保存"));
        assert!(!detail.text().contains("<script>测试备注</script>"));
        let stored = SubmissionRepo::find_by_id(&f.state.db, s.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, expected);
        assert_eq!(
            stored.approved_score,
            if score.is_empty() {
                None
            } else {
                Some(score.parse::<f64>().unwrap())
            }
        );
        assert_eq!(
            stored.review_note.as_deref(),
            Some("<script>测试备注</script>")
        );
        assert!(stored.updated_at >= s.updated_at);
    }
    let code = generate_edit_code();
    sqlx::query("UPDATE submissions SET edit_code_hash = ? WHERE id = ?")
        .bind(hash_secret(&code).unwrap())
        .bind(s.id)
        .execute(&f.state.db)
        .await
        .unwrap();
    f.server
        .post("/admin/logout")
        .form(&json!({"csrf_token":token}))
        .await;
    let token = csrf(&f.server.get("/query").await.text());
    f.server
        .post("/query")
        .form(&json!({"csrf_token":token,"submission_no":s.submission_no,"edit_code":code}))
        .await;
    let html = f
        .server
        .get(&format!("/query/{}", s.submission_no))
        .await
        .text();
    assert!(html.contains("已通过"));
    assert!(html.contains("2.35"));
    assert!(html.contains("仅可查看"));
}

#[tokio::test]
async fn admin_detail_protects_attachments_and_displays_category_and_audit_marker() {
    let f = fixture().await;
    let s = record(
        &f,
        1,
        f.year_id,
        "附件测试",
        Category::AcademicCompetition,
        SubmissionStatus::Pending,
        None,
    )
    .await;
    sqlx::query("UPDATE submissions SET student_modified_after_review = 1 WHERE id = ?")
        .bind(s.id)
        .execute(&f.state.db)
        .await
        .unwrap();
    let uploads = f
        .state
        .storage
        .save_many(
            "2025-2026",
            &s.submission_no,
            0,
            vec![UploadInput {
                original_name: "测试.pdf".into(),
                mime_type: "application/pdf".into(),
                bytes: b"proof".to_vec(),
            }],
        )
        .await
        .unwrap();
    let upload = &uploads[0];
    let attachment = AttachmentRepo::insert(
        &f.state.db,
        &NewAttachment {
            submission_id: s.id,
            original_name: upload.original_name.clone(),
            stored_name: upload.stored_name.clone(),
            mime_type: upload.mime_type.clone(),
            file_size: upload.file_size,
        },
    )
    .await
    .unwrap();
    let url = format!(
        "/submissions/{}/attachments/{}",
        s.submission_no, attachment.id
    );
    assert_eq!(f.server.get(&url).await.status_code(), 403);
    let token = login(&f).await;
    let html = f
        .server
        .get(&format!("/admin/submissions/{}", s.id))
        .await
        .text();
    for value in [
        "测试竞赛",
        "一等奖",
        "已重新提交审核",
        "测试说明",
        "学生备注",
        "测试.pdf",
    ] {
        assert!(html.contains(value), "{value}");
    }
    assert!(!html.contains(&s.edit_code_hash));
    assert!(!html.contains("<script>不可执行</script>"));
    let file = f.server.get(&url).await;
    assert_eq!(file.status_code(), 200);
    assert_eq!(&file.as_bytes()[..], b"proof");
    assert_eq!(file.headers()["content-type"], "application/pdf");
    assert_eq!(file.headers()["cache-control"], "no-store");
    assert_eq!(
        f.server.get("/admin/submissions/9999").await.status_code(),
        404
    );
    assert_eq!(
        f.server
            .post("/admin/submissions/9999/review")
            .form(&json!({"csrf_token":token,"status":"pending"}))
            .await
            .status_code(),
        404
    );
    assert_eq!(
        f.server
            .get(&format!(
                "/submissions/{}/attachments/9999",
                s.submission_no
            ))
            .await
            .status_code(),
        404
    );
    f.state
        .storage
        .remove_for_submission("2025-2026", &s.submission_no, &upload.stored_name)
        .await
        .unwrap();
    assert_eq!(f.server.get(&url).await.status_code(), 404);
    let detail = f.server.get(&format!("/admin/submissions/{}", s.id)).await;
    assert_eq!(detail.status_code(), 200);
    assert!(detail.text().contains("附件暂时无法读取"));
    f.server
        .post("/admin/logout")
        .form(&json!({"csrf_token":token}))
        .await;
    assert_eq!(f.server.get(&url).await.status_code(), 403);
}

#[tokio::test]
async fn malformed_admin_requests_return_safe_chinese_errors() {
    let f = fixture().await;
    let response = f
        .server
        .post("/admin/login")
        .json(&json!({"username":"test-admin"}))
        .await;
    assert_eq!(response.status_code(), 400);
    assert_eq!(response.text(), "请求无效");
    login(&f).await;
    for path in [
        "/admin/submissions/not-a-number",
        "/admin/submissions/1?saved=1&saved=2",
    ] {
        let response = f.server.get(path).await;
        assert_eq!(response.status_code(), 400);
        assert_eq!(response.text(), "请求无效");
    }
    for path in ["/admin/logout", "/admin/submissions/1/review"] {
        let response = f
            .server
            .post(path)
            .json(&json!({"status":"approved"}))
            .await;
        assert_eq!(response.status_code(), 400);
        assert_eq!(response.text(), "请求无效");
    }
}

#[tokio::test]
async fn dashboard_without_records_or_active_year_shows_zero_and_keeps_history_access() {
    let f = fixture().await;
    login(&f).await;
    for active in [true, false] {
        sqlx::query("UPDATE academic_years SET is_active = ?")
            .bind(active)
            .execute(&f.state.db)
            .await
            .unwrap();
        let response = f.server.get("/admin").await;
        assert_eq!(response.status_code(), 200);
        let html = response.text();
        assert!(html.contains("id=\"count-total\">0<"));
        assert!(html.contains("id=\"approved-total\">0.00<"));
        assert!(html.contains("没有符合筛选条件的申报"));
        if !active {
            assert!(html.contains("暂无开放学年"));
        }
    }
    let s = record(
        &f,
        1,
        f.year_id,
        "历史记录测试",
        Category::AcademicCompetition,
        SubmissionStatus::Pending,
        None,
    )
    .await;
    assert!(
        f.server
            .get("/admin")
            .await
            .text()
            .contains(&s.submission_no)
    );
}
