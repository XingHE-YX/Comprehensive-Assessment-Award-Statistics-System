use axum_test::{
    TestServer,
    multipart::{MultipartForm, Part},
};
use serde_json::json;
use zongce_web::{
    auth::{generate_edit_code, hash_secret, verify_secret},
    config::Config,
    db::{AttachmentRepo, SettingsRepo, SubmissionRepo},
    domain::Submission,
    state::AppState,
    storage::AttachmentStorage,
};

struct Fixture {
    admin: TestServer,
    student: TestServer,
    state: AppState,
    config: Config,
    password: String,
    token: String,
    student_cookie: String,
    _dir: tempfile::TempDir,
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

fn displayed_code(html: &str, id: &str) -> String {
    html.split(&format!("id=\"{id}\">"))
        .nth(1)
        .expect("displayed credential")
        .split('<')
        .next()
        .unwrap()
        .into()
}

async fn login(server: &TestServer, password: &str) -> String {
    let token = csrf(&server.get("/admin/login").await.text());
    assert_eq!(
        server
            .post("/admin/login")
            .form(&json!({"csrf_token":token,"username":"test-admin","password":password}))
            .await
            .status_code(),
        303
    );
    csrf(&server.get("/admin").await.text())
}

async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let mut state =
        AppState::initialize(&format!("sqlite:{}", dir.path().join("test.db").display()))
            .await
            .unwrap();
    state.storage = AttachmentStorage::new(dir.path().join("uploads"));
    let password = generate_edit_code();
    let class_code = generate_edit_code();
    SettingsRepo::set(
        &state.db,
        "class_access_code_hash",
        &hash_secret(&class_code).unwrap(),
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
    let mut admin = TestServer::new(router.clone()).unwrap();
    let mut student = TestServer::new_with_config(
        router,
        axum_test::TestServerConfig {
            transport: Some(axum_test::Transport::HttpRandomPort),
            ..Default::default()
        },
    )
    .unwrap();
    admin.save_cookies();
    student.save_cookies();
    let token = login(&admin, &password).await;
    let home = student.get("/").await;
    let student_cookie = home.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let student_token = csrf(&home.text());
    assert_eq!(
        student
            .post("/access")
            .form(&json!({"csrf_token":student_token,"access_code":class_code}))
            .await
            .status_code(),
        303
    );
    Fixture {
        admin,
        student,
        state,
        config,
        password,
        token,
        student_cookie,
        _dir: dir,
    }
}

fn edit_form(token: &str) -> MultipartForm {
    MultipartForm::new()
        .add_text("csrf_token", token)
        .add_text("student_name", "修改码测试")
        .add_text("student_no", "CODE-TEST")
        .add_text("result_name", "测试成果")
        .add_text("obtained_date", "2026-04-02")
        .add_text("category", "academic_competition")
        .add_text("competition_name", "测试竞赛")
        .add_text("competition_type", "A")
        .add_text("level", "国家")
        .add_text("award_level", "一等奖")
}

async fn create(f: &Fixture) -> Submission {
    let token = csrf(&f.student.get("/submit").await.text());
    let response = f
        .student
        .post("/submit")
        .multipart(
            edit_form(&token).add_part(
                "attachments",
                Part::bytes(b"test evidence".to_vec())
                    .file_name("test.pdf")
                    .mime_type("application/pdf"),
            ),
        )
        .await;
    assert_eq!(response.status_code(), 303);
    let number = response.headers()["location"]
        .to_str()
        .unwrap()
        .trim_start_matches("/success/");
    SubmissionRepo::find_by_no(&f.state.db, number)
        .await
        .unwrap()
        .unwrap()
}

async fn query(f: &Fixture, submission: &Submission, code: &str) -> u16 {
    let token = csrf(&f.student.get("/query").await.text());
    f.student
        .post("/query")
        .form(
            &json!({"csrf_token":token,"submission_no":submission.submission_no,"edit_code":code}),
        )
        .await
        .status_code()
        .as_u16()
}

async fn reset(f: &Fixture, submission: &Submission, version: i64) -> axum_test::TestResponse {
    f.admin
        .post(&format!(
            "/admin/submissions/{}/edit-code/reset",
            submission.id
        ))
        .form(&json!({"csrf_token":f.token,"confirm_reset":"yes","edit_code_version":version}))
        .await
}

#[tokio::test]
async fn admin_recovers_the_receipt_code_across_restart_without_student_or_export_disclosure() {
    let f = fixture().await;
    let submission = create(&f).await;
    let receipt = f
        .student
        .get(&format!("/success/{}", submission.submission_no))
        .await;
    let code = displayed_code(&receipt.text(), "edit-code");
    assert!(verify_secret(&submission.edit_code_hash, &code));
    let admin_page = f
        .admin
        .get(&format!("/admin/submissions/{}", submission.id))
        .await;
    assert_eq!(admin_page.headers()["cache-control"], "no-store");
    assert!(
        admin_page.text().contains(&code),
        "administrator must recover the submitted code"
    );
    assert_eq!(displayed_code(&admin_page.text(), "admin-edit-code"), code);
    let cipher: String =
        sqlx::query_scalar("SELECT edit_code_ciphertext FROM submissions WHERE id=?")
            .bind(submission.id)
            .fetch_one(&f.state.db)
            .await
            .unwrap();
    assert_ne!(cipher, code);
    assert!(!cipher.contains(&code));
    assert!(!admin_page.text().contains(&submission.edit_code_hash));
    assert!(!admin_page.text().contains(&cipher));
    assert_eq!(query(&f, &submission, &code).await, 303);
    assert!(
        !f.student
            .get(&format!("/query/{}", submission.submission_no))
            .await
            .text()
            .contains(&code)
    );
    assert!(!f.admin.get("/admin").await.text().contains(&code));
    assert_eq!(
        f.student
            .get(&format!("/admin/submissions/{}", submission.id))
            .await
            .status_code(),
        303
    );
    let export = f.admin.get("/admin/export.xlsx").await;
    assert_eq!(export.status_code(), 200);
    let file = f._dir.path().join("export.xlsx");
    std::fs::write(&file, export.as_bytes()).unwrap();
    let output = std::process::Command::new("python3").args(["-c","import sys,zipfile; z=zipfile.ZipFile(sys.argv[1]); print(''.join(z.read(n).decode('utf-8') for n in z.namelist() if n.endswith('.xml')))"]).arg(file).output().unwrap();
    assert!(output.status.success());
    let xml = String::from_utf8(output.stdout).unwrap();
    for secret in [&code, &cipher, &submission.edit_code_hash] {
        assert!(!xml.contains(secret));
    }
    let mut restarted = TestServer::new(
        zongce_web::routes::build_router_with_config(f.state.clone(), &f.config).unwrap(),
    )
    .unwrap();
    restarted.save_cookies();
    login(&restarted, &f.password).await;
    assert_eq!(
        displayed_code(
            &restarted
                .get(&format!("/admin/submissions/{}", submission.id))
                .await
                .text(),
            "admin-edit-code"
        ),
        code
    );
}

#[tokio::test]
async fn explicit_reset_replaces_credentials_revokes_scopes_and_preserves_business_data() {
    let f = fixture().await;
    let submission = create(&f).await;
    SubmissionRepo::update_review(
        &f.state.db,
        submission.id,
        zongce_web::domain::SubmissionStatus::NeedsRevision,
        Some("保留审核备注"),
        Some(2.5),
    )
    .await
    .unwrap();
    let submission = SubmissionRepo::find_by_id(&f.state.db, submission.id)
        .await
        .unwrap()
        .unwrap();
    let code = displayed_code(
        &f.student
            .get(&format!("/success/{}", submission.submission_no))
            .await
            .text(),
        "edit-code",
    );
    assert_eq!(query(&f, &submission, &code).await, 303);
    let attachments = AttachmentRepo::list_for_submission(&f.state.db, submission.id)
        .await
        .unwrap();
    let student_token = csrf(
        &f.student
            .get(&format!("/query/{}", submission.submission_no))
            .await
            .text(),
    );
    let response = reset(&f, &submission, 0).await;
    assert_eq!(
        response.status_code(),
        303,
        "confirmed administrator reset must exist"
    );
    let current = SubmissionRepo::find_by_id(&f.state.db, submission.id)
        .await
        .unwrap()
        .unwrap();
    let new_code = displayed_code(
        &f.admin
            .get(&format!("/admin/submissions/{}", submission.id))
            .await
            .text(),
        "admin-edit-code",
    );
    assert_ne!(new_code, code);
    assert!(!verify_secret(&current.edit_code_hash, &code));
    assert!(verify_secret(&current.edit_code_hash, &new_code));
    assert_eq!(current.status, submission.status);
    assert_eq!(current.review_note, submission.review_note);
    assert_eq!(current.approved_score, submission.approved_score);
    assert_eq!(current.result_name, submission.result_name);
    assert_eq!(current.updated_at, submission.updated_at);
    let mut business_fields = current.clone();
    business_fields.edit_code_hash = submission.edit_code_hash.clone();
    business_fields.edit_code_ciphertext = submission.edit_code_ciphertext.clone();
    business_fields.edit_code_version = submission.edit_code_version;
    assert_eq!(
        business_fields, submission,
        "reset must preserve every business field"
    );
    assert_eq!(
        AttachmentRepo::list_for_submission(&f.state.db, submission.id)
            .await
            .unwrap(),
        attachments
    );
    assert_eq!(
        f.student
            .get(&format!("/query/{}", submission.submission_no))
            .await
            .status_code(),
        403
    );
    assert_eq!(
        f.student
            .get(&format!(
                "/submissions/{}/attachments/{}",
                submission.submission_no, attachments[0].id
            ))
            .await
            .status_code(),
        403
    );
    assert_eq!(
        f.student
            .post(&format!("/query/{}/update", submission.submission_no))
            .multipart(edit_form(&student_token))
            .await
            .status_code(),
        403
    );
    assert_eq!(query(&f, &submission, &code).await, 401);
    assert_eq!(query(&f, &submission, &new_code).await, 303);
    assert_eq!(reset(&f, &submission, 0).await.status_code(), 409);
    assert_eq!(
        SubmissionRepo::find_by_id(&f.state.db, submission.id)
            .await
            .unwrap()
            .unwrap(),
        current
    );
}

#[tokio::test]
async fn legacy_original_is_unavailable_and_reset_requires_admin_csrf_and_confirmation() {
    let f = fixture().await;
    let submission = create(&f).await;
    // New schema capability is required before exercising migrated records.
    assert!(
        f.admin
            .get(&format!("/admin/submissions/{}", submission.id))
            .await
            .text()
            .contains("修改码")
    );
    sqlx::query("UPDATE submissions SET edit_code_ciphertext=NULL WHERE id=?")
        .bind(submission.id)
        .execute(&f.state.db)
        .await
        .unwrap();
    let original = SubmissionRepo::find_by_id(&f.state.db, submission.id)
        .await
        .unwrap()
        .unwrap();
    let page = f
        .admin
        .get(&format!("/admin/submissions/{}", submission.id))
        .await;
    assert!(page.text().contains("原修改码无法恢复"));
    let path = format!("/admin/submissions/{}/edit-code/reset", submission.id);
    assert_eq!(f.admin.get(&path).await.status_code(), 405);
    assert_eq!(
        f.student
            .post(&path)
            .form(&json!({"csrf_token":f.token,"confirm_reset":"yes","edit_code_version":0}))
            .await
            .status_code(),
        403
    );
    for form in [
        json!({"confirm_reset":"yes","edit_code_version":0}),
        json!({"csrf_token":f.token,"edit_code_version":0}),
        json!({"csrf_token":f.token,"confirm_reset":"yes"}),
    ] {
        assert_eq!(f.admin.post(&path).form(&form).await.status_code(), 400);
        assert_eq!(
            SubmissionRepo::find_by_id(&f.state.db, submission.id)
                .await
                .unwrap()
                .unwrap(),
            original
        );
    }
    assert_eq!(reset(&f, &submission, 0).await.status_code(), 303);
    assert_eq!(
        f.student
            .get(&format!("/success/{}", submission.submission_no))
            .await
            .status_code(),
        404,
        "an outstanding old receipt must not display an invalid code"
    );
}

#[test]
fn recovery_envelopes_reject_tampering_other_records_keys_and_versions() {
    use zongce_web::auth::EditCodeVault;
    let secret = uuid::Uuid::new_v4().as_bytes().repeat(2);
    let vault = EditCodeVault::new(&secret).unwrap();
    let code = generate_edit_code();
    let cipher = vault.encrypt("ZC2026-000001", &code);
    assert_ne!(
        cipher,
        vault.encrypt("ZC2026-000001", &code),
        "fresh nonces are required"
    );
    assert_eq!(vault.decrypt("ZC2026-000001", &cipher), Some(code));
    assert_eq!(vault.decrypt("ZC2026-000002", &cipher), None);
    let other = EditCodeVault::new(&uuid::Uuid::new_v4().as_bytes().repeat(2)).unwrap();
    assert_eq!(other.decrypt("ZC2026-000001", &cipher), None);
    for bad in [
        format!("v2:{}", &cipher[3..]),
        "v1:broken".into(),
        cipher[..cipher.len() - 3].into(),
    ] {
        assert_eq!(vault.decrypt("ZC2026-000001", &bad), None);
    }
    let mut tampered = cipher.into_bytes();
    tampered[20] = if tampered[20] == b'A' { b'B' } else { b'A' };
    assert_eq!(
        vault.decrypt("ZC2026-000001", std::str::from_utf8(&tampered).unwrap()),
        None
    );
    assert!(EditCodeVault::new(&[1; 31]).is_err());
}

#[tokio::test]
async fn bad_or_stale_envelopes_show_unavailable_without_changing_credentials() {
    let f = fixture().await;
    let submission = create(&f).await;
    let original = submission.edit_code_ciphertext.clone().unwrap();
    let second = create(&f).await;
    let other_cipher = second.edit_code_ciphertext.unwrap();
    for bad in ["v1:not-valid".to_owned(), other_cipher] {
        sqlx::query("UPDATE submissions SET edit_code_ciphertext=? WHERE id=?")
            .bind(&bad)
            .bind(submission.id)
            .execute(&f.state.db)
            .await
            .unwrap();
        let page = f
            .admin
            .get(&format!("/admin/submissions/{}", submission.id))
            .await;
        assert_eq!(page.status_code(), 200);
        assert!(page.text().contains("修改码暂时无法读取"));
        assert!(!page.text().contains(&bad));
        assert!(!page.text().contains("argon2"));
        assert_eq!(
            SubmissionRepo::find_by_id(&f.state.db, submission.id)
                .await
                .unwrap()
                .unwrap()
                .edit_code_hash,
            submission.edit_code_hash
        );
    }
    sqlx::query("UPDATE submissions SET edit_code_ciphertext=? WHERE id=?")
        .bind(&original)
        .bind(submission.id)
        .execute(&f.state.db)
        .await
        .unwrap();
    let mut wrong_config = f.config.clone();
    wrong_config.session_secret = uuid::Uuid::new_v4().as_bytes().repeat(4);
    let mut wrong_key = TestServer::new(
        zongce_web::routes::build_router_with_config(f.state.clone(), &wrong_config).unwrap(),
    )
    .unwrap();
    wrong_key.save_cookies();
    login(&wrong_key, &f.password).await;
    assert!(
        wrong_key
            .get(&format!("/admin/submissions/{}", submission.id))
            .await
            .text()
            .contains("修改码暂时无法读取")
    );
    assert_eq!(reset(&f, &submission, 0).await.status_code(), 303);
    sqlx::query("UPDATE submissions SET edit_code_ciphertext=? WHERE id=?")
        .bind(&original)
        .bind(submission.id)
        .execute(&f.state.db)
        .await
        .unwrap();
    assert!(
        f.admin
            .get(&format!("/admin/submissions/{}", submission.id))
            .await
            .text()
            .contains("修改码暂时无法读取"),
        "a genuine previous envelope does not match the current verifier"
    );
}

#[tokio::test]
async fn credential_version_guards_previously_loaded_writes_and_competing_resets() {
    use zongce_web::{
        db::AcademicYearRepo,
        validation::{SubmissionInput, validate_student_update},
    };
    let f = fixture().await;
    let original = create(&f).await;
    let year = AcademicYearRepo::find_by_id(&f.state.db, original.academic_year_id)
        .await
        .unwrap()
        .unwrap();
    let input = validate_student_update(
        SubmissionInput {
            student_name: original.student_name.clone(),
            student_no: original.student_no.clone(),
            category: original.category,
            category_data: original.category_data.clone(),
            result_name: "stale write".into(),
            obtained_date: original.obtained_date,
            detail: None,
            remark: None,
        },
        &year,
        &[],
        1,
    )
    .unwrap();
    let (first, second) = tokio::join!(reset(&f, &original, 0), reset(&f, &original, 0));
    let mut statuses = [first.status_code().as_u16(), second.status_code().as_u16()];
    statuses.sort();
    assert_eq!(statuses, [303, 409]);
    let after_reset = SubmissionRepo::find_by_id(&f.state.db, original.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after_reset.edit_code_version, 1);
    assert!(matches!(
        zongce_web::services::submissions::update_student(
            &f.state,
            &original,
            &year,
            &input,
            vec![]
        )
        .await,
        Err(zongce_web::error::AppError::Forbidden)
    ));
    assert_eq!(
        SubmissionRepo::find_by_id(&f.state.db, original.id)
            .await
            .unwrap()
            .unwrap(),
        after_reset
    );
}

#[tokio::test]
async fn reset_during_multipart_upload_rejects_old_version_before_saving_files() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let f = fixture().await;
    let cookie = &f.student_cookie;
    let submission = create(&f).await;
    let code = displayed_code(
        &f.student
            .get(&format!("/success/{}", submission.submission_no))
            .await
            .text(),
        "edit-code",
    );
    assert_eq!(query(&f, &submission, &code).await, 303);
    let token = csrf(
        &f.student
            .get(&format!("/query/{}", submission.submission_no))
            .await
            .text(),
    );
    let before_files = AttachmentRepo::list_for_submission(&f.state.db, submission.id)
        .await
        .unwrap();
    let mut body = String::new();
    for (name, value) in [
        ("csrf_token", token.as_str()),
        ("student_name", "旧会话"),
        ("student_no", "RACE-CODE"),
        ("result_name", "不应保存"),
        ("obtained_date", "2026-04-02"),
        ("category", "academic_competition"),
        ("competition_name", "竞赛"),
        ("competition_type", "A"),
        ("level", "国家"),
        ("award_level", "一等奖"),
    ] {
        body.push_str(&format!(
            "--code-boundary\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
        ));
    }
    body.push_str("--code-boundary\r\nContent-Disposition: form-data; name=\"attachments\"; filename=\"race.pdf\"\r\nContent-Type: application/pdf\r\n\r\nextra test evidence\r\n--code-boundary--\r\n");
    let mut stream = tokio::net::TcpStream::connect((
        "127.0.0.1",
        f.student.server_address().unwrap().port().unwrap(),
    ))
    .await
    .unwrap();
    stream.write_all(format!("POST /query/{}/update HTTP/1.1\r\nHost: localhost\r\nCookie: {cookie}\r\nContent-Type: multipart/form-data; boundary=code-boundary\r\nContent-Length: {}\r\nExpect: 100-continue\r\nConnection: close\r\n\r\n",submission.submission_no,body.len()).as_bytes()).await.unwrap();
    let interim = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let mut bytes = Vec::new();
        while !bytes.ends_with(b"\r\n\r\n") {
            bytes.push(stream.read_u8().await.unwrap());
        }
        String::from_utf8(bytes).unwrap()
    })
    .await
    .unwrap();
    assert!(
        interim.starts_with("HTTP/1.1 100"),
        "handler must have passed initial authorization before reset"
    );
    assert_eq!(reset(&f, &submission, 0).await.status_code(), 303);
    let after_reset = SubmissionRepo::find_by_id(&f.state.db, submission.id)
        .await
        .unwrap()
        .unwrap();
    stream.write_all(body.as_bytes()).await.unwrap();
    let mut response = String::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        stream.read_to_string(&mut response),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(response.starts_with("HTTP/1.1 403"));
    assert_eq!(
        SubmissionRepo::find_by_id(&f.state.db, submission.id)
            .await
            .unwrap()
            .unwrap(),
        after_reset
    );
    assert_eq!(
        AttachmentRepo::list_for_submission(&f.state.db, submission.id)
            .await
            .unwrap(),
        before_files
    );
    let stored_dir = f
        .state
        .storage
        .root()
        .join(format!("year-{}", submission.academic_year_id))
        .join(&submission.submission_no);
    assert_eq!(std::fs::read_dir(stored_dir).unwrap().count(), 1);
}

