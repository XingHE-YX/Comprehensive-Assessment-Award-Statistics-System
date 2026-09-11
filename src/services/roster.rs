use crate::{
    db::{RosterRepo, RosterStudent},
    error::AppError,
    validation::ValidationErrors,
};
use calamine::{Data, Reader, Xlsx};
use std::{
    collections::BTreeMap,
    io::{Cursor, Read},
};

pub const MAX_ROSTER_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_ROSTER_ROWS: usize = 5000;

fn invalid(message: &str) -> AppError {
    let mut errors = ValidationErrors::new();
    errors.add("roster", message);
    AppError::Validation(errors)
}

pub fn parse(name: Option<&str>, bytes: &[u8]) -> Result<Vec<RosterStudent>, AppError> {
    if bytes.len() > MAX_ROSTER_BYTES {
        return Err(invalid("名单文件不能超过 2 MiB"));
    }
    let rows = if name.is_some_and(|name| name.to_ascii_lowercase().ends_with(".xlsx")) {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|_| invalid("无法读取 Excel 文件，请使用提供的 .xlsx 模板"))?;
        if archive.len() > 1000
            || archive
                .decompressed_size()
                .is_none_or(|size| size > 8 * 1024 * 1024)
        {
            return Err(invalid("Excel 内容过大，请使用精简的名单模板"));
        }
        let mut remaining = 8 * 1024 * 1024;
        for index in 0..archive.len() {
            let entry = archive
                .by_index(index)
                .map_err(|_| invalid("Excel 压缩内容无效"))?;
            let worksheet =
                entry.name().starts_with("xl/worksheets/") && entry.name().ends_with(".xml");
            let mut content = Vec::new();
            entry
                .take(remaining + 1)
                .read_to_end(&mut content)
                .map_err(|_| invalid("Excel 压缩内容无效"))?;
            if content.len() as u64 > remaining {
                return Err(invalid("Excel 解压内容超过大小限制"));
            }
            remaining -= content.len() as u64;
            if worksheet {
                validate_sheet_bounds(&content)?;
            }
        }
        let mut workbook: Xlsx<_> = Xlsx::new(Cursor::new(bytes))
            .map_err(|_| invalid("Excel 文件无效，请使用提供的模板"))?;
        let range = workbook
            .worksheet_range_at(0)
            .ok_or_else(|| invalid("Excel 中没有工作表"))?
            .map_err(|_| invalid("无法读取名单工作表"))?;
        if range.height() > MAX_ROSTER_ROWS + 1 || range.width() > 20 {
            return Err(invalid("名单最多 5000 人，请仅保留姓名和学号列"));
        }
        range.rows().map(|row| row.iter().map(|cell| match cell {
            Data::Empty => Ok(String::new()),
            Data::String(value) => Ok(value.clone()),
            _ => Err(invalid("姓名和学号须使用文本单元格；请按模板重新填写，防止学号前导零或长数字丢失")),
        }).collect::<Result<Vec<_>, _>>()).collect::<Result<Vec<_>, _>>()?
    } else {
        if name.is_some_and(|name| !name.to_ascii_lowercase().ends_with(".csv")) {
            return Err(invalid("请上传 .xlsx 或 UTF-8 CSV 名单"));
        }
        let text = std::str::from_utf8(bytes)
            .map_err(|_| invalid("CSV 请保存为 UTF-8 编码，或直接从 Excel 复制粘贴"))?
            .trim_start_matches('\u{feff}');
        let delimiter = if text.lines().next().is_some_and(|line| line.contains('\t')) {
            b'\t'
        } else {
            b','
        };
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .delimiter(delimiter)
            .from_reader(text.as_bytes());
        reader
            .records()
            .enumerate()
            .map(|(index, row)| {
                if index > MAX_ROSTER_ROWS {
                    return Err(invalid("每次最多导入 5000 行学生名单"));
                }
                row.map(|row| row.iter().map(str::to_owned).collect())
                    .map_err(|_| invalid("名单行格式无效，请使用两列模板"))
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    normalize(rows)
}

// Reject sparse sheets before Calamine allocates a rectangular cell range.
fn validate_sheet_bounds(bytes: &[u8]) -> Result<(), AppError> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_reader(bytes);
    loop {
        match reader
            .read_event()
            .map_err(|_| invalid("Excel 工作表格式无效"))?
        {
            Event::Start(element) | Event::Empty(element)
                if element.local_name().as_ref() == b"c" =>
            {
                let mut found = false;
                for attribute in element.attributes() {
                    let attribute = attribute.map_err(|_| invalid("Excel 单元格格式无效"))?;
                    if attribute.key.as_ref() != b"r" {
                        continue;
                    }
                    found = true;
                    let reference = std::str::from_utf8(&attribute.value)
                        .map_err(|_| invalid("Excel 单元格位置无效"))?;
                    let letters = reference.bytes().take_while(u8::is_ascii_uppercase).count();
                    let (column, row) = reference.split_at(letters);
                    let row = row
                        .parse::<usize>()
                        .map_err(|_| invalid("Excel 单元格位置无效"))?;
                    if column.len() != 1
                        || column.as_bytes()[0] > b'T'
                        || row == 0
                        || row > MAX_ROSTER_ROWS + 1
                    {
                        return Err(invalid(
                            "Excel 名单范围过大，请使用模板并仅保留姓名和学号列",
                        ));
                    }
                }
                if !found {
                    return Err(invalid("Excel 单元格缺少位置，请重新另存为 .xlsx 文件"));
                }
            }
            Event::Eof => return Ok(()),
            _ => {}
        }
    }
}

fn normalize(rows: Vec<Vec<String>>) -> Result<Vec<RosterStudent>, AppError> {
    let mut rows = rows
        .into_iter()
        .filter(|row| row.iter().any(|value| !value.trim().is_empty()));
    let header = rows.next().ok_or_else(|| invalid("请先导入学生名单"))?;
    let name_col = header.iter().position(|value| value.trim() == "姓名");
    let number_col = header.iter().position(|value| value.trim() == "学号");
    let (Some(name_col), Some(number_col)) = (name_col, number_col) else {
        return Err(invalid("第一行必须包含“姓名”和“学号”两列表头"));
    };
    let mut students = BTreeMap::new();
    for (index, row) in rows.enumerate() {
        if index >= MAX_ROSTER_ROWS {
            return Err(invalid("每次最多导入 5000 行学生名单"));
        }
        let name = row.get(name_col).map_or("", String::as_str).trim();
        let number = row.get(number_col).map_or("", String::as_str).trim();
        if !(1..=50).contains(&name.chars().count())
            || !(1..=30).contains(&number.chars().count())
            || name.chars().chain(number.chars()).any(char::is_control)
        {
            return Err(invalid(&format!(
                "第 {} 行姓名或学号为空、过长或包含无效字符",
                index + 2
            )));
        }
        if students
            .insert(number.to_owned(), name.to_owned())
            .is_some_and(|old| old != name)
        {
            return Err(invalid("同一学号对应了不同姓名，请核对名单后重新导入"));
        }
    }
    if students.is_empty() {
        return Err(invalid("名单只有表头，请至少添加一名学生"));
    }
    Ok(students
        .into_iter()
        .map(|(student_no, student_name)| RosterStudent {
            student_name,
            student_no,
        })
        .collect())
}

pub async fn validate_identity(
    connection: &mut sqlx::SqliteConnection,
    year_id: i64,
    name: &str,
    number: &str,
) -> Result<(), AppError> {
    if RosterRepo::matches(connection, year_id, name, number).await? {
        return Ok(());
    }
    let mut errors = ValidationErrors::new();
    errors.add(
        "student_no",
        "姓名与学号不在本学年允许申报名单中，请核对填写内容或联系管理员补录",
    );
    Err(AppError::Validation(errors))
}

pub async fn append(
    pool: &sqlx::SqlitePool,
    year_id: i64,
    students: &[RosterStudent],
) -> Result<usize, AppError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let added = RosterRepo::append(&mut tx, year_id, students).await?;
    tx.commit().await?;
    Ok(added)
}

pub fn template() -> Result<Vec<u8>, AppError> {
    use rust_xlsxwriter::{Format, Workbook};
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    let text = Format::new().set_num_format("@");
    let header = Format::new().set_bold();
    sheet.set_name("学生名单").map_err(|_| AppError::Export)?;
    sheet
        .set_column_width(0, 24)
        .map_err(|_| AppError::Export)?;
    sheet
        .set_column_width(1, 30)
        .map_err(|_| AppError::Export)?;
    sheet
        .set_column_format(0, &text)
        .map_err(|_| AppError::Export)?;
    sheet
        .set_column_format(1, &text)
        .map_err(|_| AppError::Export)?;
    sheet
        .write_string_with_format(0, 0, "姓名", &header)
        .map_err(|_| AppError::Export)?;
    sheet
        .write_string_with_format(0, 1, "学号", &header)
        .map_err(|_| AppError::Export)?;
    workbook.save_to_buffer().map_err(|_| AppError::Export)
}
