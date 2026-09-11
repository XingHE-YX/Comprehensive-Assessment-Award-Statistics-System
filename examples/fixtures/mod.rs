pub async fn seed(pool: &sqlx::SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    zongce_web::db::seed_default_academic_year(pool).await?;
    let year = zongce_web::db::AcademicYearRepo::current(pool)
        .await?
        .ok_or("missing preview year")?;
    let mut students = Vec::new();
    for width in [320, 390, 768, 1440] {
        for (name, prefix) in [
            ("测试学生", "TEST"),
            ("无材料测试学生", "TEST-NONE"),
            ("后台流程测试", "ADMIN-TEST"),
            ("后台声明测试", "ADMIN-NONE"),
        ] {
            students.push(zongce_web::db::RosterStudent {
                student_name: name.into(),
                student_no: format!("{prefix}-{width}"),
            });
        }
    }
    zongce_web::services::roster::append(pool, year.id, &students).await?;
    Ok(())
}
