use std::{borrow::Cow, collections::BTreeMap};

use chrono::Datelike;

use rust_xlsxwriter::{ExcelDateTime, Format, Workbook, Worksheet, XlsxError};
use serde_json::Value;

use super::ExportData;
use crate::domain::{Category, Submission, SubmissionStatus, keys};

// This order is the 34-column contract in the original requirements, section 11.
#[derive(Clone, Copy)]
#[repr(usize)]
enum Column {
    Sequence,
    SubmissionNo,
    StudentName,
    StudentNo,
    Year,
    Category,
    ResultName,
    ObtainedDate,
    CompetitionType,
    CatalogNo,
    Level,
    AwardLevel,
    ActualAward,
    ArticleNature,
    Platform,
    AuthorOrder,
    PracticeIdentity,
    PatentType,
    PatentStatus,
    PatentRank,
    PatentNo,
    CertificateType,
    ExamScore,
    ComputerCategory,
    Qualification,
    Detail,
    Remark,
    AttachmentCount,
    Attachments,
    ReviewStatus,
    ReviewNote,
    ApprovedScore,
    CreatedAt,
    UpdatedAt,
}

const DETAIL_HEADERS: [&str; 34] = [
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
];
const SUMMARY_HEADERS: [&str; 10] = [
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
];

#[derive(Default)]
enum Cell {
    #[default]
    Blank,
    Text(String),
    Number(f64),
    Score(f64),
    Date(String),
    Timestamp(String),
}
impl From<&str> for Cell {
    fn from(value: &str) -> Self {
        if value.is_empty() {
            Self::Blank
        } else {
            Self::Text(value.to_owned())
        }
    }
}

struct Formats {
    text: Format,
    score: Format,
    date: Format,
    timestamp: Format,
}
impl Formats {
    fn new() -> Self {
        Self {
            text: Format::new().set_text_wrap(),
            score: Format::new().set_num_format("0.00"),
            date: Format::new().set_num_format("yyyy-mm-dd"),
            timestamp: Format::new().set_num_format("yyyy-mm-dd hh:mm:ss"),
        }
    }
}

pub fn export_xlsx(data: &ExportData) -> Result<Vec<u8>, XlsxError> {
    let mut workbook = Workbook::new();
    let formats = Formats::new();
    let sheet = workbook.add_worksheet().set_name("申报明细")?;
    prepare_sheet(sheet, &DETAIL_HEADERS, data.submissions.len())?;
    for (column, width) in [
        (0, 8),
        (1, 21),
        (3, 26),
        (4, 24),
        (5, 24),
        (6, 36),
        (7, 14),
        (20, 26),
        (25, 60),
        (26, 40),
        (28, 64),
        (30, 40),
        (32, 24),
        (33, 24),
    ] {
        sheet.set_column_width(column, width)?;
    }
    let years: BTreeMap<_, _> = data.years.iter().map(|y| (y.id, y.name.as_str())).collect();
    for (index, submission) in data.submissions.iter().enumerate() {
        let row = (index + 1) as u32;
        let mut cells = detail_cells(data, &years, submission, row);
        let source = format!("/admin/submissions/{}", submission.id);
        for cell in &mut cells {
            if let Cell::Text(value) = cell
                && let Cow::Owned(bounded) = bounded_text(value, &source)
            {
                *value = bounded;
            }
        }
        write_row(sheet, row, &cells, &formats)?;
    }

    let summary = summarize(data);
    let sheet = workbook.add_worksheet().set_name("学生汇总")?;
    prepare_sheet(sheet, &SUMMARY_HEADERS, summary.len())?;
    sheet.set_column_width(0, 8)?;
    sheet.set_column_width(2, 26)?;
    sheet.set_column_width(8, 22)?;
    sheet.set_column_width(9, 26)?;
    for (index, ((number, name), student)) in summary.into_iter().enumerate() {
        let row = (index + 1) as u32;
        let [submitted, approved, pending, revision, rejected] = student.counts;
        let cells = [
            Cell::Number(row as f64),
            Cell::Text(name),
            Cell::Text(number),
            Cell::Number(submitted as f64),
            Cell::Number(approved as f64),
            Cell::Number(pending as f64),
            Cell::Number(revision as f64),
            Cell::Number(rejected as f64),
            Cell::Score(round_score(student.approved_total)),
            Cell::from(if student.declared { "是" } else { "否" }),
        ];
        write_row(sheet, row, &cells, &formats)?;
    }
    workbook.save_to_buffer()
}

