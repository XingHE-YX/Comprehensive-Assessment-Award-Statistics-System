use serde_json::{Map, Value};

use crate::domain::{Category, keys};

use super::ValidationErrors;

pub fn validate_category(category: Category, data: &Value) -> Result<(), ValidationErrors> {
    let mut errors = ValidationErrors::new();
    let Some(object) = data.as_object() else {
        errors.add("category_data", "类别字段必须是结构化对象");
        return Err(errors);
    };

    match category {
        Category::AcademicCompetition => validate_academic(object, &mut errors),
        Category::SportsArtsCompetition => validate_sports(object, &mut errors),
        Category::OtherAward => validate_other_award(object, &mut errors),
        Category::PublishedArticle => validate_article(object, &mut errors),
        Category::SocialPractice => validate_social(object, &mut errors),
        Category::Patent => validate_patent(object, &mut errors),
        Category::Certification => validate_certification(object, &mut errors),
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_academic(data: &Map<String, Value>, errors: &mut ValidationErrors) {
    required_text(data, errors, keys::COMPETITION_NAME, "竞赛完整名称");
    one_of(
        data,
        errors,
        keys::COMPETITION_TYPE,
        "学科竞赛类别",
        &["A+/A", "A+", "A", "B", "C", "未列入/不清楚"],
    );
    one_of(
        data,
        errors,
        keys::LEVEL,
        "竞赛级别",
        &["国家", "省部", "市", "校"],
    );
    let award = one_of(
        data,
        errors,
        keys::AWARD_LEVEL,
        "获奖等级",
        &["一等奖", "二等奖", "三等奖", "优秀/鼓励奖", "其他"],
    );
    if award == Some("其他") {
        required_text_any(
            data,
            errors,
            &[keys::OTHER_AWARD, "award_detail", "actual_award_rank"],
            "实际奖项或名次",
        );
    }
    optional_text(data, errors, keys::CATALOG_NO, "目录序号", 100);
}

fn validate_sports(data: &Map<String, Value>, errors: &mut ValidationErrors) {
    required_text(data, errors, keys::COMPETITION_NAME, "比赛完整名称");
    one_of(
        data,
        errors,
        keys::LEVEL,
        "比赛级别",
        &["国家", "省部", "市", "校", "其他/不清楚"],
    );
    let has_award = boolean_choice(data, errors, keys::HAS_AWARD_LEVEL, "是否有明确奖项等级");
    if has_award == Some(true) {
        required_text(data, errors, keys::AWARD_LEVEL, "奖项等级");
    } else if has_award == Some(false) {
        required_text(data, errors, keys::RANK, "实际名次");
    }
    optional_boolean(
        data,
        errors,
        keys::IS_SEU_SPORTS_MEET,
        "是否属于东南大学运动会相关项目",
    );
}

fn validate_other_award(data: &Map<String, Value>, errors: &mut ValidationErrors) {
    required_text(data, errors, keys::AWARD_NAME, "荣誉完整名称");
    required_text(data, errors, keys::RECOGNITION_LEVEL, "表彰级别");
    one_of(
        data,
        errors,
        keys::IS_SCHOLARSHIP,
        "是否奖学金/助学金",
        &["yes", "no", "uncertain", "是", "否", "不确定"],
    );
    required_text(data, errors, keys::SCHOOL_HONOR_CATEGORY, "校级荣誉类别");
}

fn validate_article(data: &Map<String, Value>, errors: &mut ValidationErrors) {
    required_text(data, errors, keys::TITLE, "文章标题");
    let nature = one_of(
        data,
        errors,
        keys::NATURE,
        "文章性质",
        &["academic", "non_academic", "学术论文", "非学术文章"],
    );
    if matches!(nature, Some("academic" | "学术论文")) {
        required_text(data, errors, keys::PUBLICATION_TYPE, "发表类型");
        required_text(data, errors, keys::AUTHOR_ORDER, "作者排序");
        required_text(data, errors, keys::JOURNAL_NAME, "期刊名称");
    } else if matches!(nature, Some("non_academic" | "非学术文章")) {
        required_text(data, errors, keys::PLATFORM, "发表平台");
        required_text(data, errors, keys::PUBLICATION_FORM, "发表形式");
        required_text(data, errors, keys::LINK_OR_INFO, "链接或发表信息");
    }
}

fn validate_social(data: &Map<String, Value>, errors: &mut ValidationErrors) {
    required_text(data, errors, keys::PROJECT_NAME, "项目名称");
    required_text(data, errors, keys::LEVEL, "项目级别");
    required_text(data, errors, keys::IDENTITY, "本人身份");
    required_text(
        data,
        errors,
        keys::AWARD_LEVEL_OR_NONE,
        "获奖等级或无具体等级",
    );
}

fn validate_patent(data: &Map<String, Value>, errors: &mut ValidationErrors) {
    required_text(data, errors, keys::NAME, "专利名称");
    required_text(data, errors, keys::TYPE, "专利类型");
    required_text(data, errors, keys::STATUS, "专利状态");
    required_text(data, errors, keys::RANKING, "项目组排名");
    required_text(data, errors, keys::PATENT_NO, "专利号或申请号");
}

fn validate_certification(data: &Map<String, Value>, errors: &mut ValidationErrors) {
    let certificate_type = one_of(
        data,
        errors,
        keys::CERTIFICATE_TYPE,
        "证书/考试类型",
        &[
            "CET-6",
            "computer",
            "雅思/托福",
            "other",
            "计算机",
            "其他资格证书",
        ],
    );
    match certificate_type {
        Some("CET-6") => required_number(data, errors, keys::CET6_SCORE, "CET-6 成绩"),
        Some("computer" | "计算机") => {
            required_text(data, errors, keys::COMPUTER_CATEGORY, "计算机专业类别");
            required_text(data, errors, keys::EXAM_LEVEL, "计算机考试等级");
        }
        Some("雅思/托福") => {
            required_number(data, errors, keys::LANGUAGE_SCORE, "雅思/托福成绩")
        }
        Some("other" | "其他资格证书") => {
            required_text(data, errors, keys::QUALIFICATION_NAME, "其他资格证书名称");
        }
        _ => {}
    }
}

fn required_text(data: &Map<String, Value>, errors: &mut ValidationErrors, key: &str, label: &str) {
    let Some(value) = data.get(key).and_then(Value::as_str) else {
        errors.add(key, format!("{label}为必填项"));
        return;
    };
    if value.trim().is_empty() {
        errors.add(key, format!("{label}为必填项"));
    }
}

fn required_text_any(
    data: &Map<String, Value>,
    errors: &mut ValidationErrors,
    keys: &[&str],
    label: &str,
) {
    if keys.iter().any(|key| {
        data.get(*key)
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
    }) {
        return;
    }
    errors.add(keys[0], format!("{label}为必填项"));
}

fn optional_text(
    data: &Map<String, Value>,
    errors: &mut ValidationErrors,
    key: &str,
    label: &str,
    max: usize,
) {
    if let Some(value) = data.get(key).and_then(Value::as_str)
        && value.chars().count() > max
    {
        errors.add(key, format!("{label}不能超过 {max} 个字符"));
    }
}

fn one_of<'a>(
    data: &'a Map<String, Value>,
    errors: &mut ValidationErrors,
    key: &str,
    label: &str,
    allowed: &[&str],
) -> Option<&'a str> {
    let Some(value) = data.get(key).and_then(Value::as_str) else {
        errors.add(key, format!("{label}为必填项"));
        return None;
    };
    if !allowed.contains(&value) {
        errors.add(key, format!("{label}取值无效"));
        None
    } else {
        Some(value)
    }
}

