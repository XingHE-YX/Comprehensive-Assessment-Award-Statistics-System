use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
