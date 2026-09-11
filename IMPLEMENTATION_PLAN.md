# 综测成果申报系统实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 按 `PRD.md`、`APP_FLOW.md`、`TECH_STACK.md`、`FRONTEND_GUIDELINES.md` 和 `BACKEND_STRUCTURE.md` 构建可上线的单班级综测成果申报系统。

**Architecture:** Axum + Askama 服务端渲染单体；SQLite 保存业务和哈希；上传文件独立持久化；原生 JavaScript 只做条件字段；Caddy 提供 HTTPS。先完成可运行的学生纵向流程，再接入管理员审核、Excel、部署和备份。

**Tech Stack:** 以 `TECH_STACK.md` 的精确版本为唯一基线。

**Spec:** 本目录的五份规范文档。

## 0. 开工约束

- 先创建 `Cargo.toml`、`src/`、`templates/`、`static/`、`migrations/`、`tests/`。
- 每个任务先写失败测试，再写最小实现，再运行定向测试。
- 每个任务结束后运行 `cargo fmt`、定向测试和 `cargo clippy --all-targets --all-features -- -D warnings`。
- 所有写操作使用事务；所有 SQL 参数化；所有模板自动转义。
- 不引入 Node、React、Vue、Redis、PostgreSQL、对象存储或 P2 功能。

## 1. 任务序列

### Task 1: 工程骨架和配置

Files: `Cargo.toml`, `src/main.rs`, `src/config.rs`, `src/state.rs`, `src/error.rs`, `templates/layout.html`, `templates/errors/*.html`, `.env.example`, `README.md`, `tests/health.rs`。

- [x] 写 `GET /healthz` 测试和缺失环境变量测试。
- [x] 固定 `TECH_STACK.md` 中的依赖版本，定义 `Config`、`AppState`、`AppError`。
- [x] 初始化 tracing、SQLite pool、Askama、路由和 graceful shutdown。
- [x] 运行 `cargo test --test health`、`cargo fmt --check`、clippy。

完成条件：程序可启动，`/healthz` 返回 `ok`，错误页为中文且不泄露内部信息。

Task 1 入口在 Task 10 中基于当前实现补齐；保留旧 scaffold 工作树，未覆盖后续任务的路由或页面。

### Task 2: Migration 和 repository

Files: `migrations/0001_initial.sql`, `migrations/0002_indexes.sql`, `src/domain/*`, `src/db/*`, `tests/db.rs`。

- [x] 测试五张表、外键、唯一编号、唯一 active 学年和声明唯一性。
- [x] 实现 `academic_years`、`submissions`、`attachments`、`student_declarations`、`settings` 表。
- [x] 实现 `AcademicYearRepo::current`、`SubmissionRepo::insert/find/list/update_review`、`SettingsRepo::get/set` 和附件查询。
- [x] 启动时执行 migration；没有学年时可重复 seed `2025-2026学年`。
- [x] 运行 `cargo test --test db`。

完成条件：内存/临时 SQLite 能完成 migration、插入、查询和事务回滚。

### Task 3: Argon2id、session、CSRF 和附件存储

Files: `src/auth/*`, `src/storage/*`, `tests/auth.rs`, `tests/storage.rs`。

- [x] 测试 secret 哈希、错误凭据、修改码字符集、CSRF token、路径穿越、10 MiB/10 文件限制。
- [x] 实现 `hash_secret`、`verify_secret`、`generate_edit_code`、`generate_submission_no`。
- [x] 实现 admin/student/receipt/verified-student session，设置安全 Cookie 属性。
- [x] 实现临时文件、随机存储名、数据库元数据和受保护读取。
- [x] 运行定向测试和 clippy。

完成条件：附件目录不是静态公开目录；口令/密码保存哈希，修改码按最新用户要求保存验证哈希和管理员专用认证加密密文；越权下载测试失败。

### Task 4: 七类 schema 和服务端校验

Files: `src/domain/category.rs`, `src/validation/*`, `tests/validation.rs`。

- [x] 为七类字段分别写合法样例和缺失条件字段测试。
- [x] 定义固定 `Category`、`SubmissionStatus`、category JSON keys 和中文 labels。
- [x] 实现姓名/学号/日期/截止时间/类别/分值/条件字段/上传验证。
- [x] 明确奖学金性质为“可提交但需人工确认”。
- [x] 运行 `cargo test --test validation`。

完成条件：前端绕过时服务端仍拒绝所有非法数据，类别不适用字段不影响校验。

最终复核更新（2026-09-10，aa41e71）：F2 已修复，校级荣誉类别仅适用于校级且可选，六项预置及历史文本保留已验证。七类合法/条件缺字段、CET 数值及旧值兼容、非校级合法输入都有 HTTP/仓库回归证据。复选框按现有源码和执行证据同步，不据此倒推历史红绿执行过程。

