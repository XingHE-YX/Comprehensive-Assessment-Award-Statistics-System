use chrono::{NaiveDate, Utc};
use serde_json::json;
use sqlx::{Row, SqlitePool, sqlite::SqlitePoolOptions};
use zongce_web::db::{
    AcademicYearRepo, AttachmentRepo, DeclarationRepo, NewAcademicYear, NewAttachment,
    NewSubmission, SettingsRepo, SubmissionFilter, SubmissionRepo, migrate,
    seed_default_academic_year,
};
use zongce_web::domain::{Category, SubmissionStatus};

async fn test_pool() -> SqlitePool {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect test database")
}

async fn migrated_pool() -> SqlitePool {
    let pool = test_pool().await;
    migrate(&pool).await.expect("run migrations");
    pool
}

fn year(name: &str, active: bool) -> NewAcademicYear {
    NewAcademicYear {
        name: name.to_owned(),
        start_date: NaiveDate::from_ymd_opt(2025, 8, 31).expect("valid date"),
        end_date: NaiveDate::from_ymd_opt(2026, 8, 28).expect("valid date"),
        deadline: None,
        is_active: active,
        announcement: Some("请按要求提交材料".to_owned()),
    }
}

#[tokio::test]
async fn migration_creates_tables_and_enforces_foreign_keys() {
    let pool = migrated_pool().await;

    let tables = sqlx::query(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name IN
         ('academic_years', 'submissions', 'attachments', 'student_declarations', 'settings')
         ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .expect("query tables");
    let names = tables
        .into_iter()
        .map(|row| row.get::<String, _>("name"))
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "academic_years",
            "attachments",
            "settings",
            "student_declarations",
            "submissions"
        ]
    );

    let result = sqlx::query(
        "INSERT INTO attachments
         (submission_id, original_name, stored_name, mime_type, file_size, created_at)
         VALUES (999, 'proof.pdf', 'random.pdf', 'application/pdf', 10, ?)",
    )
    .bind(Utc::now())
    .execute(&pool)
    .await;
    assert!(result.is_err(), "foreign key must reject orphan attachment");
}

#[tokio::test]
async fn active_year_is_unique_and_default_seed_is_idempotent() {
    let pool = migrated_pool().await;
    seed_default_academic_year(&pool)
        .await
        .expect("seed default year");
    seed_default_academic_year(&pool)
        .await
        .expect("repeat default seed");
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM academic_years WHERE name = '2025-2026学年'")
            .fetch_one(&pool)
            .await
            .expect("count seed");
    assert_eq!(count, 1);

    AcademicYearRepo::insert(&pool, &year("2024-2025学年", false))
        .await
        .expect("insert historical year");

    let active = AcademicYearRepo::current(&pool)
        .await
        .expect("load current year")
        .expect("current year");
    assert_eq!(active.name, "2025-2026学年");

    let duplicate_active = sqlx::query(
        "INSERT INTO academic_years
         (name, start_date, end_date, deadline, is_active, announcement, created_at, updated_at)
         VALUES (?, ?, ?, NULL, 1, NULL, ?, ?)",
    )
    .bind("2025-2026学年")
    .bind("2025-08-31")
    .bind("2026-08-28")
    .bind(Utc::now())
    .bind(Utc::now())
    .execute(&pool)
    .await;
    assert!(
        duplicate_active.is_err(),
        "active year partial index must be unique"
    );
}