fn prepare_sheet(sheet: &mut Worksheet, headers: &[&str], rows: usize) -> Result<(), XlsxError> {
    // Reserve row 1 for headers; do not truncate oversized result sets.
    if rows >= 1_048_576 {
        return Err(XlsxError::RowColumnLimitError);
    }
    let header = Format::new().set_bold().set_text_wrap();
    for (column, text) in headers.iter().enumerate() {
        sheet.write_string_with_format(0, column as u16, *text, &header)?;
        sheet.set_column_width(column as u16, 18)?;
    }
    sheet.set_freeze_panes(1, 0)?;
    sheet.autofilter(0, 0, rows as u32, headers.len() as u16 - 1)?;
    Ok(())
}

fn write_row(
    sheet: &mut Worksheet,
    row: u32,
    cells: &[Cell],
    formats: &Formats,
) -> Result<(), XlsxError> {
    for (column, cell) in cells.iter().enumerate() {
        let column = column as u16;
        match cell {
            Cell::Blank => {}
            // Explicit string writes preserve leading zeros and never create formulas/links.
            Cell::Text(value) => {
                sheet.write_string_with_format(row, column, value, &formats.text)?;
            }
            Cell::Number(value) => {
                ensure_finite(*value)?;
                sheet.write_number(row, column, *value)?;
            }
            Cell::Score(value) => {
                ensure_finite(*value)?;
                sheet.write_number_with_format(row, column, *value, &formats.score)?;
            }
            Cell::Date(value) | Cell::Timestamp(value) => {
                let format = if matches!(cell, Cell::Date(_)) {
                    &formats.date
                } else {
                    &formats.timestamp
                };
                let date = ExcelDateTime::parse_from_str(value)?;
                sheet.write_datetime_with_format(row, column, &date, format)?;
            }
        }
    }
    Ok(())
}

fn ensure_finite(value: f64) -> Result<(), XlsxError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(XlsxError::ParameterError("导出数值无效".into()))
    }
}

fn round_score(value: f64) -> f64 {
    let cents = value * 100.0;
    if cents.is_finite() {
        cents.round() / 100.0
    } else {
        value
    }
}

fn bounded_text<'a>(value: &'a str, source: &str) -> Cow<'a, str> {
    const LIMIT: usize = 32767;
    if value.encode_utf16().count() <= LIMIT {
        return Cow::Borrowed(value);
    }
    let notice = format!("\n[内容过长，已截断；完整内容见 {source}]");
    let available = LIMIT - notice.encode_utf16().count();
    let mut units = 0;
    let mut prefix: String = value
        .chars()
        .take_while(|c| {
            units += c.len_utf16();
            units <= available
        })
        .collect();
    prefix.push_str(&notice);
    Cow::Owned(prefix)
}