### Task 5: 学生首页、口令进入和提交

Files: `src/routes/student.rs`, `templates/student/{home,submit,success}.html`, `static/js/submission-form.js`, `static/css/app.css`, `tests/submission_flow.rs`。

- [x] 写错误口令、无 active year、截止后、合法 multipart、无材料声明和编号唯一测试。
- [x] 实现 `/`、`/access`、`/submit`、`/success/:submission_no`。
- [x] 在一个事务中创建 submission、附件和哈希修改码；失败时清理临时文件。
- [x] 原生 JS 按类别显示条件字段并禁用隐藏输入；按 `FRONTEND_GUIDELINES.md` 完成手机布局。
- [x] 运行 `cargo test --test submission_flow`。

完成条件：学生可完成一项成果提交并在成功页看到明文修改码；无材料声明不创建附件。

### Task 6: 查询、修改和附件访问

Files: `src/routes/query.rs`, `templates/student/{query,detail}.html`, `tests/query_flow.rs`。

- [x] 写错误修改码、session 越权、四种状态编辑权限、修改回 pending 和附件权限测试。
- [x] 实现 `/query`、`/query/:submission_no`、更新 POST 和保护附件 GET。
- [x] 更新时复用 Task 4 校验；保留审核备注，设置 `student_modified_after_review`。
- [x] 运行 `cargo test --test query_flow`。

完成条件：学生只能访问自己的单条申报；已通过和不予认定只读。

### Task 7: 管理员后台

Files: `src/routes/admin_auth.rs`, `src/routes/admin.rs`, `src/routes/admin_submissions.rs`, `templates/admin/{login,index,submission}.html`, `tests/admin_flow.rs`。

- [x] 写未登录保护、错误凭据统一提示、筛选、附件查看、状态和分值校验测试。
- [x] 实现 `/admin/login`、`/admin/logout`、`/admin`、详情和审核 POST。
- [x] 加入 `require_admin` middleware、CSRF、统计计数和中文状态标签。
- [x] 列表默认 `created_at DESC`；筛选参数只允许后端定义的枚举和值。
- [x] 运行 `cargo test --test admin_flow`。

完成条件：管理员可完整审核一条申报，学生 session 不可调用管理员路由。

### Task 8: 学年和班级口令设置

Files: `src/routes/admin_settings.rs`, `templates/admin/settings.html`, `tests/settings.rs`。

- [x] 写 active 唯一性、日期顺序、截止时间、历史保留和口令更新测试。
- [x] 实现新建/编辑/激活学年和班级口令 POST 表单。
- [x] 激活操作在一个事务内先关闭其他 active 再激活目标。
- [x] 运行 `cargo test --test settings`。

完成条件：设置修改立即生效，历史申报仍可查看和导出。

Task 8 交付：设置与历史数据保留已验证；实际 XLSX 下载依任务顺序在 Task 9 实现。新增设置集成测试、浏览器回归及联动修复说明见 `docs/testing-admin-flow.md` 和 `progress.txt`。

### Task 9: Excel 导出

Files: `src/export/*`, `tests/export.rs`, admin export route/templates。

- [x] 写两张工作表、34 列明细、学生分组和 approved 分值测试。
- [x] 实现固定列映射，类别 JSON 展开，附件数量/标识输出。
- [x] 设置首行加粗、冻结、筛选、列宽、日期格式和数值格式。
- [x] 实现 `/admin/export.xlsx`，支持当前学年和与列表相同的可选筛选。
- [x] 运行 `cargo test --test export`。

完成条件：Excel 在 LibreOffice/Excel 中打开无乱码，字段可筛选，空结果也有合法表头。

Task 9 交付：12 项导出集成测试覆盖固定列、七类条件分支、历史筛选、学生汇总、权限与格式；真实浏览器样例和 LibreOffice 往返验证记录见 `docs/testing-export.md`、`progress.txt`。Excel 单元格超长采用明确截断提示及受保护原文入口，1900 年前日期保留 ISO 文本；原始数据不变。

### Task 10: 安全、日志、部署和备份

Files: `Dockerfile`, `docker-compose.yml`, `Caddyfile.example`, `scripts/backup.sh`, `tests/security.rs`, `tests/smoke.sh`, `README.md`。