#[tokio::test]
async fn migration_preserves_legacy_hash_review_and_attachment_without_resetting() {
    use sqlx::{Row, migrate::Migrator};
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let all = sqlx::migrate!("./migrations");
    let old = Migrator {
        migrations: std::borrow::Cow::Owned(
            all.iter()
                .filter(|migration| migration.version < 3)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    };
    old.run(&pool).await.unwrap();
    zongce_web::db::seed_default_academic_year(&pool)
        .await
        .unwrap();
    let code = generate_edit_code();
    let hash = hash_secret(&code).unwrap();
    sqlx::query("INSERT INTO submissions (submission_no,academic_year_id,student_name,student_no,category,result_name,obtained_date,category_data,status,review_note,approved_score,edit_code_hash,created_at,updated_at) VALUES ('ZC2026-000001',1,'迁移测试','LEGACY','academic_competition','原成果','2026-04-02','{}','approved','原备注',2.5,?,'2026-04-02T00:00:00Z','2026-04-03T00:00:00Z')").bind(&hash).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO attachments (submission_id,original_name,stored_name,mime_type,file_size,created_at) VALUES (1,'old.pdf','legacy-random.pdf','application/pdf',12,'2026-04-02T00:00:00Z')").execute(&pool).await.unwrap();
    all.run(&pool).await.unwrap();
    all.run(&pool).await.unwrap();
    let row = sqlx::query("SELECT * FROM submissions WHERE id=1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("edit_code_hash"), hash);
    assert_eq!(row.get::<Option<String>, _>("edit_code_ciphertext"), None);
    assert_eq!(row.get::<i64, _>("edit_code_version"), 0);
    assert_eq!(row.get::<String, _>("status"), "approved");
    assert_eq!(row.get::<String, _>("review_note"), "原备注");
    assert_eq!(row.get::<f64, _>("approved_score"), 2.5);
    assert_eq!(row.get::<String, _>("updated_at"), "2026-04-03T00:00:00Z");
    assert!(
        zongce_web::auth::verify_student_access(&pool, "ZC2026-000001", &code)
            .await
            .is_ok()
    );
    assert_eq!(
        AttachmentRepo::list_for_submission(&pool, 1).await.unwrap()[0].stored_name,
        "legacy-random.pdf"
    );
    assert!(
        sqlx::query("UPDATE submissions SET edit_code_version=-1 WHERE id=1")
            .execute(&pool)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn legacy_scope_defaults_to_zero_and_new_scope_keeps_id_and_version_together() {
    use zongce_web::auth::{self, VERIFIED_EXPIRES_AT_KEY, VERIFIED_SUBMISSION_ID_KEY};
    let session = tower_sessions::Session::new(
        None,
        std::sync::Arc::new(tower_sessions::MemoryStore::default()),
        None,
    );
    session
        .insert(VERIFIED_SUBMISSION_ID_KEY, 12_i64)
        .await
        .unwrap();
    session
        .insert(
            VERIFIED_EXPIRES_AT_KEY,
            chrono::Utc::now() + chrono::Duration::minutes(30),
        )
        .await
        .unwrap();
    assert!(
        auth::verify_student_session_at_version(&session, 12, 0)
            .await
            .is_ok()
    );
    assert!(
        auth::verify_student_session_at_version(&session, 12, 1)
            .await
            .is_err()
    );
    let (first, second) = tokio::join!(
        auth::establish_verified_student_session_at_version(&session, 12, 1),
        auth::establish_verified_student_session_at_version(&session, 13, 2)
    );
    first.unwrap();
    second.unwrap();
    assert!(
        auth::verify_student_session_at_version(&session, 12, 2)
            .await
            .is_err()
    );
    assert!(
        auth::verify_student_session_at_version(&session, 13, 1)
            .await
            .is_err()
    );
    assert!(
        auth::verify_student_session_at_version(&session, 12, 1)
            .await
            .is_ok()
            || auth::verify_student_session_at_version(&session, 13, 2)
                .await
                .is_ok()
    );
    // A query verified before reset can finish late; its original version remains revoked.
    auth::establish_verified_student_session_at_version(&session, 12, 0)
        .await
        .unwrap();
    assert!(
        auth::verify_student_session_at_version(&session, 12, 1)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn concurrent_receipts_never_mix_a_number_with_another_code_and_are_consumed() {
    use zongce_web::auth;
    let session = tower_sessions::Session::new(
        None,
        std::sync::Arc::new(tower_sessions::MemoryStore::default()),
        None,
    );
    let first_code = generate_edit_code();
    let second_code = generate_edit_code();
    let (first, second) = tokio::join!(
        auth::establish_receipt_session(&session, "ZC2026-000001", &first_code),
        auth::establish_receipt_session(&session, "ZC2026-000002", &second_code)
    );
    first.unwrap();
    second.unwrap();
    let receipt = auth::take_receipt(&session).await.unwrap().unwrap();
    assert!(matches!(
        receipt.submission_no.as_str(),
        "ZC2026-000001" | "ZC2026-000002"
    ));
    if receipt.submission_no == "ZC2026-000001" {
        assert_eq!(receipt.edit_code, first_code);
    } else {
        assert_eq!(receipt.edit_code, second_code);
    }
    assert!(auth::take_receipt(&session).await.unwrap().is_none());
}

#[tokio::test]
async fn reset_database_failure_rolls_back_all_credential_fields() {
    let f = fixture().await;
    let submission = create(&f).await;
    sqlx::query("CREATE TRIGGER reject_reset BEFORE UPDATE OF edit_code_version ON submissions BEGIN SELECT RAISE(ABORT, 'test reset rejection'); END").execute(&f.state.db).await.unwrap();
    let response = reset(&f, &submission, 0).await;
    assert_eq!(response.status_code(), 500);
    assert!(!response.text().contains("test reset rejection"));
    assert_eq!(
        SubmissionRepo::find_by_id(&f.state.db, submission.id)
            .await
            .unwrap()
            .unwrap(),
        submission
    );
    let code = displayed_code(
        &f.admin
            .get(&format!("/admin/submissions/{}", submission.id))
            .await
            .text(),
        "admin-edit-code",
    );
    assert_eq!(query(&f, &submission, &code).await, 303);
}

#[derive(Clone)]
struct Capture(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
impl std::io::Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test(flavor = "current_thread")]
async fn create_disclosure_and_reset_logs_never_include_credentials() {
    let captured = Capture(std::sync::Arc::new(std::sync::Mutex::new(Vec::new())));
    let writer = captured.clone();
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_writer(move || writer.clone())
            .finish(),
    )
    .unwrap();
    let f = fixture().await;
    let submission = create(&f).await;
    let code = displayed_code(
        &f.admin
            .get(&format!("/admin/submissions/{}", submission.id))
            .await
            .text(),
        "admin-edit-code",
    );
    assert_eq!(query(&f, &submission, &code).await, 303);
    assert_eq!(reset(&f, &submission, 0).await.status_code(), 303);
    let new_code = displayed_code(
        &f.admin
            .get(&format!("/admin/submissions/{}", submission.id))
            .await
            .text(),
        "admin-edit-code",
    );
    let logs = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
    assert!(logs.contains("edit_code_reset"));
    assert!(logs.contains("student submission created"));
    for secret in [
        &code,
        &new_code,
        &submission.edit_code_hash,
        submission.edit_code_ciphertext.as_ref().unwrap(),
        &f.password,
        &f.token,
    ] {
        assert!(!logs.contains(secret));
    }
    assert!(!format!("{submission:?}").contains(&code));
}
