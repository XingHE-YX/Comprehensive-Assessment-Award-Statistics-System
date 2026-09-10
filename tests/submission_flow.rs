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

async fn server(active: bool) -> (TestServer, sqlx::SqlitePool, tempfile::TempDir) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("pool");
    migrate(&pool).await.expect("migrate");
    if active {
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
    }
    let dir = tempdir().expect("tempdir");
    let app = zongce_web::routes::build_router(AppState {
        db: pool.clone(),
        storage: AttachmentStorage::new(dir.path()),
    });
    let mut server = TestServer::new(app).expect("server");
    server.save_cookies();
    (server, pool, dir)
}

#[tokio::test]
async fn wrong_access_code_is_rejected_and_correct_code_opens_submit() {
    let (server, _pool, _dir) = server(true).await;
    let home = server.get("/").await;
    let home_html = home.text();
    let csrf = home_html
        .split("name=\"csrf_token\" value=\"")
        .nth(1)
        .and_then(|v| v.split('"').next())
        .expect("csrf");
    let wrong = server
        .post("/access")
        .form(&json!({"csrf_token": csrf, "access_code": "wrong"}))
        .await;
    assert_eq!(wrong.status_code(), 400);
    let ok = server
        .post("/access")
        .form(&json!({"csrf_token": csrf, "access_code": "class-code"}))
        .await;
    assert_eq!(ok.status_code(), 303);
    assert_eq!(ok.headers()["location"], "/submit");
}

#[tokio::test]
async fn no_active_year_keeps_submission_closed() {
    let (server, _pool, _dir) = server(false).await;
    let response = server.get("/").await;
    assert_eq!(response.status_code(), 200);
    assert!(response.text().contains("申报暂未开放"));
}

#[tokio::test]
async fn form_refresh_preserves_text_opens_declaration_and_never_saves_uploads() {
    use axum_test::multipart::{MultipartForm, Part};
    let (server, pool, dir) = server(true).await;
    let token = |html: String| {
        html.split("name=\"csrf_token\" value=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .to_owned()
    };
    let csrf = token(server.get("/").await.text());
    server
        .post("/access")
        .form(&json!({"csrf_token": csrf, "access_code": "class-code"}))
        .await;
    let csrf = token(server.get("/submit").await.text());
    let refresh = server
        .post("/submit")
        .multipart(
            MultipartForm::new()
                .add_text("csrf_token", &csrf)
                .add_text("form_action", "refresh")
                .add_text("has_result", "no")
                .add_text("student_name", "无材料学生")
                .add_text("student_no", "NOJS-001")
                .add_part(
                    "attachments",
                    Part::bytes(b"discard".to_vec())
                        .file_name("unused.pdf")
                        .mime_type("application/pdf"),
                ),
        )
        .await;
    assert_eq!(refresh.status_code(), 200);
    let html = refresh.text();
    assert!(html.contains("value=\"无材料学生\""));
    assert!(
        !html
            .split("id=\"declaration-confirm\"")
            .nth(1)
            .unwrap()
            .split('>')
            .next()
            .unwrap()
            .contains("hidden")
    );
    assert!(
        html.split("data-result-fields")
            .nth(1)
            .unwrap()
            .split('>')
            .next()
            .unwrap()
            .contains("disabled")
    );
    for table in ["submissions", "student_declarations", "attachments"] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
    }
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    let missing_csrf = server
        .post("/submit")
        .multipart(MultipartForm::new().add_text("form_action", "refresh"))
        .await;
    assert_eq!(missing_csrf.status_code(), 400);
    let csrf = token(html);
    let no_confirmation = server
        .post("/submit")
        .multipart(
            MultipartForm::new()
                .add_text("csrf_token", &csrf)
                .add_text("has_result", "no")
                .add_text("student_name", "无材料学生")
                .add_text("student_no", "NOJS-001"),
        )
        .await;
    assert_eq!(no_confirmation.status_code(), 422);
    let csrf = token(no_confirmation.text());
    let submitted = server
        .post("/submit")
        .multipart(
            MultipartForm::new()
                .add_text("csrf_token", csrf)
                .add_text("has_result", "no")
                .add_text("student_name", "无材料学生")
                .add_text("student_no", "NOJS-001")
                .add_text("no_result_confirm", "yes"),
        )
        .await;
    assert_eq!(submitted.status_code(), 303);
    assert_eq!(submitted.headers()["location"], "/success/declaration");
}

