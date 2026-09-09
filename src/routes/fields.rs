use crate::domain::Category;

pub(super) struct Field {
    pub key: &'static str,
    pub label: &'static str,
    pub options: &'static [(&'static str, &'static str)],
    pub required: bool,
    pub when_key: &'static str,
    pub when_value: &'static str,
}

impl Field {
    fn text(key: &'static str, label: &'static str) -> Self {
        Self {
            key,
            label,
            options: &[],
            required: true,
            when_key: "",
            when_value: "",
        }
    }
    fn optional(mut self) -> Self {
        self.required = false;
        self
    }
    fn options(mut self, options: &'static [(&'static str, &'static str)]) -> Self {
        self.options = options;
        self
    }
    fn when(mut self, key: &'static str, value: &'static str) -> Self {
        self.when_key = key;
        self.when_value = value;
        self
    }
}

pub(super) struct CategorySection {
    pub category: Category,
    pub fields: Vec<Field>,
}

pub(super) fn category_sections() -> Vec<CategorySection> {
    use Field as F;
    const LEVELS: &[(&str, &str)] = &[
        ("国家", "国家"),
        ("省部", "省部"),
        ("市", "市"),
        ("校", "校"),
    ];
    const YES_NO: &[(&str, &str)] = &[("yes", "是"), ("no", "否")];
    vec![
        CategorySection {
            category: Category::AcademicCompetition,
            fields: vec![
                F::text("competition_name", "竞赛完整名称"),
                F::text("competition_type", "学科竞赛类别").options(&[
                    ("A+/A", "A+/A"),
                    ("A+", "A+"),
                    ("A", "A"),
                    ("B", "B"),
                    ("C", "C"),
                    ("未列入/不清楚", "未列入/不清楚"),
                ]),
                F::text("catalog_no", "目录序号").optional(),
                F::text("level", "竞赛级别").options(LEVELS),
                F::text("award_level", "获奖等级").options(&[
                    ("一等奖", "一等奖"),
                    ("二等奖", "二等奖"),
                    ("三等奖", "三等奖"),
                    ("优秀/鼓励奖", "优秀/鼓励奖"),
                    ("其他", "其他"),
                ]),
                F::text("other_award", "实际奖项或名次").when("award_level", "其他"),
            ],
        },
        CategorySection {
            category: Category::SportsArtsCompetition,
            fields: vec![
                F::text("competition_name", "比赛完整名称"),
                F::text("level", "比赛级别").options(&[
                    ("国家", "国家"),
                    ("省部", "省部"),
                    ("市", "市"),
                    ("校", "校"),
                    ("其他/不清楚", "其他/不清楚"),
                ]),
                F::text("has_award_level", "是否有明确奖项等级").options(YES_NO),
                F::text("award_level", "奖项等级").when("has_award_level", "yes"),
                F::text("rank", "实际名次").when("has_award_level", "no"),
                F::text("is_seu_sports_meet", "是否属于东南大学运动会相关项目")
                    .options(YES_NO)
                    .optional(),
            ],
        },
        CategorySection {
            category: Category::OtherAward,
            fields: vec![
                F::text("award_name", "荣誉完整名称"),
                F::text("recognition_level", "表彰级别"),
                F::text("school_honor_category", "校级荣誉类别"),
                F::text("is_scholarship", "是否奖学金/助学金").options(&[
                    ("yes", "是"),
                    ("no", "否"),
                    ("uncertain", "不确定"),
                ]),
            ],
        },
        CategorySection {
            category: Category::PublishedArticle,
            fields: vec![
                F::text("title", "文章标题"),
                F::text("nature", "文章性质")
                    .options(&[("academic", "学术论文"), ("non_academic", "非学术文章")]),
                F::text("publication_type", "发表类型").when("nature", "academic"),
                F::text("author_order", "作者排序").when("nature", "academic"),
                F::text("journal_name", "期刊名称").when("nature", "academic"),
                F::text("platform", "发表平台").when("nature", "non_academic"),
                F::text("publication_form", "发表形式").when("nature", "non_academic"),
                F::text("link_or_info", "链接或发表信息").when("nature", "non_academic"),
            ],
        },
        CategorySection {
            category: Category::SocialPractice,
            fields: vec![
                F::text("project_name", "项目名称"),
                F::text("level", "项目级别"),
                F::text("identity", "本人身份"),
                F::text("award_level_or_none", "获奖等级或无具体等级"),
            ],
        },
        CategorySection {
            category: Category::Patent,
            fields: vec![
                F::text("name", "专利名称"),
                F::text("type", "专利类型"),
                F::text("status", "专利状态"),
                F::text("ranking", "项目组排名"),
                F::text("patent_no", "专利号或申请号"),
            ],
        },
        CategorySection {
            category: Category::Certification,
            fields: vec![
                F::text("certificate_type", "证书/考试类型").options(&[
                    ("CET-6", "CET-6"),
                    ("computer", "计算机"),
                    ("雅思/托福", "雅思/托福"),
                    ("other", "其他资格证书"),
                ]),
                F::text("cet6_score", "CET-6 成绩").when("certificate_type", "CET-6"),
                F::text("computer_category", "计算机专业类别").when("certificate_type", "computer"),
                F::text("exam_level", "计算机考试等级").when("certificate_type", "computer"),
                F::text("language_score", "雅思/托福成绩").when("certificate_type", "雅思/托福"),
                F::text("qualification_name", "其他资格证书名称").when("certificate_type", "other"),
            ],
        },
    ]
}