fn boolean_choice(
    data: &Map<String, Value>,
    errors: &mut ValidationErrors,
    key: &str,
    label: &str,
) -> Option<bool> {
    let Some(value) = data.get(key) else {
        errors.add(key, format!("{label}为必填项"));
        return None;
    };
    match value {
        Value::Bool(value) => Some(*value),
        Value::String(value) if matches!(value.as_str(), "yes" | "是") => Some(true),
        Value::String(value) if matches!(value.as_str(), "no" | "否") => Some(false),
        _ => {
            errors.add(key, format!("{label}取值无效"));
            None
        }
    }
}

fn optional_boolean(
    data: &Map<String, Value>,
    errors: &mut ValidationErrors,
    key: &str,
    label: &str,
) {
    if data.contains_key(key) {
        let _ = boolean_choice(data, errors, key, label);
    }
}

fn required_number(
    data: &Map<String, Value>,
    errors: &mut ValidationErrors,
    key: &str,
    label: &str,
) {
    let Some(value) = data.get(key) else {
        errors.add(key, format!("{label}为必填项"));
        return;
    };
    let valid = match value {
        Value::Number(number) => number
            .as_f64()
            .is_some_and(|number| number.is_finite() && number >= 0.0),
        Value::String(value) => value
            .trim()
            .parse::<f64>()
            .is_ok_and(|number| number.is_finite() && number >= 0.0),
        _ => false,
    };
    if !valid {
        errors.add(key, format!("{label}必须是非负数字"));
    }
}
