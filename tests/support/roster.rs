pub async fn add(pool: &sqlx::SqlitePool, year_id: i64, identities: &[(&str, &str)]) {
    let students = identities
        .iter()
        .map(|(name, number)| zongce_web::db::RosterStudent {
            student_name: (*name).into(),
            student_no: (*number).into(),
        })
        .collect::<Vec<_>>();
    zongce_web::services::roster::append(pool, year_id, &students)
        .await
        .unwrap();
}

pub async fn seed(pool: &sqlx::SqlitePool, identities: &[(&str, &str)]) {
    zongce_web::db::seed_default_academic_year(pool)
        .await
        .unwrap();
    let year = zongce_web::db::AcademicYearRepo::current(pool)
        .await
        .unwrap()
        .unwrap();
    add(pool, year.id, identities).await;
}
