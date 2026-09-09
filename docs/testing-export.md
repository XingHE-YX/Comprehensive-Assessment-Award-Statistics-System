# Excel export contract and verification (Task 9)

Administrators can download the current academic year from the shared navigation or the displayed filter selection beside the submissions table. Historical/all-year selection uses the existing dashboard controls. The two-sheet export includes declaration-only students, and only Approved scores enter the total. A student who declared no materials and later submitted results keeps both the results and the declaration flag.

The fixed schema comes from section 11 of the original requirements. Detail rows use the same descending submission-time/id order as the dashboard. Summary rows sort by student number, then name. All-year exports group the same number/name across years; per-record years remain visible in Detail. Declarations use only year/name/number filters. Empty selections still have two header-only sheets; a declaration-only selection has an empty Detail sheet and populated Summary.

## `申报明细` — 34 columns

| Column | Header | Source / behavior |
|---|---|---|
| A | 序号 | One-based row sequence |
| B | 申报编号 | Public submission number, text |
| C | 姓名 | Student name |
| D | 学号 | Text, including leading zeros and long identifiers |
| E | 学年 | Stored academic-year display name |
| F | 成果类别 | Chinese category label |
| G | 成果名称 | Common result name |
| H | 取得日期 | Native date, `yyyy-mm-dd` |
| I | 学科竞赛类别 | Academic competition type |
| J | 学科竞赛目录序号 | Catalog number, text |
| K | 竞赛/成果级别 | Competition/practice level, recognition level, or computer exam level |
| L | 获奖等级 | Competition award or practice award/none |
| M | 实际名次/奖项 | Academic Other award; sports actual rank when no award level |
| N | 文章性质 | 学术论文 / 非学术文章 |
| O | 发表平台/期刊 | Selected article branch's journal/platform |
| P | 作者排序 | Academic article author order |
| Q | 社会实践身份 | Practice identity |
| R | 专利类型 | Patent type |
| S | 专利状态 | Patent status |
| T | 专利成员排名 | Patent team ranking, text |
| U | 专利号/申请号 | Patent/application number, text |
| V | 证书/考试类型 | Chinese certificate type or CET-6 |
| W | 考试成绩 | CET-6/language score, numeric |
| X | 专业类别 | Computer specialization |
| Y | 资格证书具体名称 | Other qualification |
| Z | 详细说明 | Original detail plus labeled supplementary category fields |
| AA | 备注 | Student remark |
| AB | 附件数量 | Selected record's metadata count, numeric |
| AC | 附件文件名/下载标识 | Original names and protected `/submissions/<number>/attachments/<id>` identifiers, one per line |
| AD | 审核状态 | Chinese status |
| AE | 审核备注 | Administrator note |
| AF | 核定分值 | Numeric `0.00`; empty when unassigned, including for pending records |
| AG | 提交时间 | Native UTC timestamp, `yyyy-mm-dd hh:mm:ss` |
| AH | 最后修改时间 | Native UTC timestamp, `yyyy-mm-dd hh:mm:ss` |

Column Z preserves the fields that have no dedicated column in the required 34-column schema: category-specific full name/title, sports award-level/SEU flags, scholarship status and school honor category, publication type/form/link or information. These are Chinese labeled lines following the original detail; no raw JSON is emitted. Conditional branches exclude stale/unselected fields. The two academic Other-award aliases accepted by the shared validator remain supported.

## `学生汇总` — 10 columns

`序号`, `姓名`, `学号`, `成果提交数量`, `已通过数量`, `待审核数量`, `需补充数量`, `不予认定数量`, `已通过核定总分`, `是否提交无材料声明`.

Counts and scores are numeric; the last column is 是/否. Approved totals round to two decimals so `0.10 + 0.20` is exported as `0.30`; very large finite scores avoid an intermediate multiplication overflow. Non-finite stored scores return a safe generation error. Other statuses can retain a score in Detail but never contribute to the total. Both sheets have bold headers, a frozen first row, automatic filters, and explicit column widths. Text cells are written explicitly, so `=SUM(...)`, URLs and leading-zero identifiers cannot become executable formulas or lose their text identity.

