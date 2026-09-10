use std::process::Command;

use axum_test::TestServer;
use chrono::NaiveDate;
use serde_json::{Value, json};
use zongce_web::{
    auth::{generate_edit_code, hash_secret},
    config::Config,
    db::{
        AcademicYearRepo, AttachmentRepo, DeclarationRepo, NewAcademicYear, NewAttachment,
        NewSubmission, SettingsRepo, SubmissionRepo,
    },
    domain::{Category, Submission, SubmissionStatus},
    state::AppState,
    storage::{AttachmentStorage, UploadInput},
};

struct Fixture {
    server: TestServer,
    state: AppState,
    _dir: tempfile::TempDir,
    password: String,
    year_id: i64,
    edit_code: String,
    edit_hash: String,
}

async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let mut state = AppState::initialize("sqlite::memory:").await.unwrap();
    state.storage = AttachmentStorage::new(dir.path());
    let password = generate_edit_code();
    let edit_code = generate_edit_code();
    let edit_hash = hash_secret(&edit_code).unwrap();
    let config = Config {
        app_env: "development".into(),
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        admin_username: "export-test-admin".into(),
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
        edit_code,
        edit_hash,
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

async fn login(f: &Fixture) {
    let token = csrf(&f.server.get("/admin/login").await.text());
    assert_eq!(
        f.server
            .post("/admin/login")
            .form(&json!({"csrf_token":token,"username":"export-test-admin","password":f.password}))
            .await
            .status_code(),
        303
    );
}

fn sample(f: &Fixture, seq: u32, category: Category, data: Value) -> NewSubmission {
    zongce_web::validation::validate_category(category, &data).unwrap();
    NewSubmission {
        submission_no: format!("ZC2026-{seq:06}"),
        academic_year_id: f.year_id,
        student_name: "导出测试甲".into(),
        student_no: "0001234567890123456789".into(),
        category,
        result_name: "=SUM(1,2)".into(),
        obtained_date: NaiveDate::from_ymd_opt(2026, 4, 2).unwrap(),
        detail: Some("原始说明 <中文> & 内容".into()),
        remark: Some("+备注".into()),
        category_data: data,
        edit_code_hash: f.edit_hash.clone(),
    }
}

fn academic(f: &Fixture, seq: u32) -> NewSubmission {
    sample(
        f,
        seq,
        Category::AcademicCompetition,
        json!({"competition_name":"测试竞赛","competition_type":"A","catalog_no":"0012","level":"国家","award_level":"其他","other_award":"金奖"}),
    )
}

async fn insert(
    f: &Fixture,
    input: &NewSubmission,
    status: SubmissionStatus,
    score: Option<f64>,
) -> Submission {
    let record = SubmissionRepo::insert(&f.state.db, input).await.unwrap();
    SubmissionRepo::update_review(&f.state.db, record.id, status, Some("审核说明"), score)
        .await
        .unwrap();
    sqlx::query("UPDATE submissions SET created_at = '2026-04-02T03:04:05Z', updated_at = '2026-04-03T04:05:06Z' WHERE id = ?").bind(record.id).execute(&f.state.db).await.unwrap();
    record
}

fn inspect(bytes: &[u8]) -> Value {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), bytes).unwrap();
    let output = Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/read_xlsx.py"
        ))
        .arg(file.path())
        .output()
        .expect("XLSX inspection requires Python 3 (standard library only)");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

async fn download(f: &Fixture, query: &str) -> Value {
    let response = f.server.get(&format!("/admin/export.xlsx{query}")).await;
    assert_eq!(response.status_code(), 200, "{query}: {}", response.text());
    assert_eq!(
        response.headers()["content-type"],
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    );
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    inspect(response.as_bytes())
}

fn cell<'a>(sheet: &'a Value, column: &str, row: usize) -> &'a Value {
    &sheet["rows"][row - 1][format!("{column}{row}")]["value"]
}