fn detail_cells(
    data: &ExportData,
    years: &BTreeMap<i64, &str>,
    s: &Submission,
    sequence: u32,
) -> [Cell; 34] {
    use Column as C;
    let mut cells = std::array::from_fn(|_| Cell::Blank);
    for (column, value) in [
        (C::SubmissionNo, s.submission_no.as_str()),
        (C::StudentName, &s.student_name),
        (C::StudentNo, &s.student_no),
        (
            C::Year,
            years.get(&s.academic_year_id).copied().unwrap_or(""),
        ),
        (C::Category, s.category.label()),
        (C::ResultName, &s.result_name),
        (C::Remark, s.remark.as_deref().unwrap_or("")),
        (C::ReviewStatus, s.status.label()),
        (C::ReviewNote, s.review_note.as_deref().unwrap_or("")),
    ] {
        cells[column as usize] = value.into();
    }
    cells[C::Sequence as usize] = Cell::Number(sequence as f64);
    // Excel's 1900 date system cannot display earlier dates reliably. Settings
    // allow years 0001–9999, so retain an early date as readable ISO text.
    cells[C::ObtainedDate as usize] = if s.obtained_date.year() < 1900 {
        Cell::Text(s.obtained_date.to_string())
    } else {
        Cell::Date(s.obtained_date.to_string())
    };
    cells[C::CreatedAt as usize] =
        Cell::Timestamp(s.created_at.format("%Y-%m-%dT%H:%M:%S").to_string());
    cells[C::UpdatedAt as usize] =
        Cell::Timestamp(s.updated_at.format("%Y-%m-%dT%H:%M:%S").to_string());
    cells[C::ApprovedScore as usize] = s.approved_score.map_or(Cell::Blank, Cell::Score);
    let attachments = data.attachments.get(&s.id).map_or(&[][..], Vec::as_slice);
    cells[C::AttachmentCount as usize] = Cell::Number(attachments.len() as f64);
    let identifiers = attachments
        .iter()
        .map(|a| {
            format!(
                "{} — /submissions/{}/attachments/{}",
                a.original_name, s.submission_no, a.id,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    cells[C::Attachments as usize] = identifiers.as_str().into();
    expand_category(s, &mut cells);
    cells
}

fn text<'a>(data: &'a Value, key: &str) -> &'a str {
    data.get(key).and_then(Value::as_str).unwrap_or("")
}

fn choice(value: &Value) -> &str {
    match value {
        Value::Bool(true) => "是",
        Value::Bool(false) => "否",
        Value::String(value) => match value.as_str() {
            "yes" | "是" => "是",
            "no" | "否" => "否",
            "uncertain" | "不确定" => "不确定",
            _ => "",
        },
        _ => "",
    }
}

fn supplement(detail: &mut Vec<String>, label: &str, value: &str) {
    if !value.is_empty() {
        detail.push(format!("{label}：{value}"));
    }
}

fn expand_category(s: &Submission, cells: &mut [Cell; 34]) {
    use Column as C;
    let d = &s.category_data;
    let mut detail = Vec::new();
    if let Some(value) = &s.detail {
        detail.push(value.clone());
    }
    let mut mapping = Vec::new();
    match s.category {
        Category::AcademicCompetition => {
            mapping.extend([
                (C::CompetitionType, keys::COMPETITION_TYPE),
                (C::CatalogNo, keys::CATALOG_NO),
                (C::Level, keys::LEVEL),
                (C::AwardLevel, keys::AWARD_LEVEL),
            ]);
            if text(d, keys::AWARD_LEVEL) == "其他" {
                // The shared validator also accepts these legacy aliases.
                let award = [keys::OTHER_AWARD, "award_detail", "actual_award_rank"]
                    .into_iter()
                    .map(|key| text(d, key))
                    .find(|v| !v.trim().is_empty())
                    .unwrap_or("");
                cells[C::ActualAward as usize] = award.into();
            }
            supplement(&mut detail, "竞赛完整名称", text(d, keys::COMPETITION_NAME));
        }
        Category::SportsArtsCompetition => {
            mapping.push((C::Level, keys::LEVEL));
            match choice(&d[keys::HAS_AWARD_LEVEL]) {
                "是" => mapping.push((C::AwardLevel, keys::AWARD_LEVEL)),
                "否" => mapping.push((C::ActualAward, keys::RANK)),
                _ => {}
            }
            supplement(&mut detail, "比赛完整名称", text(d, keys::COMPETITION_NAME));
            supplement(
                &mut detail,
                "是否有明确奖项等级",
                choice(&d[keys::HAS_AWARD_LEVEL]),
            );
            supplement(
                &mut detail,
                "是否属于东南大学运动会相关项目",
                choice(&d[keys::IS_SEU_SPORTS_MEET]),
            );
        }
        Category::OtherAward => {
            mapping.push((C::Level, keys::RECOGNITION_LEVEL));
            supplement(&mut detail, "荣誉完整名称", text(d, keys::AWARD_NAME));
            supplement(
                &mut detail,
                "是否奖学金/助学金",
                choice(&d[keys::IS_SCHOLARSHIP]),
            );
            supplement(
                &mut detail,
                "校级荣誉类别",
                text(d, keys::SCHOOL_HONOR_CATEGORY),
            );
        }
        Category::PublishedArticle => {
            supplement(&mut detail, "文章标题", text(d, keys::TITLE));
            match text(d, keys::NATURE) {
                "academic" | "学术论文" => {
                    cells[C::ArticleNature as usize] = "学术论文".into();
                    mapping.extend([
                        (C::Platform, keys::JOURNAL_NAME),
                        (C::AuthorOrder, keys::AUTHOR_ORDER),
                    ]);
                    supplement(&mut detail, "发表类型", text(d, keys::PUBLICATION_TYPE));
                }
                "non_academic" | "非学术文章" => {
                    cells[C::ArticleNature as usize] = "非学术文章".into();
                    mapping.push((C::Platform, keys::PLATFORM));
                    supplement(&mut detail, "发表形式", text(d, keys::PUBLICATION_FORM));
                    supplement(&mut detail, "链接或发表信息", text(d, keys::LINK_OR_INFO));
                }
                _ => {}
            }
        }
        Category::SocialPractice => {
            mapping.extend([
                (C::Level, keys::LEVEL),
                (C::PracticeIdentity, keys::IDENTITY),
                (C::AwardLevel, keys::AWARD_LEVEL_OR_NONE),
            ]);
            supplement(&mut detail, "项目名称", text(d, keys::PROJECT_NAME));
        }
        Category::Patent => {
            mapping.extend([
                (C::PatentType, keys::TYPE),
                (C::PatentStatus, keys::STATUS),
                (C::PatentRank, keys::RANKING),
                (C::PatentNo, keys::PATENT_NO),
            ]);
            supplement(&mut detail, "专利名称", text(d, keys::NAME));
        }
        Category::Certification => {
            let kind = text(d, keys::CERTIFICATE_TYPE);
            cells[C::CertificateType as usize] = match kind {
                "computer" | "计算机" => "计算机",
                "other" | "其他资格证书" => "其他资格证书",
                _ => kind,
            }
            .into();
            match kind {
                "CET-6" | "雅思/托福" => {
                    let key = if kind == "CET-6" {
                        keys::CET6_SCORE
                    } else {
                        keys::LANGUAGE_SCORE
                    };
                    let number = d[key]
                        .as_f64()
                        .or_else(|| text(d, key).trim().parse::<f64>().ok());
                    cells[C::ExamScore as usize] = number.map_or(Cell::Blank, Cell::Number);
                }
                "computer" | "计算机" => {
                    mapping.extend([
                        (C::ComputerCategory, keys::COMPUTER_CATEGORY),
                        (C::Level, keys::EXAM_LEVEL),
                    ]);
                }
                "other" | "其他资格证书" => {
                    mapping.push((C::Qualification, keys::QUALIFICATION_NAME))
                }
                _ => {}
            }
        }
    }
    for (column, key) in mapping {
        cells[column as usize] = text(d, key).into();
    }
    // Preserve fields absent from the fixed schema as labeled prose, never raw JSON.
    cells[C::Detail as usize] = detail.join("\n").as_str().into();
}

#[derive(Default)]
struct StudentSummary {
    counts: [usize; 5],
    approved_total: f64,
    declared: bool,
}

fn summarize(data: &ExportData) -> BTreeMap<(String, String), StudentSummary> {
    let mut students = BTreeMap::<_, StudentSummary>::new();
    for s in &data.submissions {
        let student = students
            .entry((s.student_no.clone(), s.student_name.clone()))
            .or_default();
        student.counts[0] += 1;
        let status_index = match s.status {
            SubmissionStatus::Approved => 1,
            SubmissionStatus::Pending => 2,
            SubmissionStatus::NeedsRevision => 3,
            SubmissionStatus::Rejected => 4,
        };
        student.counts[status_index] += 1;
        if s.status == SubmissionStatus::Approved {
            // Round the final total, without overflowing valid large scores via ×100.
            student.approved_total += s.approved_score.unwrap_or(0.0);
        }
    }
    for d in &data.declarations {
        if !d.has_submission_material {
            students
                .entry((d.student_no.clone(), d.student_name.clone()))
                .or_default()
                .declared = true;
        }
    }
    students
}
