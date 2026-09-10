use chrono::{Duration, NaiveDate, Utc};
use serde_json::json;
use zongce_web::{
    domain::{AcademicYear, Category},
    storage::UploadInput,
    validation::{
        SubmissionInput, validate_category, validate_declaration, validate_score,
        validate_submission,
    },
};

fn year() -> AcademicYear {
    AcademicYear {
        id: 1,
        name: "2025-2026学年".to_owned(),
        start_date: NaiveDate::from_ymd_opt(2025, 8, 31).expect("date"),
        end_date: NaiveDate::from_ymd_opt(2026, 8, 28).expect("date"),
        deadline: None,
        is_active: true,
        announcement: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn uploads() -> Vec<UploadInput> {
    vec![UploadInput::new(
        "proof.pdf",
        "application/pdf",
        b"proof".to_vec(),
    )]
}

fn input(category: Category, category_data: serde_json::Value) -> SubmissionInput {
    SubmissionInput {
        student_name: "张三".to_owned(),
        student_no: "20250001".to_owned(),
        result_name: "成果名称".to_owned(),
        obtained_date: NaiveDate::from_ymd_opt(2026, 4, 1).expect("date"),
        detail: Some("详细说明".to_owned()),
        remark: None,
        category,
        category_data,
    }
}

fn valid_category_data(category: Category) -> serde_json::Value {
    match category {
        Category::AcademicCompetition => json!({
            "competition_name": "全国大学生竞赛",
            "competition_type": "A",
            "catalog_no": "1",
            "level": "国家",
            "award_level": "一等奖"
        }),
        Category::SportsArtsCompetition => json!({
            "competition_name": "校运会",
            "level": "校",
            "has_award_level": "yes",
            "award_level": "一等奖",
            "is_seu_sports_meet": "yes"
        }),
        Category::OtherAward => json!({
            "award_name": "优秀学生",
            "recognition_level": "校级",
            "is_scholarship": "no",
            "school_honor_category": "优秀学生"
        }),
        Category::PublishedArticle => json!({
            "title": "文章标题",
            "nature": "academic",
            "publication_type": "期刊",
            "author_order": "第一作者",
            "journal_name": "期刊名称"
        }),
        Category::SocialPractice => json!({
            "project_name": "志愿服务",
            "level": "校",
            "identity": "成员",
            "award_level_or_none": "无具体等级"
        }),
        Category::Patent => json!({
            "name": "发明专利",
            "type": "发明",
            "status": "已授权",
            "ranking": "1",
            "patent_no": "CN123456"
        }),
        Category::Certification => json!({
            "certificate_type": "CET-6",
            "cet6_score": "520"
        }),
    }
}

#[test]
fn every_category_accepts_a_complete_sample() {
    for category in Category::ALL {
        let result = validate_category(category, &valid_category_data(category));
        assert!(result.is_ok(), "{category} should validate: {result:?}");
    }
}

#[test]
fn combined_cet_requires_numeric_score_and_legacy_cet_remains_valid() {
    for kind in ["CET-4/CET-6", "CET-6"] {
        assert!(
            validate_category(
                Category::Certification,
                &json!({"certificate_type": kind, "cet6_score": "520"})
            )
            .is_ok()
        );
        for score in [json!(""), json!("not-a-score")] {
            assert!(
                validate_category(
                    Category::Certification,
                    &json!({"certificate_type": kind, "cet6_score": score})
                )
                .is_err()
            );
        }
    }
}

#[test]
fn school_honor_category_is_optional_for_national_and_school_awards() {
    for level in ["国家级", "校级"] {
        assert!(validate_category(Category::OtherAward, &json!({"award_name": "优秀个人", "recognition_level": level, "is_scholarship": "no"})).is_ok());
    }
    assert!(validate_category(Category::OtherAward, &json!({"award_name": "优秀个人", "recognition_level": "校级", "is_scholarship": "no", "school_honor_category": "历史自填荣誉"})).is_ok());
}

#[test]
fn category_rules_require_conditional_fields() {
    let mut data = valid_category_data(Category::AcademicCompetition);
    data["award_level"] = json!("其他");
    assert!(validate_category(Category::AcademicCompetition, &data).is_err());

    let mut data = valid_category_data(Category::PublishedArticle);
    data["nature"] = json!("academic");
    data.as_object_mut().expect("object").remove("journal_name");
    assert!(validate_category(Category::PublishedArticle, &data).is_err());

    let mut data = valid_category_data(Category::Certification);
    data["certificate_type"] = json!("computer");
    data.as_object_mut().expect("object").remove("exam_level");
    assert!(validate_category(Category::Certification, &data).is_err());

    let mut data = valid_category_data(Category::SportsArtsCompetition);
    data["has_award_level"] = json!("no");
    data.as_object_mut().expect("object").remove("rank");
    assert!(validate_category(Category::SportsArtsCompetition, &data).is_err());
}

#[test]
fn common_submission_rules_cover_identity_date_deadline_and_uploads() {
    let mut value = input(
        Category::OtherAward,
        valid_category_data(Category::OtherAward),
    );
    value.student_name = " ".to_owned();
    assert!(validate_submission(value.clone(), &year(), &uploads()).is_err());

    value = input(
        Category::OtherAward,
        valid_category_data(Category::OtherAward),
    );
    value.obtained_date = NaiveDate::from_ymd_opt(2025, 8, 30).expect("date");
    assert!(validate_submission(value.clone(), &year(), &uploads()).is_err());

    let mut closed = year();
    closed.deadline = Some(Utc::now() - Duration::minutes(1));
    assert!(validate_submission(value.clone(), &closed, &uploads()).is_err());
    assert!(validate_submission(value.clone(), &year(), &[]).is_err());

    value.obtained_date = NaiveDate::from_ymd_opt(2026, 4, 1).expect("date");
    let validated = validate_submission(value, &year(), &uploads()).expect("valid");
    assert_eq!(validated.student_name, "张三");
}

#[test]
fn declaration_does_not_require_materials_but_score_is_strict() {
    assert!(validate_declaration("张三", "20250001").is_ok());
    assert!(validate_declaration("", "20250001").is_err());
    assert!(validate_score(Some("0")).is_ok());
    assert!(validate_score(Some("1.23")).is_ok());
    assert!(validate_score(Some("1.234")).is_err());
    assert!(validate_score(Some("1.230")).is_err());
    assert!(validate_score(None).is_ok());
}
