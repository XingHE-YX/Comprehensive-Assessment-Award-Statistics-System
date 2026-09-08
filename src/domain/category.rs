use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    AcademicCompetition,
    SportsArtsCompetition,
    OtherAward,
    PublishedArticle,
    SocialPractice,
    Patent,
    Certification,
}

impl Category {
    pub const ALL: [Self; 7] = [
        Self::AcademicCompetition,
        Self::SportsArtsCompetition,
        Self::OtherAward,
        Self::PublishedArticle,
        Self::SocialPractice,
        Self::Patent,
        Self::Certification,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AcademicCompetition => "academic_competition",
            Self::SportsArtsCompetition => "sports_arts_competition",
            Self::OtherAward => "other_award",
            Self::PublishedArticle => "published_article",
            Self::SocialPractice => "social_practice",
            Self::Patent => "patent",
            Self::Certification => "certification",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::AcademicCompetition => "学术科技类竞赛",
            Self::SportsArtsCompetition => "文体类比赛",
            Self::OtherAward => "其他获奖表彰",
            Self::PublishedArticle => "发表文章",
            Self::SocialPractice => "社会实践/服务",
            Self::Patent => "专利",
            Self::Certification => "学习技能/资格证书",
        }
    }

    pub const fn all() -> &'static [Self; 7] {
        &Self::ALL
    }
}

pub mod keys {
    pub const COMPETITION_NAME: &str = "competition_name";
    pub const COMPETITION_TYPE: &str = "competition_type";
    pub const CATALOG_NO: &str = "catalog_no";
    pub const LEVEL: &str = "level";
    pub const AWARD_LEVEL: &str = "award_level";
    pub const OTHER_AWARD: &str = "other_award";
    pub const HAS_AWARD_LEVEL: &str = "has_award_level";
    pub const RANK: &str = "rank";
    pub const IS_SEU_SPORTS_MEET: &str = "is_seu_sports_meet";
    pub const AWARD_NAME: &str = "award_name";
    pub const RECOGNITION_LEVEL: &str = "recognition_level";
    pub const IS_SCHOLARSHIP: &str = "is_scholarship";
    pub const SCHOOL_HONOR_CATEGORY: &str = "school_honor_category";
    pub const TITLE: &str = "title";
    pub const NATURE: &str = "nature";
    pub const PUBLICATION_TYPE: &str = "publication_type";
    pub const AUTHOR_ORDER: &str = "author_order";
    pub const JOURNAL_NAME: &str = "journal_name";
    pub const PLATFORM: &str = "platform";
    pub const PUBLICATION_FORM: &str = "publication_form";
    pub const LINK_OR_INFO: &str = "link_or_info";
    pub const PROJECT_NAME: &str = "project_name";
    pub const IDENTITY: &str = "identity";
    pub const AWARD_LEVEL_OR_NONE: &str = "award_level_or_none";
    pub const NAME: &str = "name";
    pub const TYPE: &str = "type";
    pub const STATUS: &str = "status";
    pub const RANKING: &str = "ranking";
    pub const PATENT_NO: &str = "patent_no";
    pub const CERTIFICATE_TYPE: &str = "certificate_type";
    pub const CET6_SCORE: &str = "cet6_score";
    pub const COMPUTER_CATEGORY: &str = "computer_category";
    pub const EXAM_LEVEL: &str = "exam_level";
    pub const LANGUAGE_SCORE: &str = "language_score";
    pub const QUALIFICATION_NAME: &str = "qualification_name";
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Category {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "academic_competition" => Ok(Self::AcademicCompetition),
            "sports_arts_competition" => Ok(Self::SportsArtsCompetition),
            "other_award" => Ok(Self::OtherAward),
            "published_article" => Ok(Self::PublishedArticle),
            "social_practice" => Ok(Self::SocialPractice),
            "patent" => Ok(Self::Patent),
            "certification" => Ok(Self::Certification),
            _ => Err(format!("unknown category: {value}")),
        }
    }
}