#[tokio::test]
async fn repositories_support_submission_review_attachments_declarations_and_settings() {
    let pool = migrated_pool().await;
    let academic_year = AcademicYearRepo::insert(&pool, &year("2025-2026学年", true))
        .await
        .expect("insert year");
    let created = SubmissionRepo::insert(
        &pool,
        &NewSubmission {
            submission_no: "ZC2026-000001".to_owned(),
            academic_year_id: academic_year.id,
            student_name: "张三".to_owned(),
            student_no: "20250001".to_owned(),
            category: Category::AcademicCompetition,
            result_name: "全国大学生竞赛".to_owned(),
            obtained_date: NaiveDate::from_ymd_opt(2026, 4, 1).expect("valid date"),
            detail: Some("一等奖".to_owned()),
            remark: None,
            category_data: json!({"level": "national"}),
            edit_code_hash: "argon2id$test".to_owned(),
        },
    )
    .await
    .expect("insert submission");
    assert_eq!(created.status, SubmissionStatus::Pending);

    let found = SubmissionRepo::find_by_no(&pool, "ZC2026-000001")
        .await
        .expect("find submission")
        .expect("submission exists");
    assert_eq!(found.student_no, "20250001");
    assert_eq!(found.category, Category::AcademicCompetition);

    let duplicate = SubmissionRepo::insert(
        &pool,
        &NewSubmission {
            submission_no: "ZC2026-000001".to_owned(),
            academic_year_id: academic_year.id,
            student_name: "李四".to_owned(),
            student_no: "20250002".to_owned(),
            category: Category::Patent,
            result_name: "专利".to_owned(),
            obtained_date: NaiveDate::from_ymd_opt(2026, 4, 2).expect("valid date"),
            detail: None,
            remark: None,
            category_data: json!({}),
            edit_code_hash: "argon2id$test".to_owned(),
        },
    )
    .await;
    assert!(
        duplicate.is_err(),
        "submission numbers must be globally unique"
    );

    let listed = SubmissionRepo::list(
        &pool,
        &SubmissionFilter {
            academic_year_id: Some(academic_year.id),
            name: Some("张".to_owned()),
            student_no: None,
            category: None,
            status: Some(SubmissionStatus::Pending),
        },
    )
    .await
    .expect("list submissions");
    assert_eq!(listed.len(), 1);

    SubmissionRepo::update_review(
        &pool,
        created.id,
        SubmissionStatus::Approved,
        Some("材料清晰"),
        Some(3.5),
    )
    .await
    .expect("update review");
    let reviewed = SubmissionRepo::find_by_no(&pool, "ZC2026-000001")
        .await
        .expect("find reviewed submission")
        .expect("reviewed submission exists");
    assert_eq!(reviewed.status, SubmissionStatus::Approved);
    assert_eq!(reviewed.approved_score, Some(3.5));

    let attachment = AttachmentRepo::insert(
        &pool,
        &NewAttachment {
            submission_id: created.id,
            original_name: "proof.pdf".to_owned(),
            stored_name: "2bd2c0f7.pdf".to_owned(),
            mime_type: "application/pdf".to_owned(),
            file_size: 10,
        },
    )
    .await
    .expect("insert attachment");
    assert_eq!(
        AttachmentRepo::list_for_submission(&pool, created.id)
            .await
            .expect("list attachments")
            .len(),
        1
    );

    DeclarationRepo::upsert(&pool, academic_year.id, "张三", "20250001")
        .await
        .expect("insert declaration");
    DeclarationRepo::upsert(&pool, academic_year.id, "张三", "20250001")
        .await
        .expect("update declaration");
    let declaration_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM student_declarations WHERE academic_year_id = ? AND student_no = ?",
    )
    .bind(academic_year.id)
    .bind("20250001")
    .fetch_one(&pool)
    .await
    .expect("count declarations");
    assert_eq!(declaration_count, 1);

    SettingsRepo::set(&pool, "class_access_code_hash", "argon2id$test")
        .await
        .expect("set setting");
    assert_eq!(
        SettingsRepo::get(&pool, "class_access_code_hash")
            .await
            .expect("get setting")
            .as_deref(),
        Some("argon2id$test")
    );

    sqlx::query("DELETE FROM submissions WHERE id = ?")
        .bind(created.id)
        .execute(&pool)
        .await
        .expect("delete submission");
    let attachment_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM attachments WHERE id = ?")
        .bind(attachment.id)
        .fetch_one(&pool)
        .await
        .expect("count attachments");
    assert_eq!(
        attachment_count, 0,
        "attachment must cascade with submission"
    );
}

#[tokio::test]
async fn transaction_rolls_back_submission_and_attachment_together() {
    let pool = migrated_pool().await;
    let academic_year = AcademicYearRepo::insert(&pool, &year("2025-2026学年", true))
        .await
        .expect("insert year");
    let mut transaction = pool.begin().await.expect("begin transaction");
    sqlx::query(
        "INSERT INTO submissions
         (submission_no, academic_year_id, student_name, student_no, category, result_name,
          obtained_date, category_data, edit_code_hash, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("ZC2026-000002")
    .bind(academic_year.id)
    .bind("李四")
    .bind("20250002")
    .bind(Category::Patent.as_str())
    .bind("实用新型专利")
    .bind("2026-05-01")
    .bind("{}")
    .bind("argon2id$test")
    .bind(Utc::now())
    .bind(Utc::now())
    .execute(&mut *transaction)
    .await
    .expect("insert in transaction");
    sqlx::query(
        "INSERT INTO attachments
         (submission_id, original_name, stored_name, mime_type, file_size, created_at)
         VALUES (last_insert_rowid(), ?, ?, ?, ?, ?)",
    )
    .bind("proof.pdf")
    .bind("rollback.pdf")
    .bind("application/pdf")
    .bind(10_i64)
    .bind(Utc::now())
    .execute(&mut *transaction)
    .await
    .expect("insert attachment in transaction");
    transaction.rollback().await.expect("rollback transaction");

    assert!(
        SubmissionRepo::find_by_no(&pool, "ZC2026-000002")
            .await
            .expect("query rolled back submission")
            .is_none()
    );
    let attachments: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM attachments WHERE stored_name = 'rollback.pdf'")
            .fetch_one(&pool)
            .await
            .expect("query rolled back attachment");
    assert_eq!(attachments, 0);
}