#[tokio::test]
async fn valid_submission_creates_unique_number_and_attachment() {
    let (server, pool, _dir) = server(true).await;
    let home = server.get("/").await;
    let home_html = home.text();
    let csrf = home_html
        .split("name=\"csrf_token\" value=\"")
        .nth(1)
        .and_then(|v| v.split('"').next())
        .expect("csrf")
        .to_owned();
    server
        .post("/access")
        .form(&json!({"csrf_token": csrf, "access_code": "class-code"}))
        .await;
    let form = server.get("/submit").await;
    let form_html = form.text();
    let submit_csrf = form_html
        .split("name=\"csrf_token\" value=\"")
        .nth(1)
        .and_then(|v| v.split('"').next())
        .expect("csrf");
    let response = server
        .post("/submit")
        .multipart(
            axum_test::multipart::MultipartForm::new()
                .add_text("csrf_token", submit_csrf)
                .add_text("has_result", "yes")
                .add_text("student_name", "张三")
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
    let location = response.headers()["location"]
        .to_str()
        .expect("location")
        .to_owned();
    let success = server.get(&location).await;
    assert_eq!(success.status_code(), 200);
    assert!(success.text().contains("ZC2026-000001"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM attachments")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );

    let second_form = server.get("/submit").await;
    let second_csrf = second_form
        .text()
        .split("name=\"csrf_token\" value=\"")
        .nth(1)
        .and_then(|v| v.split('"').next())
        .expect("csrf")
        .to_owned();
    let second = server
        .post("/submit")
        .multipart(
            axum_test::multipart::MultipartForm::new()
                .add_text("csrf_token", second_csrf)
                .add_text("has_result", "yes")
                .add_text("student_name", "王五")
                .add_text("student_no", "20250003")
                .add_text("result_name", "文体比赛二等奖")
                .add_text("obtained_date", "2026-04-02")
                .add_text("category", "sports_arts_competition")
                .add_text("competition_name", "校运会")
                .add_text("level", "校")
                .add_text("has_award_level", "yes")
                .add_text("award_level", "二等奖")
                .add_part(
                    "attachments",
                    axum_test::multipart::Part::bytes(b"proof-2".to_vec())
                        .file_name("proof-2.pdf")
                        .mime_type("application/pdf"),
                ),
        )
        .await;
    assert_eq!(second.status_code(), 303);
    let second_location = second.headers()["location"]
        .to_str()
        .expect("location")
        .to_owned();
    assert_ne!(location, second_location);
    assert!(second_location.ends_with("ZC2026-000002"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM submissions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
}

#[tokio::test]
async fn no_material_declaration_creates_no_submission_or_attachment() {
    let (server, pool, _dir) = server(true).await;
    let home = server.get("/").await;
    let home_html = home.text();
    let csrf = home_html
        .split("name=\"csrf_token\" value=\"")
        .nth(1)
        .and_then(|v| v.split('"').next())
        .expect("csrf");
    server
        .post("/access")
        .form(&json!({"csrf_token": csrf, "access_code": "class-code"}))
        .await;
    let form = server.get("/submit").await;
    let form_html = form.text();
    let submit_csrf = form_html
        .split("name=\"csrf_token\" value=\"")
        .nth(1)
        .and_then(|v| v.split('"').next())
        .expect("csrf");
    let response = server
        .post("/submit")
        .multipart(
            axum_test::multipart::MultipartForm::new()
                .add_text("csrf_token", submit_csrf)
                .add_text("has_result", "no")
                .add_text("no_result_confirm", "yes")
                .add_text("student_name", "李四")
                .add_text("student_no", "20250002"),
        )
        .await;
    assert_eq!(response.status_code(), 303);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM submissions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM student_declarations")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn submission_after_deadline_is_rejected() {
    let (server, pool, _dir) = server(true).await;
    sqlx::query("UPDATE academic_years SET deadline = datetime('now', '-1 minute')")
        .execute(&pool)
        .await
        .expect("deadline");

    let home = server.get("/").await;
    let csrf = home
        .text()
        .split("name=\"csrf_token\" value=\"")
        .nth(1)
        .and_then(|v| v.split('"').next())
        .expect("csrf")
        .to_owned();
    server
        .post("/access")
        .form(&json!({"csrf_token": csrf, "access_code": "class-code"}))
        .await;
    let form = server.get("/submit").await;
    let submit_csrf = form
        .text()
        .split("name=\"csrf_token\" value=\"")
        .nth(1)
        .and_then(|v| v.split('"').next())
        .expect("csrf")
        .to_owned();
    let response = server
        .post("/submit")
        .multipart(
            axum_test::multipart::MultipartForm::new()
                .add_text("csrf_token", submit_csrf)
                .add_text("has_result", "yes")
                .add_text("student_name", "赵六")
                .add_text("student_no", "20250004")
                .add_text("result_name", "竞赛成果")
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
    assert_eq!(response.status_code(), 422);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM submissions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}