- [x] 写 cookie、CSRF、日志脱敏、错误页、Compose 挂载和重启持久化检查。
- [x] 完成 tracing 事件、登录失败延迟、请求体限制、Caddy HTTP->HTTPS、健康检查。
- [x] Compose 只包含 `web` 和 `caddy`；挂载 `/opt/zongce/data`、`/opt/zongce/uploads`、`/opt/zongce/backups`。
- [x] `backup.sh` 每日复制 SQLite 和 uploads，数据库备份至少保留 7 份，失败返回非零并记录日志。
- [x] 完善 README：本地启动、管理员 hash 生成、初始化、部署、恢复和 cron/systemd timer。
- [x] 执行 `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-targets && docker compose config`。

完成条件：开发机完成学生提交、查询修改、管理员审核、Excel 下载；容器重启后数据和附件存在。

Task 10 交付：74 项 Rust 测试、八种备份故障检查、四尺寸真实入口浏览器流程、固定版本 amd64 镜像、Compose 配置、HTTPS 冒烟与脱敏、容器及宿主虚拟机重启、七套备份保留和成套恢复均已验证。Compose 使用 `config --quiet` 避免打印秘密；本地 CA 验证不替代上线后的公网证书检查。完整记录见 `docs/testing-deployment.md`、`progress.txt`。

## 2. 交付门禁

1. Task 1-4 完成后才能进入页面开发；所有领域类型和校验接口冻结。
2. Task 5-6 完成后执行一次手机宽度和桌面宽度完整学生流程。
3. Task 7-8 完成后执行管理员审核和设置回归。
4. Task 9 完成后用真实七类样例核对 Excel 每一列。
5. Task 10 完成后才能部署；部署前必须备份恢复演练一次。

## 3. 最终验收矩阵

2026-09-10 最终结论更新：**本地 P0 验收通过，F1、F2 已关闭**。最终受测应用代码 `aa41e71`，完整复现、修复证据和边界见 [`docs/final-acceptance.md`](docs/final-acceptance.md)。`7ad62cf` 的未通过结论保留为历史记录。

| Area | Evidence | Result |
|---|---|---|
| Student submit | valid result, no-result declaration, all validation failures | 通过：七类/条件验证、CET-4/CET-6、无 JS 声明和字段刷新、Enter 提交；F1/F2 回归通过。 |
| Student edit | wrong code, each status, resubmission to pending | 通过：状态/权限/回待审核、无 JS 编辑、历史 CET/荣誉文本保留、重置后旧会话及并发写入失效。 |
| Admin | login, filters, detail, attachment, review, settings | 通过：原流程、无申报材料列表、受保护修改码显示及确认重置、四尺寸浏览器验证。 |
| Export | two sheets, expanded columns, approved-only totals | 通过：两表、34/10 列、七类展开、分组及仅 Approved 汇总；22 条成果与 4 条声明，CET 分值保持数值。 |
| Security | hashed/encrypted secrets, protected files, CSRF, safe errors/logs | 通过：修改码加密恢复/篡改隔离/版本撤销/日志边界新增回归通过；有界内存缓冲差异 D2 保留。 |
| Deployment | HTTPS redirect, persistent volumes, restart recovery, seven backups | 本地通过：固定 SQLite 3.46.1 镜像、CA 验证 HTTPS、迁移/重启、成套备份保留及恢复；当前预览升级保留数据，公网域名上线时验收。 |

最终 95 项 Rust 测试、格式、严格 Clippy、八种备份故障检查、Compose 配置、四尺寸实际镜像浏览器流程、HTTPS 和迁移/重启检查均通过；独立最终审查无 Important/Critical 遗留项。F1/F2 有专门回归证据，D1/D3 同步关闭。公网证书及新一轮办公软件 GUI 验证不在本次证据范围内。

## 4. Post-MVP boundary

### 2026-09-10 用户新增范围：名单与删除

- [x] 先导入姓名＋学号再创建学年，模板、CSV/粘贴、按学年名单和开放后补录。
- [x] 成果/无材料声明/修改的服务端名单校验，移除生产启动自动 seed。
- [x] 所有审核状态及声明单条/批量删除、回收站恢复、统计/导出/附件权限同步。
- [x] 删除已有学年，明确确认并清理其名单、全部记录及附件，支持同名重建。
- [x] 101 项 Rust 测试、严格 Clippy、格式、Compose 配置、原有和新增四尺寸浏览器、实际 amd64 镜像 SQLite 3.46.1 检查。
- [x] 成套备份及冷迁移保留验证；仅重建 26–27 本地测试学年，25–26 学年与原有配置保留。

本次用户明确要求将名单校验纳入当前版本，优先于旧 P2 边界。复现步骤与结果见 [名单与删除验收](docs/testing-roster-recycle.md)。

After all P0 evidence is recorded, P1 may be planned as separate change sets. P2 remains outside this release and must not be added while any P0 acceptance item is failing.