Excel compatibility boundaries: dates before 1900 remain readable ISO text because Excel cannot reliably display them as native dates. A text cell exceeding 32,767 UTF-16 units (including supplementary Unicode characters) retains a prefix and an explicit Chinese truncation notice plus its protected administrator detail path. Original database text stays complete; both canonical and legacy award fields are visible at that source. These fallbacks prevent one otherwise valid record from breaking the entire export without silently discarding information.

## Repeatable checks

Rust is pinned to 1.88.0. Install Python 3 for the independent ZIP/XML inspector (`python3`; verified with 3.9.6, standard library only). It parses worksheet relationships, shared strings, styles, cells and every XML part without adding a workbook-reader Rust dependency.

```sh
cargo +1.88.0 fmt --check
cargo +1.88.0 test --test export
cargo +1.88.0 clippy --all-targets --all-features -- -D warnings
cargo +1.88.0 test --all-targets
cargo +1.88.0 build --examples
```

The 12 export integration tests cover all seven categories and article/certificate/sports branches against hand-checked column expectations; headers, types, formatting and Chinese strings; identity grouping, all review counts, zero/decimal/large finite totals; declaration-only records; current/historical/all years, no active year, combined/literal filters and dashboard download links; admin-only access and logout; protected attachment identifiers; UTF-8/safe filenames; real HTTP submission of oversized Unicode text and complete original lookup (including both legacy award aliases); pre-1900 dates; and generation/database failures without partial downloads.

Use the external Playwright 1.52.0 installation described in `testing-student-flow.md`. All browser tests start an isolated preview with temporary data and a generated password; the application gains no Node runtime dependency. Set `ZONGCE_PREVIEW_BIN` when using a custom Cargo target directory.

```sh
ZONGCE_PLAYWRIGHT_MODULE=/path/to/external/node_modules/playwright node tests/admin_browser.cjs
ZONGCE_PLAYWRIGHT_MODULE=/path/to/external/node_modules/playwright node tests/settings_browser.cjs
ZONGCE_PLAYWRIGHT_MODULE=/path/to/external/node_modules/playwright node tests/export_browser.cjs
```

The administrator suite downloads filtered/empty/current-year workbooks after student submission, amendment and approval at 320x568, 390x844, 768x1024 and 1440x900, inspecting values and permission revocation. The tablet administrator run disables JavaScript. The settings suite also downloads renamed historical years after activation changes.

The export browser suite runs the existing student regression at all four sizes, including all seven categories and conditional branches, then downloads the 17 actual submissions and 4 declarations. It verifies category coverage, edited values, attachment counts and pending-only zero totals. Sample artifacts and screenshots default to `/tmp/zongce-export-screenshots`; the main sample is `seven-categories.xlsx`. Administrator/empty samples are in `/tmp/zongce-admin-screenshots`, and historical samples in `/tmp/zongce-settings-screenshots`. These contain synthetic test data only and are not committed.

To verify application compatibility, open the sample in Excel/LibreOffice, or use an installed LibreOffice executable with an isolated profile and output directory:

```sh
soffice -env:UserInstallation=file:///tmp/zongce-lo-task9-profile --headless --convert-to xlsx --outdir /tmp/zongce-lo-task9-output /tmp/zongce-export-screenshots/seven-categories.xlsx
python3 tests/support/read_xlsx.py /tmp/zongce-lo-task9-output/seven-categories.xlsx
```

Use a fresh temporary profile/output directory when repeating the check; compare sheet names, cell values/types, filters and frozen panes before/after. LibreOffice may normalize style format spellings. Docker/Compose and production startup remain the separately tracked Task 10 / Task 1 reconciliation work.