fn numbers(sheet: &Value) -> Vec<String> {
    (2..=sheet["rows"].as_array().unwrap().len())
        .map(|row| cell(sheet, "B", row).as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn export_requires_admin_even_with_class_or_verified_student_sessions() {
    let f = fixture().await;
    for path in [
        "/admin/export.xlsx",
        "/admin/export.xlsx?academic_year_id=bad",
    ] {
        let response = f.server.get(path).await;
        assert_eq!(response.status_code(), 303);
        assert_eq!(response.headers()["location"], "/admin/login");
    }
    let code = generate_edit_code();
    SettingsRepo::set(
        &f.state.db,
        "class_access_code_hash",
        &hash_secret(&code).unwrap(),
    )
    .await
    .unwrap();
    let token = csrf(&f.server.get("/").await.text());
    assert_eq!(
        f.server
            .post("/access")
            .form(&json!({"csrf_token":token,"access_code":code}))
            .await
            .status_code(),
        303
    );
    assert_eq!(f.server.get("/admin/export.xlsx").await.status_code(), 303);
    let s = insert(&f, &academic(&f, 1), SubmissionStatus::Pending, None).await;
    assert_eq!(
        f.server
            .post("/query")
            .form(
                &json!({"csrf_token":token,"submission_no":s.submission_no,"edit_code":f.edit_code})
            )
            .await
            .status_code(),
        303
    );
    assert_eq!(f.server.get("/admin/export.xlsx").await.status_code(), 303);
    login(&f).await;
    assert_eq!(f.server.get("/admin/export.xlsx").await.status_code(), 200);
    let token = csrf(&f.server.get("/admin").await.text());
    f.server
        .post("/admin/logout")
        .form(&json!({"csrf_token":token}))
        .await;
    assert_eq!(f.server.get("/admin/export.xlsx").await.status_code(), 303);
}

#[tokio::test]
async fn empty_export_has_two_fixed_header_sheets_with_filter_freeze_and_styles() {
    let f = fixture().await;
    login(&f).await;
    let sheets = download(&f, "").await;
    assert_eq!(sheets.as_array().unwrap().len(), 2);
    let headers = [
        vec![
            "序号",
            "申报编号",
            "姓名",
            "学号",
            "学年",
            "成果类别",
            "成果名称",
            "取得日期",
            "学科竞赛类别",
            "学科竞赛目录序号",
            "竞赛/成果级别",
            "获奖等级",
            "实际名次/奖项",
            "文章性质",
            "发表平台/期刊",
            "作者排序",
            "社会实践身份",
            "专利类型",
            "专利状态",
            "专利成员排名",
            "专利号/申请号",
            "证书/考试类型",
            "考试成绩",
            "专业类别",
            "资格证书具体名称",
            "详细说明",
            "备注",
            "附件数量",
            "附件文件名/下载标识",
            "审核状态",
            "审核备注",
            "核定分值",
            "提交时间",
            "最后修改时间",
        ],
        vec![
            "序号",
            "姓名",
            "学号",
            "成果提交数量",
            "已通过数量",
            "待审核数量",
            "需补充数量",
            "不予认定数量",
            "已通过核定总分",
            "是否提交无材料声明",
        ],
    ];
    for (index, name, end) in [(0, "申报明细", "AH"), (1, "学生汇总", "J")] {
        let sheet = &sheets[index];
        assert_eq!(sheet["name"], name);
        assert_eq!(sheet["rows"].as_array().unwrap().len(), 1);
        let row = sheet["rows"][0].as_object().unwrap();
        assert_eq!(row.len(), headers[index].len());
        for (column, header) in headers[index].iter().enumerate() {
            let label = if column < 26 {
                ((b'A' + column as u8) as char).to_string()
            } else {
                format!("A{}", (b'A' + (column - 26) as u8) as char)
            };
            assert_eq!(cell(sheet, &label, 1), header);
            assert_eq!(row[&format!("{label}1")]["bold"], true);
        }
        assert_eq!(sheet["pane"]["state"], "frozen");
        assert_eq!(sheet["pane"]["ySplit"], "1");
        assert_eq!(sheet["filter"]["ref"], format!("A1:{end}1"));
        assert!(!sheet["columns"].as_array().unwrap().is_empty());
    }
    let response = f.server.get("/admin/export.xlsx").await;
    let disposition = response.headers()["content-disposition"].to_str().unwrap();
    assert!(disposition.starts_with("attachment;"));
    assert!(disposition.contains("filename*=UTF-8''"));
    assert!(
        urlencoding::decode(disposition)
            .unwrap()
            .contains("2025-2026学年综测申报汇总.xlsx")
    );
}

#[tokio::test]
async fn seven_categories_and_conditional_branches_expand_into_typed_columns() {
    let f = fixture().await;
    type CategoryExample = (Category, Value, Vec<(&'static str, Value)>);
    let cases: Vec<CategoryExample> = vec![
        (
            Category::AcademicCompetition,
            academic(&f, 1).category_data,
            vec![
                ("I", json!("A")),
                ("J", json!("0012")),
                ("K", json!("国家")),
                ("L", json!("其他")),
                ("M", json!("金奖")),
            ],
        ),
        (
            Category::SportsArtsCompetition,
            json!({"competition_name":"文体样例","level":"校","has_award_level":false,"rank":"第六名","is_seu_sports_meet":true,"award_level":"隐藏旧值"}),
            vec![("K", json!("校")), ("M", json!("第六名"))],
        ),
        (
            Category::SportsArtsCompetition,
            json!({"competition_name":"文体奖项","level":"省部","has_award_level":"是","award_level":"二等奖","rank":"隐藏旧值"}),
            vec![("K", json!("省部")), ("L", json!("二等奖"))],
        ),
        (
            Category::OtherAward,
            json!({"award_name":"荣誉样例","recognition_level":"校级","is_scholarship":"uncertain","school_honor_category":"优秀学生"}),
            vec![("K", json!("校级"))],
        ),
        (
            Category::PublishedArticle,
            json!({"title":"学术标题","nature":"academic","publication_type":"SCI","author_order":"第一作者","journal_name":"中文期刊","platform":"隐藏旧值"}),
            vec![
                ("N", json!("学术论文")),
                ("O", json!("中文期刊")),
                ("P", json!("第一作者")),
            ],
        ),
        (
            Category::PublishedArticle,
            json!({"title":"非学术标题","nature":"非学术文章","platform":"校报","publication_form":"纸质","link_or_info":"第42期","author_order":"隐藏旧值"}),
            vec![("N", json!("非学术文章")), ("O", json!("校报"))],
        ),
        (
            Category::SocialPractice,
            json!({"project_name":"服务样例","level":"省级","identity":"负责人","award_level_or_none":"无具体等级"}),
            vec![
                ("K", json!("省级")),
                ("L", json!("无具体等级")),
                ("Q", json!("负责人")),
            ],
        ),
        (
            Category::Patent,
            json!({"name":"专利样例","type":"发明","status":"已授权","ranking":"2/5","patent_no":"00123456"}),
            vec![
                ("R", json!("发明")),
                ("S", json!("已授权")),
                ("T", json!("2/5")),
                ("U", json!("00123456")),
            ],
        ),
        (
            Category::Certification,
            json!({"certificate_type":"CET-4/CET-6","cet6_score":"525"}),
            vec![("V", json!("CET-4/CET-6")), ("W", json!(525))],
        ),
        (
            Category::Certification,
            json!({"certificate_type":"CET-6","cet6_score":620,"computer_category":"隐藏旧值"}),
            vec![("V", json!("CET-6")), ("W", json!(620))],
        ),
        (
            Category::Certification,
            json!({"certificate_type":"computer","computer_category":"非计算机专业","exam_level":"三级","cet6_score":999}),
            vec![
                ("K", json!("三级")),
                ("V", json!("计算机")),
                ("X", json!("非计算机专业")),
            ],
        ),
        (
            Category::Certification,
            json!({"certificate_type":"雅思/托福","language_score":"7.5"}),
            vec![("V", json!("雅思/托福")), ("W", json!(7.5))],
        ),
        (
            Category::Certification,
            json!({"certificate_type":"其他资格证书","qualification_name":"教师资格证"}),
            vec![("V", json!("其他资格证书")), ("Y", json!("教师资格证"))],
        ),
    ];
    for (i, (category, data, _)) in cases.iter().enumerate() {
        insert(
            &f,
            &sample(&f, i as u32 + 1, *category, data.clone()),
            SubmissionStatus::Approved,
            Some(1.25),
        )
        .await;
    }
    login(&f).await;
    let sheets = download(&f, "").await;
    let sheet = &sheets[0];
    assert_eq!(sheet["rows"].as_array().unwrap().len(), 14);
    for (i, (category, _, expected)) in cases.iter().enumerate() {
        let row = 14 - i;
        assert_eq!(
            cell(sheet, "B", row).as_str().unwrap(),
            format!("ZC2026-{:06}", i + 1)
        );
        assert_eq!(cell(sheet, "C", row), "导出测试甲");
        assert_eq!(cell(sheet, "D", row), "0001234567890123456789");
        assert_eq!(cell(sheet, "E", row), "2025-2026学年");
        assert_eq!(cell(sheet, "F", row), category.label());
        assert_eq!(cell(sheet, "G", row), "=SUM(1,2)");
        assert_eq!(cell(sheet, "AA", row), "+备注");
        assert_eq!(cell(sheet, "AD", row), "已通过");
        assert_eq!(cell(sheet, "AE", row), "审核说明");
        assert_eq!(cell(sheet, "AF", row), 1.25);
        assert_eq!(cell(sheet, "H", row), 46114.0);
        let created = cell(sheet, "AG", row).as_f64().unwrap();
        assert!((created - (46114.0 + 11045.0 / 86400.0)).abs() < 0.000001);
        let updated = cell(sheet, "AH", row).as_f64().unwrap();
        assert!((updated - (46115.0 + 14706.0 / 86400.0)).abs() < 0.000001);
        assert!(
            cell(sheet, "Z", row)
                .as_str()
                .unwrap()
                .contains("原始说明 <中文> & 内容")
        );
        for column in [
            "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y",
        ] {
            if let Some((_, value)) = expected.iter().find(|(key, _)| *key == column) {
                if let Some(number) = value.as_f64() {
                    assert_eq!(cell(sheet, column, row).as_f64(), Some(number));
                } else {
                    assert_eq!(cell(sheet, column, row), value, "{category} {column}");
                }
            } else {
                assert!(
                    cell(sheet, column, row).is_null() || cell(sheet, column, row) == "",
                    "{category} {column} must be blank"
                );
            }
        }
        let cells = sheet["rows"][row - 1].as_object().unwrap();
        assert_eq!(cells[&format!("H{row}")]["format"], "yyyy-mm-dd");
        assert_eq!(cells[&format!("AF{row}")]["format"], "0.00");
        assert_eq!(cells[&format!("AG{row}")]["format"], "yyyy-mm-dd hh:mm:ss");
        for data in cells.values() {
            assert_eq!(data["formula"], false);
        }
    }
    let text = sheets.to_string();
    assert!(!text.contains("隐藏旧值"));
    assert!(!text.contains("competition_type"));
    assert!(!text.contains(&f.edit_hash));
    for text in [
        "竞赛完整名称：测试竞赛",
        "是否属于东南大学运动会相关项目：是",
        "校级荣誉类别：优秀学生",
        "是否奖学金/助学金：不确定",
        "发表类型：SCI",
        "发表形式：纸质",
        "链接或发表信息：第42期",
    ] {
        assert!(sheets.to_string().contains(text), "{text}");
    }
}

#[tokio::test]
async fn combined_cet_create_query_edit_exports_the_latest_numeric_score() {
    use axum_test::multipart::{MultipartForm, Part};
    let f = fixture().await;
    SettingsRepo::set(
        &f.state.db,
        "class_access_code_hash",
        &hash_secret("cet-test-class").unwrap(),
    )
    .await
    .unwrap();
    let token = csrf(&f.server.get("/").await.text());
    f.server
        .post("/access")
        .form(&json!({"csrf_token": token, "access_code":"cet-test-class"}))
        .await;
    let token = csrf(&f.server.get("/submit").await.text());
    let form = |token: String, score: &str| {
        MultipartForm::new()
            .add_text("csrf_token", token)
            .add_text("student_name", "证书测试学生")
            .add_text("student_no", "CET-001")
            .add_text("result_name", "英语考试")
            .add_text("obtained_date", "2026-04-02")
            .add_text("category", "certification")
            .add_text("certificate_type", "CET-4/CET-6")
            .add_text("cet6_score", score)
    };
    let created = f
        .server
        .post("/submit")
        .multipart(
            form(token, "515").add_part(
                "attachments",
                Part::bytes(b"proof".to_vec())
                    .file_name("proof.pdf")
                    .mime_type("application/pdf"),
            ),
        )
        .await;
    assert_eq!(created.status_code(), 303);
    let receipt = f
        .server
        .get(created.headers()["location"].to_str().unwrap())
        .await
        .text();
    let code = receipt
        .split("id=\"edit-code\">")
        .nth(1)
        .unwrap()
        .split('<')
        .next()
        .unwrap();
    let number: String = sqlx::query_scalar("SELECT submission_no FROM submissions")
        .fetch_one(&f.state.db)
        .await
        .unwrap();
    let token = csrf(&f.server.get("/query").await.text());
    assert_eq!(
        f.server
            .post("/query")
            .form(&json!({"csrf_token": token, "submission_no":number, "edit_code":code}))
            .await
            .status_code(),
        303
    );
    let detail = f.server.get(&format!("/query/{number}")).await.text();
    assert!(detail.contains("value=\"515\""));
    let edited = f
        .server
        .post(&format!("/query/{number}/update"))
        .multipart(form(csrf(&detail), "525"))
        .await;
    assert_eq!(edited.status_code(), 303);
    let data: String = sqlx::query_scalar("SELECT category_data FROM submissions")
        .fetch_one(&f.state.db)
        .await
        .unwrap();
    let data: Value = serde_json::from_str(&data).unwrap();
    assert_eq!(data["certificate_type"], "CET-4/CET-6");
    assert_eq!(data["cet6_score"], "525");
    assert!(data.get("form_action").is_none());
    login(&f).await;
    let sheets = download(&f, "").await;
    assert_eq!(cell(&sheets[0], "V", 2), "CET-4/CET-6");
    assert_eq!(cell(&sheets[0], "W", 2).as_f64(), Some(525.0));
    assert_eq!(sheets[0]["rows"][1]["W2"]["type"], "n");
}

#[tokio::test]
async fn summary_unions_declarations_groups_both_identity_fields_and_sums_only_approved() {
    let f = fixture().await;
    for (seq, status, score) in [
        (1, SubmissionStatus::Approved, Some(0.1)),
        (2, SubmissionStatus::Approved, Some(0.2)),
        (3, SubmissionStatus::Pending, Some(88.0)),
        (4, SubmissionStatus::NeedsRevision, Some(99.0)),
        (5, SubmissionStatus::Rejected, Some(77.0)),
        (6, SubmissionStatus::Approved, Some(0.0)),
    ] {
        insert(&f, &academic(&f, seq), status, score).await;
    }
    let mut other = academic(&f, 7);
    other.student_name = "导出测试乙".into();
    insert(&f, &other, SubmissionStatus::Pending, None).await;
    other.submission_no = "ZC2026-000008".into();
    other.student_name = "导出测试甲".into();
    other.student_no = "0002".into();
    insert(&f, &other, SubmissionStatus::Pending, None).await;
    DeclarationRepo::upsert(
        &f.state.db,
        f.year_id,
        "导出测试甲",
        "0001234567890123456789",
    )
    .await
    .unwrap();
    DeclarationRepo::upsert(&f.state.db, f.year_id, "仅声明测试", "0003")
        .await
        .unwrap();
    login(&f).await;
    let sheets = download(&f, "").await;
    let summary = &sheets[1];
    assert_eq!(summary["rows"].as_array().unwrap().len(), 5);
    let row = (2..=5)
        .find(|r| {
            cell(summary, "B", *r) == "导出测试甲"
                && cell(summary, "C", *r) == "0001234567890123456789"
        })
        .unwrap();
    for (column, want) in [
        ("D", 6.0),
        ("E", 3.0),
        ("F", 1.0),
        ("G", 1.0),
        ("H", 1.0),
        ("I", 0.3),
    ] {
        assert_eq!(cell(summary, column, row), want);
    }
    assert_eq!(cell(summary, "J", row), "是");
    let row = (2..=5)
        .find(|r| cell(summary, "B", *r) == "仅声明测试")
        .unwrap();
    assert_eq!(cell(summary, "D", row), 0.0);
    assert_eq!(cell(summary, "I", row), 0.0);
    assert_eq!(cell(summary, "J", row), "是");
    let filtered = download(&f, "?status=approved").await;
    assert_eq!(filtered[0]["rows"].as_array().unwrap().len(), 4);
    assert_eq!(
        filtered[1]["rows"].as_array().unwrap().len(),
        3,
        "declaration-only students survive status filters"
    );
    assert_eq!(cell(&filtered[1], "D", 2), 3.0);
}

#[tokio::test]
async fn export_filters_match_dashboard_including_history_and_literal_keywords() {
    let f = fixture().await;
    let history = AcademicYearRepo::insert(
        &f.state.db,
        &NewAcademicYear {
            name: "历史/测试学年".into(),
            start_date: NaiveDate::from_ymd_opt(2024, 9, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2025, 8, 31).unwrap(),
            deadline: None,
            is_active: false,
            announcement: None,
        },
    )
    .await
    .unwrap();
    let mut input = academic(&f, 1);
    input.student_name = "导出%_\\测试".into();
    input.student_no = "000_A%\\".into();
    insert(&f, &input, SubmissionStatus::Approved, Some(2.5)).await;
    input = academic(&f, 2);
    input.student_name = "普通测试".into();
    insert(&f, &input, SubmissionStatus::Pending, None).await;
    input = sample(
        &f,
        3,
        Category::Patent,
        json!({"name":"历史专利","type":"发明","status":"已授权","ranking":"1","patent_no":"001"}),
    );
    input.academic_year_id = history.id;
    insert(&f, &input, SubmissionStatus::Approved, Some(10.0)).await;
    DeclarationRepo::upsert(&f.state.db, history.id, "历史声明测试", "HIST")
        .await
        .unwrap();
    login(&f).await;
    for (query, expected) in [
        ("".to_owned(), vec!["ZC2026-000002", "ZC2026-000001"]),
        (
            format!("?academic_year_id={}", history.id),
            vec!["ZC2026-000003"],
        ),
        (
            "?academic_year_id=".into(),
            vec!["ZC2026-000003", "ZC2026-000002", "ZC2026-000001"],
        ),
        ("?name=%25_%5C".into(), vec!["ZC2026-000001"]),
        ("?student_no=000_A%25%5C".into(), vec!["ZC2026-000001"]),
        (
            "?academic_year_id=&category=patent&status=approved".into(),
            vec!["ZC2026-000003"],
        ),
        ("?name=%27%20OR%201=1--".into(), vec![]),
        ("?name=普通&status=approved".into(), vec![]),
    ] {
        let workbook = download(&f, &query).await;
        assert_eq!(numbers(&workbook[0]), expected, "{query}");
        let dashboard = f.server.get(&format!("/admin{query}")).await.text();
        assert!(dashboard.contains(&format!("id=\"count-total\">{}<", expected.len())));
        let link = dashboard
            .split("id=\"export-filtered\" href=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .replace("&amp;", "&")
            .replace("&#38;", "&");
        assert_eq!(
            numbers(&inspect(f.server.get(&link).await.as_bytes())[0]),
            expected,
            "query {query}, rendered link {link}"
        );
    }
    let historical = download(&f, &format!("?academic_year_id={}", history.id)).await;
    assert_eq!(historical[1]["rows"].as_array().unwrap().len(), 3);
    let response = f
        .server
        .get(&format!(
            "/admin/export.xlsx?academic_year_id={}",
            history.id
        ))
        .await;
    let disposition =
        urlencoding::decode(response.headers()["content-disposition"].to_str().unwrap()).unwrap();
    assert!(disposition.contains("历史_测试学年综测申报汇总.xlsx"));
    sqlx::query("UPDATE academic_years SET is_active=0")
        .execute(&f.state.db)
        .await
        .unwrap();
    assert_eq!(numbers(&download(&f, "").await[0]).len(), 3);
}

#[tokio::test]
async fn invalid_export_filters_return_to_visible_dashboard_notice() {
    let f = fixture().await;
    login(&f).await;
    for query in [
        "status=invalid&name=保留",
        "category=invalid",
        "academic_year_id=99999",
        "academic_year_id=bad",
        "name=%00",
    ] {
        let response = f.server.get(&format!("/admin/export.xlsx?{query}")).await;
        assert_eq!(response.status_code(), 303);
        let location = response.headers()["location"].to_str().unwrap();
        assert!(location.starts_with("/admin?"));
        let page = f.server.get(location).await;
        assert!(page.text().contains("已忽略无效筛选条件"));
    }
}

#[tokio::test]
async fn attachments_export_names_and_protected_identifiers_without_storage_or_secrets() {
    let f = fixture().await;
    let s = insert(&f, &academic(&f, 1), SubmissionStatus::Pending, None).await;
    let mut urls = Vec::new();
    let mut stored_names = Vec::new();
    for (name, mime, bytes) in [
        ("证明中文.png", "image/png", b"png".as_slice()),
        ("证明材料.pdf", "application/pdf", b"pdf".as_slice()),
    ] {
        let uploads = f
            .state
            .storage
            .save_many(
                &format!("year-{}", f.year_id),
                &s.submission_no,
                0,
                vec![UploadInput {
                    original_name: name.into(),
                    mime_type: mime.into(),
                    bytes: bytes.to_vec(),
                }],
            )
            .await
            .unwrap();
        let item = &uploads[0];
        let a = AttachmentRepo::insert(
            &f.state.db,
            &NewAttachment {
                submission_id: s.id,
                original_name: item.original_name.clone(),
                stored_name: item.stored_name.clone(),
                mime_type: item.mime_type.clone(),
                file_size: item.file_size,
            },
        )
        .await
        .unwrap();
        urls.push(format!(
            "/submissions/{}/attachments/{}",
            s.submission_no, a.id
        ));
        stored_names.push(a.stored_name);
    }
    login(&f).await;
    let workbook = download(&f, "").await;
    assert_eq!(cell(&workbook[0], "AB", 2), 2.0);
    let identifiers = cell(&workbook[0], "AC", 2).as_str().unwrap();
    assert!(identifiers.contains("证明中文.png"));
    assert!(identifiers.contains("证明材料.pdf"));
    for url in &urls {
        assert!(identifiers.contains(url));
        assert_eq!(f.server.get(url).await.status_code(), 200);
    }
    let text = workbook.to_string();
    for secret in stored_names
        .iter()
        .chain([&f.edit_hash, &f.edit_code, &f.password])
    {
        assert!(!text.contains(secret));
    }
    assert!(!text.contains("/uploads"));
    let token = csrf(&f.server.get("/admin").await.text());
    f.server
        .post("/admin/logout")
        .form(&json!({"csrf_token":token}))
        .await;
    for url in &urls {
        assert_eq!(f.server.get(url).await.status_code(), 403);
    }
}

#[tokio::test]
async fn generation_and_database_errors_return_safe_chinese_responses() {
    let f = fixture().await;
    // Simulate a corrupted/non-finite stored score without bypassing database constraints.
    insert(
        &f,
        &academic(&f, 1),
        SubmissionStatus::Approved,
        Some(f64::INFINITY),
    )
    .await;
    login(&f).await;
    let response = f.server.get("/admin/export.xlsx").await;
    assert_eq!(response.status_code(), 500);
    assert!(
        response.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/html")
    );
    assert!(response.text().contains("导出失败，请稍后重试"));
    assert!(!response.headers().contains_key("content-disposition"));
    sqlx::query("DROP TABLE student_declarations")
        .execute(&f.state.db)
        .await
        .unwrap();
    let response = f.server.get("/admin/export.xlsx").await;
    assert_eq!(response.status_code(), 500);
    assert!(
        response.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/html")
    );
    assert!(response.text().contains("服务暂时不可用"));
}

#[tokio::test]
async fn oversized_student_text_exports_with_visible_source_notice_and_keeps_original() {
    use axum_test::multipart::{MultipartForm, Part};
    let f = fixture().await;
    let code = generate_edit_code();
    SettingsRepo::set(
        &f.state.db,
        "class_access_code_hash",
        &hash_secret(&code).unwrap(),
    )
    .await
    .unwrap();
    let token = csrf(&f.server.get("/").await.text());
    assert_eq!(
        f.server
            .post("/access")
            .form(&json!({"csrf_token":token,"access_code":code}))
            .await
            .status_code(),
        303
    );
    let long = "超😀".repeat(16000);
    let form = MultipartForm::new()
        .add_text("csrf_token", token)
        .add_text("has_result", "yes")
        .add_text("student_name", "长字段测试")
        .add_text("student_no", "000-LONG")
        .add_text("result_name", "长文本竞赛")
        .add_text("obtained_date", "2026-04-02")
        .add_text("category", "academic_competition")
        .add_text("competition_name", long.clone())
        .add_text("competition_type", "A")
        .add_text("level", "国家")
        .add_text("award_level", "一等奖")
        .add_part(
            "attachments",
            Part::bytes(b"%PDF-1.4 test".to_vec())
                .file_name("proof.pdf")
                .mime_type("application/pdf"),
        );
    let response = f.server.post("/submit").multipart(form).await;
    assert_eq!(response.status_code(), 303);
    let no = response.headers()["location"]
        .to_str()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap();
    let record = SubmissionRepo::find_by_no(&f.state.db, no)
        .await
        .unwrap()
        .unwrap();
    login(&f).await;
    let workbook = download(&f, "").await;
    let detail = cell(&workbook[0], "Z", 2).as_str().unwrap();
    assert!(detail.starts_with("竞赛完整名称：超😀"));
    assert!(detail.encode_utf16().count() <= 32767);
    assert!(detail.contains("内容过长，已截断"));
    let source = format!("/admin/submissions/{}", record.id);
    assert!(detail.contains(&source));
    assert!(f.server.get(&source).await.text().contains(&long));
    let stored = SubmissionRepo::find_by_no(&f.state.db, no)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.category_data["competition_name"], long);
}

#[tokio::test]
async fn dates_before_excel_epoch_remain_readable_without_breaking_historical_exports() {
    let f = fixture().await;
    let year = AcademicYearRepo::insert(
        &f.state.db,
        &NewAcademicYear {
            name: "早期日期测试学年".into(),
            start_date: NaiveDate::from_ymd_opt(1800, 1, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(1800, 12, 31).unwrap(),
            deadline: None,
            is_active: false,
            announcement: None,
        },
    )
    .await
    .unwrap();
    let mut input = academic(&f, 1);
    input.academic_year_id = year.id;
    input.obtained_date = NaiveDate::from_ymd_opt(1800, 4, 2).unwrap();
    insert(&f, &input, SubmissionStatus::Pending, None).await;
    login(&f).await;
    let workbook = download(&f, &format!("?academic_year_id={}", year.id)).await;
    assert_eq!(cell(&workbook[0], "H", 2), "1800-04-02");
    assert_eq!(workbook[0]["rows"][1]["H2"]["type"], "s");
}

#[tokio::test]
async fn large_finite_approved_scores_do_not_overflow_during_cent_rounding() {
    let f = fixture().await;
    insert(
        &f,
        &academic(&f, 1),
        SubmissionStatus::Approved,
        Some(1.0e308),
    )
    .await;
    login(&f).await;
    let workbook = download(&f, "").await;
    assert_eq!(cell(&workbook[0], "AF", 2).as_f64(), Some(1.0e308));
    assert_eq!(cell(&workbook[1], "I", 2).as_f64(), Some(1.0e308));
}

#[tokio::test]
async fn truncated_legacy_award_aliases_link_to_details_with_the_complete_original() {
    let f = fixture().await;
    let long = "旧奖项".repeat(12000);
    let mut records = Vec::new();
    for (index, alias) in ["award_detail", "actual_award_rank"]
        .into_iter()
        .enumerate()
    {
        let mut input = academic(&f, index as u32 + 1);
        input
            .category_data
            .as_object_mut()
            .unwrap()
            .remove("other_award");
        input.category_data[alias] = json!(long);
        zongce_web::validation::validate_category(input.category, &input.category_data).unwrap();
        records.push(insert(&f, &input, SubmissionStatus::Pending, None).await);
    }
    login(&f).await;
    let workbook = download(&f, "").await;
    let exported = numbers(&workbook[0]);
    for record in records {
        let row = exported
            .iter()
            .position(|no| *no == record.submission_no)
            .unwrap()
            + 2;
        let value = cell(&workbook[0], "M", row).as_str().unwrap();
        assert!(value.encode_utf16().count() <= 32767);
        assert!(value.contains("内容过长，已截断"));
        let path = format!("/admin/submissions/{}", record.id);
        assert!(value.contains(&path));
        assert!(f.server.get(&path).await.text().contains(&long));
    }
}
