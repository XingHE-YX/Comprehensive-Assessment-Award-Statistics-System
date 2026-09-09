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

- [ ] 写 `GET /healthz` 测试和缺失环境变量测试。
- [ ] 固定 `TECH_STACK.md` 中的依赖版本，定义 `Config`、`AppState`、`AppError`。
- [ ] 初始化 tracing、SQLite pool、Askama、路由和 graceful shutdown。
- [ ] 运行 `cargo test --test health`、`cargo fmt --check`、clippy。

完成条件：程序可启动，`/healthz` 返回 `ok`，错误页为中文且不泄露内部信息。

### Task 2: Migration 和 repository

Files: `migrations/0001_initial.sql`, `migrations/0002_indexes.sql`, `src/domain/*`, `src/db/*`, `tests/db.rs`。

- [ ] 测试五张表、外键、唯一编号、唯一 active 学年和声明唯一性。
- [ ] 实现 `academic_years`、`submissions`、`attachments`、`student_declarations`、`settings` 表。
- [ ] 实现 `AcademicYearRepo::current`、`SubmissionRepo::insert/find/list/update_review`、`SettingsRepo::get/set` 和附件查询。
- [ ] 启动时执行 migration；没有学年时可重复 seed `2025-2026学年`。
- [ ] 运行 `cargo test --test db`。

完成条件：内存/临时 SQLite 能完成 migration、插入、查询和事务回滚。

### Task 3: Argon2id、session、CSRF 和附件存储

Files: `src/auth/*`, `src/storage/*`, `tests/auth.rs`, `tests/storage.rs`。

- [ ] 测试 secret 哈希、错误凭据、修改码字符集、CSRF token、路径穿越、10 MiB/10 文件限制。
- [ ] 实现 `hash_secret`、`verify_secret`、`generate_edit_code`、`generate_submission_no`。
- [ ] 实现 admin/student/receipt/verified-student session，设置安全 Cookie 属性。
- [ ] 实现临时文件、随机存储名、数据库元数据和受保护读取。
- [ ] 运行定向测试和 clippy。

完成条件：附件目录不是静态公开目录；凭证只保存哈希；越权下载测试失败。

### Task 4: 七类 schema 和服务端校验

Files: `src/domain/category.rs`, `src/validation/*`, `tests/validation.rs`。

- [ ] 为七类字段分别写合法样例和缺失条件字段测试。
- [ ] 定义固定 `Category`、`SubmissionStatus`、category JSON keys 和中文 labels。
- [ ] 实现姓名/学号/日期/截止时间/类别/分值/条件字段/上传验证。
- [ ] 明确奖学金性质为“可提交但需人工确认”。
- [ ] 运行 `cargo test --test validation`。

完成条件：前端绕过时服务端仍拒绝所有非法数据，类别不适用字段不影响校验。

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

- [ ] 写 active 唯一性、日期顺序、截止时间、历史保留和口令更新测试。
- [ ] 实现新建/编辑/激活学年和班级口令 POST 表单。
- [ ] 激活操作在一个事务内先关闭其他 active 再激活目标。
- [ ] 运行 `cargo test --test settings`。

完成条件：设置修改立即生效，历史申报仍可查看和导出。

### Task 9: Excel 导出

Files: `src/export/*`, `tests/export.rs`, admin export route/templates。

- [ ] 写两张工作表、34 列明细、学生分组和 approved 分值测试。
- [ ] 实现固定列映射，类别 JSON 展开，附件数量/标识输出。
- [ ] 设置首行加粗、冻结、筛选、列宽、日期格式和数值格式。
- [ ] 实现 `/admin/export.xlsx`，支持当前学年和与列表相同的可选筛选。
- [ ] 运行 `cargo test --test export`。

完成条件：Excel 在 LibreOffice/Excel 中打开无乱码，字段可筛选，空结果也有合法表头。

### Task 10: 安全、日志、部署和备份

Files: `Dockerfile`, `docker-compose.yml`, `Caddyfile.example`, `scripts/backup.sh`, `tests/security.rs`, `tests/smoke.sh`, `README.md`。

- [ ] 写 cookie、CSRF、日志脱敏、错误页、Compose 挂载和重启持久化检查。
- [ ] 完成 tracing 事件、登录失败延迟、请求体限制、Caddy HTTP->HTTPS、健康检查。
- [ ] Compose 只包含 `web` 和 `caddy`；挂载 `/opt/zongce/data`、`/opt/zongce/uploads`、`/opt/zongce/backups`。
- [ ] `backup.sh` 每日复制 SQLite 和 uploads，数据库备份至少保留 7 份，失败返回非零并记录日志。
- [ ] 完善 README：本地启动、管理员 hash 生成、初始化、部署、恢复和 cron/systemd timer。
- [ ] 执行 `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-targets && docker compose config`。

完成条件：开发机完成学生提交、查询修改、管理员审核、Excel 下载；容器重启后数据和附件存在。

## 2. 交付门禁

1. Task 1-4 完成后才能进入页面开发；所有领域类型和校验接口冻结。
2. Task 5-6 完成后执行一次手机宽度和桌面宽度完整学生流程。
3. Task 7-8 完成后执行管理员审核和设置回归。
4. Task 9 完成后用真实七类样例核对 Excel 每一列。
5. Task 10 完成后才能部署；部署前必须备份恢复演练一次。

## 3. 最终验收矩阵

| Area | Evidence |
|---|---|
| Student submit | valid result, no-result declaration, all validation failures |
| Student edit | wrong code, each status, resubmission to pending |
| Admin | login, filters, detail, attachment, review, settings |
| Export | two sheets, expanded columns, approved-only totals |
| Security | hashed secrets, protected files, CSRF, safe errors/logs |
| Deployment | HTTPS redirect, persistent volumes, restart recovery, seven backups |

## 4. Post-MVP boundary

After all P0 evidence is recorded, P1 may be planned as separate change sets. P2 remains outside this release and must not be added while any P0 acceptance item is failing.
