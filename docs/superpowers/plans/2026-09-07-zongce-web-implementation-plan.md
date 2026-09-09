# 综测成果申报系统（单班级版）实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 构建一个面向单班级、可跨学年复用的综测成果申报系统，完成学生申报与附件上传、申报查询/修改、管理员审核与分值核定、Excel 导出以及 Docker + Caddy 部署。

**Architecture:** 采用服务端渲染单体应用。Axum 负责 HTTP 路由和请求处理，Askama 负责 HTML 模板，原生 JavaScript 负责成果类别条件字段和少量交互，SQLite 负责持久化，上传文件存放在独立的宿主机挂载目录；Caddy 负责 HTTPS 和反向代理。类别差异字段存入 `category_data` JSON，但在 Excel 导出时展开成固定列。

**Tech Stack:** Rust stable、Axum、Askama、SQLx（SQLite）、Serde/serde_json、`argon2`、`rand`、Cookie Session（`tower-sessions` 或等价方案）、`rust_xlsxwriter`、`tracing`/`tracing-subscriber`、原生 JavaScript、手写或轻量 CSS、Docker Compose、Caddy。

**Spec:** `/Users/xingheluqi/Downloads/zongce-web-dev-requirements.md`

## 项目结构

目标目录结构如下。模块按职责拆分，路由层只负责请求编排，校验、数据库访问、文件处理和导出逻辑分别放在独立模块中。

```text
.
├── Cargo.toml
├── Cargo.lock
├── README.md
├── .env.example
├── Dockerfile
├── docker-compose.yml
├── Caddyfile.example
├── migrations/
│   ├── 0001_initial.sql
│   └── 0002_indexes.sql
├── scripts/
│   └── backup.sh
├── src/
│   ├── main.rs                 # 配置、日志、数据库、路由、静态资源启动
│   ├── config.rs               # 环境变量和运行时配置
│   ├── error.rs                # AppError、中文错误页和状态码映射
│   ├── state.rs                # AppState 及共享服务
│   ├── auth/
│   │   ├── mod.rs              # 管理员 session、班级口令、修改码
│   │   └── password.rs         # Argon2id 哈希和随机码
│   ├── domain/
│   │   ├── mod.rs
│   │   ├── academic_year.rs    # 学年模型与当前学年规则
│   │   ├── submission.rs       # 申报模型、状态和类别
│   │   ├── category.rs         # 七类字段定义、Serde 类型和标签
│   │   ├── attachment.rs       # 附件元数据
│   │   └── declaration.rs      # 无材料声明
│   ├── validation/
│   │   ├── mod.rs
│   │   ├── submission.rs       # 服务端基础字段与日期校验
│   │   ├── category.rs         # 条件字段校验
│   │   └── upload.rs           # 数量、大小、MIME、扩展名校验
│   ├── db/
│   │   ├── mod.rs
│   │   ├── academic_years.rs
│   │   ├── submissions.rs
│   │   ├── attachments.rs
│   │   ├── declarations.rs
│   │   └── settings.rs
│   ├── storage/
│   │   ├── mod.rs              # 随机文件名、目录布局、流式写入
│   │   └── attachments.rs      # 上传、受保护读取和删除
│   ├── export/
│   │   ├── mod.rs
│   │   └── xlsx.rs              # 申报明细和学生汇总两个工作表
│   ├── routes/
│   │   ├── mod.rs
│   │   ├── student.rs          # /、/submit、/success
│   │   ├── query.rs            # /query、附件访问、学生修改
│   │   ├── admin_auth.rs       # /admin/login、登出
│   │   ├── admin.rs            # /admin 列表和筛选
│   │   ├── admin_submissions.rs# 详情、审核、分值
│   │   └── admin_settings.rs   # 学年和班级口令设置
│   └── templates/
│       ├── layout.html
│       ├── errors/{403,404,500}.html
│       ├── student/{home,submit,success,query,detail}.html
│       └── admin/{login,index,submission,settings}.html
├── static/
│   ├── css/app.css
│   └── js/submission-form.js
└── tests/
    ├── validation.rs
    ├── auth.rs
    ├── submission_flow.rs
    ├── admin_flow.rs
    └── export.rs
```

## 全局约束

- 采用 Rust + Axum + Askama + SQLite 的服务端渲染单体应用。
- 前端只使用原生 JavaScript，禁止 React、Vue、Leptos、Yew 和 Node.js 运行时依赖。
- 第一版只服务一个班级；同时只能有一个 `is_active = true` 的当前学年。
- 学生无需账号，通过班级口令进入；每项成果独立提交，并获得申报编号和修改码。
- 修改码和班级口令只保存安全哈希；管理员密码使用 Argon2id。
- 附件仅允许 JPG、JPEG、PNG、PDF；单文件最大 10 MiB；每条成果最多 10 个，成果申报至少 1 个。
- 所有前端校验必须在服务端重复执行；数据库访问必须使用参数化查询。
- 附件不能通过可猜测的公开 URL 访问，只能由管理员 session 或通过申报编号 + 修改码验证后的学生访问。
- 管理员写操作使用 POST/PUT；实现基础 CSRF 防护和登录失败延迟/限速。
- Excel 必须生成 `申报明细` 和 `学生汇总` 两个工作表，类别字段展开成独立列。
- SQLite、`uploads/`、备份目录必须使用宿主机持久化挂载。
- 生产配置通过环境变量提供：`APP_ENV`、`BIND_ADDR`、`ADMIN_USERNAME`、`ADMIN_PASSWORD_HASH`、`SESSION_SECRET`、`DATABASE_URL`、`UPLOAD_DIR`。

## 实施计划

### Task 1: 项目骨架、配置和错误处理

**Files:**
- Create: `Cargo.toml`, `src/main.rs`, `src/config.rs`, `src/state.rs`, `src/error.rs`
- Create: `templates/layout.html`, `templates/errors/{403,404,500}.html`, `static/css/app.css`
- Create: `.env.example`, `README.md`
- Test: `tests/health.rs`

**Interfaces:**
- `Config::from_env() -> Result<Config, ConfigError>` 读取并校验所有运行时配置。
- `AppState { db: SqlitePool, config: Config, storage: AttachmentStorage }` 作为 Axum state。
- `build_router(state: AppState) -> Router` 注册公共、学生端和管理员端路由。
- `AppError` 实现 `IntoResponse`，将 403/404/500 映射到中文模板。

- [x] **Step 1: 定义依赖、配置字段和健康检查测试**

  测试 `GET /healthz` 返回 `200 OK`，配置缺失时 `Config::from_env` 返回可读错误。

- [x] **Step 2: 运行测试确认骨架尚未完成**

  运行：`cargo test --test health`

  预期：因路由和配置实现不存在而失败。

- [x] **Step 3: 实现配置、共享状态、错误页和基础路由**

  `main.rs` 初始化 `tracing_subscriber`、SQLite pool、migration，并绑定 `BIND_ADDR`；开发环境允许 HTTP，生产环境由 Caddy 终止 HTTPS。

- [x] **Step 4: 运行测试并提交**

  运行：`cargo test --test health`

  预期：PASS。

  ```bash
  git add Cargo.toml Cargo.lock src templates static .env.example README.md tests/health.rs
  git commit -m "chore: scaffold axum application"
  ```

### Task 2: 数据库 migration、领域类型和种子初始化

**Files:**
- Create: `migrations/0001_initial.sql`, `migrations/0002_indexes.sql`
- Create: `src/domain/{mod,academic_year,submission,category,attachment,declaration}.rs`
- Create: `src/db/{mod,academic_years,submissions,attachments,declarations,settings}.rs`
- Modify: `src/main.rs`, `src/state.rs`
- Test: `tests/db.rs`

**Interfaces:**
- 表：`academic_years`、`submissions`、`attachments`、`student_declarations`、`settings`。
- `SubmissionStatus = Pending | Approved | NeedsRevision | Rejected`，数据库值分别为 `pending`、`approved`、`needs_revision`、`rejected`。
- `Submission.category_data: serde_json::Value` 保存类别专属字段。
- `AcademicYearRepo::current(&pool) -> Result<Option<AcademicYear>>`。
- `SubmissionRepo::insert(...) -> Result<Submission>`、`find_by_no(...)`、`list(filter)`、`update_review(...)`。
- `SettingsRepo::get/set(key)` 保存班级口令哈希等配置。

- [ ] **Step 1: 编写 migration 和数据库集成测试**

  测试 migration 后所有表存在；插入两个学年时只能把一个设置为 active；提交编号唯一；附件通过 `submission_id` 级联关联。

- [ ] **Step 2: 运行 `cargo test --test db` 确认失败**

- [ ] **Step 3: 实现 SQLx models、repository 和启动 migration**

  使用外键、唯一索引、状态约束和日期字段；提供可重复执行的默认学年 seed（名称 `2025-2026学年`、起止日期 `2025-08-31` 至 `2026-08-28`），不把日期写成唯一业务规则。

- [ ] **Step 4: 运行测试并提交**

  运行：`cargo test --test db`

  ```bash
  git add migrations src/domain src/db src/main.rs src/state.rs tests/db.rs
  git commit -m "feat: add sqlite schema and repositories"
  ```

### Task 3: 认证、申报编号和受保护附件存储

**Files:**
- Create: `src/auth/{mod,password}.rs`
- Create: `src/storage/{mod,attachments}.rs`
- Modify: `src/config.rs`, `src/state.rs`, `src/error.rs`
- Test: `tests/auth.rs`, `tests/storage.rs`

**Interfaces:**
- `hash_secret(plain: &str) -> Result<String>`、`verify_secret(hash, plain) -> bool` 使用 Argon2id。
- `generate_edit_code() -> String` 返回 8-10 位、排除 `0/O/1/I/l` 的随机码。
- `generate_submission_no(year: &AcademicYear, sequence: u64) -> String` 生成如 `ZC2026-000001` 的编号。
- `AttachmentStorage::save(year_name, submission_no, upload) -> Result<StoredAttachment>`。
- `AttachmentStorage::open(stored_name) -> Result<impl AsyncRead>` 只接受数据库中的安全存储名。
- `require_admin(session) -> Result<AdminUser>` 和 `verify_student_access(submission_no, edit_code) -> Result<Submission>`。

- [ ] **Step 1: 编写哈希、随机码、路径穿越和附件限制测试**

  覆盖正确/错误哈希、随机码字符集、随机存储文件名、拒绝 `../`、拒绝超过 10 MiB 或不支持 MIME、同一成果最多 10 个附件。

- [ ] **Step 2: 运行认证和存储测试确认失败**

  运行：`cargo test --test auth --test storage`

- [ ] **Step 3: 实现认证、session、上传流式写入和受保护读取**

  原始文件名只进数据库；磁盘文件名使用随机 ID；按学年/申报编号建目录；附件路由不暴露静态目录；session Cookie 设置 `HttpOnly`、`SameSite=Lax`，生产配置 `Secure`。

- [ ] **Step 4: 运行测试并提交**

  运行：`cargo test --test auth --test storage`

  ```bash
  git add src/auth src/storage src/config.rs src/state.rs src/error.rs tests/auth.rs tests/storage.rs
  git commit -m "feat: add hashed access codes and protected uploads"
  ```

### Task 4: 七类成果 schema、动态表单和服务端校验

**Files:**
- Modify: `src/domain/category.rs`
- Create: `src/validation/{mod,submission,category,upload}.rs`
- Create: `templates/student/{home,submit}.html`
- Create: `static/js/submission-form.js`
- Modify: `static/css/app.css`
- Test: `tests/validation.rs`

**Interfaces:**
- `Category` 固定七个枚举值，并提供中文 label。
- `validate_submission(input, current_year, uploads) -> Result<ValidatedSubmission, ValidationErrors>`。
- `validate_category(category, data) -> Result<(), ValidationErrors>`。
- `validate_upload(file) -> Result<ValidatedUpload, ValidationErrors>`。
- `category_data` 的 key 与 Excel 列固定：竞赛目录序号、级别、获奖等级、文章性质、平台/期刊、作者排序、社会实践身份、专利类型/状态/排名/号码、证书类型、考试成绩、专业类别、资格证书名称等。

- [ ] **Step 1: 为每个类别写服务端校验测试**

  覆盖七类字段、条件必填（如竞赛“其他奖项或名次”、论文字段、计算机等级字段）、日期范围、姓名/学号长度、附件至少一个和声明无需附件。

- [ ] **Step 2: 运行 `cargo test --test validation` 确认失败**

- [ ] **Step 3: 实现固定 schema、Askama 表单和原生 JS 条件显示**

  前端按 `data-category` 切换区块；无关字段隐藏且不提交；奖学金性质项目显示“需人工确认”提示但不自动拒绝。

- [ ] **Step 4: 运行测试并提交**

  运行：`cargo test --test validation`

  ```bash
  git add src/domain/category.rs src/validation templates/student static/js/submission-form.js static/css/app.css tests/validation.rs
  git commit -m "feat: add category schemas and validation"
  ```

### Task 5: 学生提交、无材料声明和成功页

**Files:**
- Create/modify: `src/routes/student.rs`, `src/db/declarations.rs`
- Create: `templates/student/success.html`
- Modify: `src/routes/mod.rs`, `src/state.rs`
- Test: `tests/submission_flow.rs`

**Interfaces:**
- `GET /`：展示当前学年、有效日期、截止时间、说明和班级口令入口。
- `POST /access`：校验班级口令并建立短期学生访问 session。
- `GET /submit`、`POST /submit`：提交一项成果或无材料声明。
- `GET /success/:submission_no`：仅展示本次提交生成的明文修改码。
- `SubmissionService::create(input, uploads) -> Result<SubmissionReceipt>`。
- `DeclarationRepo::upsert(academic_year_id, student_name, student_no)`。

- [ ] **Step 1: 编写端到端提交测试**

  覆盖错误口令、正确口令、成果提交生成唯一编号和一次性可见修改码、无材料声明、当前未开放学年、截止时间和重复姓名+学号仍可提交成果。

- [ ] **Step 2: 运行测试确认失败**

  运行：`cargo test --test submission_flow`

- [ ] **Step 3: 实现 route handler、multipart 上传和事务提交**

  在同一事务中写入 submission 与 attachments；生成编号时使用学年年份 + 数据库序列/锁保证唯一；日志只记录编号，不记录修改码。

- [ ] **Step 4: 运行测试并提交**

  运行：`cargo test --test submission_flow`

  ```bash
  git add src/routes/student.rs src/routes/mod.rs src/db/declarations.rs src/state.rs templates/student/success.html tests/submission_flow.rs
  git commit -m "feat: implement student submission flow"
  ```

### Task 6: 查询、学生修改和附件访问

**Files:**
- Create: `src/routes/query.rs`
- Create: `templates/student/{query,detail}.html`
- Modify: `src/db/submissions.rs`, `src/storage/attachments.rs`
- Test: `tests/query_flow.rs`

**Interfaces:**
- `GET /query`、`POST /query`：使用申报编号 + 修改码验证。
- `GET /submissions/:submission_no/attachments/:id`：验证学生访问或管理员 session 后读取附件。
- `POST /submissions/:submission_no/update`：仅 `pending` 和 `needs_revision` 可修改。
- `can_student_edit(status) -> bool`：`pending`/`needs_revision` 为 true，其余为 false。
- 修改成功后将状态设为 `pending`、保留审核备注、设置 `student_modified_after_review = true`、更新 `updated_at`。

- [ ] **Step 1: 编写状态权限和越权测试**

  错误修改码、错误申报编号、访问他人附件、已通过/不予认定修改、需补充材料修改后回待审核均须覆盖。

- [ ] **Step 2: 运行测试确认失败**

  运行：`cargo test --test query_flow`

- [ ] **Step 3: 实现查询、详情、修改和附件读取**

  查询成功后只在 session 中保存已验证的 submission id；模板自动转义用户内容；修改表单沿用 Task 4 的类别字段和服务端校验。

- [ ] **Step 4: 运行测试并提交**

  运行：`cargo test --test query_flow`

  ```bash
  git add src/routes/query.rs src/db/submissions.rs src/storage/attachments.rs templates/student tests/query_flow.rs
  git commit -m "feat: add submission query and student edits"
  ```

### Task 7: 管理员登录、列表、筛选、详情和审核

**Files:**
- Create: `src/routes/admin_auth.rs`, `src/routes/admin.rs`, `src/routes/admin_submissions.rs`
- Create: `templates/admin/{login,index,submission}.html`
- Modify: `src/auth/mod.rs`, `src/routes/mod.rs`, `static/css/app.css`
- Test: `tests/admin_flow.rs`

**Interfaces:**
- `GET/POST /admin/login`、`POST /admin/logout`。
- `GET /admin`：筛选参数 `academic_year_id`、`name`、`student_no`、`category`、`status`，默认 `created_at DESC`。
- `GET /admin/submissions/:id`：展示通用字段、类别字段、附件和修改标记。
- `POST /admin/submissions/:id/review`：更新状态、审核备注、核定分值。
- `require_admin` middleware 保护所有 `/admin/*`。
- `approved` 建议要求 `approved_score` 非空；`rejected` 可默认 0；分值允许 0，最多两位小数。

- [x] **Step 1: 编写管理员权限、筛选和审核测试**

  覆盖未登录 403/重定向、错误凭据统一提示、状态筛选、管理员查看附件、状态变更、分值校验和学生修改标记。

- [x] **Step 2: 运行测试确认失败**

  运行：`cargo test --test admin_flow`

- [x] **Step 3: 实现后台页面和 POST 审核流程**

  顶部展示当前学年、总数、各状态数、无材料声明数和已通过总分；状态标签使用明确中文；加入隐藏 CSRF token 并在 POST 中验证。

- [x] **Step 4: 运行测试并提交**

  运行：`cargo test --test admin_flow`

  ```bash
  git add src/routes/admin_auth.rs src/routes/admin.rs src/routes/admin_submissions.rs src/auth src/routes/mod.rs templates/admin static/css/app.css tests/admin_flow.rs
  git commit -m "feat: add admin review workflow"
  ```

### Task 8: 学年、班级口令和系统设置

**Files:**
- Create: `src/routes/admin_settings.rs`
- Create: `templates/admin/settings.html`
- Modify: `src/db/academic_years.rs`, `src/db/settings.rs`, `src/routes/mod.rs`
- Test: `tests/settings.rs`

**Interfaces:**
- `GET /admin/settings`：列出历史学年和设置表单。
- `POST /admin/years`：新建学年。
- `POST /admin/years/:id`：编辑日期、截止时间、说明。
- `POST /admin/years/:id/activate`：事务内关闭其他 active，再激活目标学年。
- `POST /admin/settings/class-code`：哈希后更新班级口令，无需重启。

- [x] **Step 1: 编写 active 学年唯一性和口令更新测试**

- [x] **Step 2: 运行 `cargo test --test settings` 确认失败**

- [x] **Step 3: 实现设置页面和事务更新**

  未配置学年时学生首页显示“当前暂未开放申报”；历史数据保持可查询和可导出。

- [x] **Step 4: 运行测试并提交**

  运行：`cargo test --test settings`

  ```bash
  git add src/routes/admin_settings.rs src/db/academic_years.rs src/db/settings.rs src/routes/mod.rs templates/admin/settings.html tests/settings.rs
  git commit -m "feat: add academic year and class settings"
  ```

### Task 9: Excel 导出和基础统计

**Files:**
- Create: `src/export/{mod,xlsx}.rs`
- Modify: `src/routes/admin.rs`, `src/routes/admin_submissions.rs`, `src/routes/mod.rs`
- Test: `tests/export.rs`

**Interfaces:**
- `GET /admin/export.xlsx?academic_year_id=...`：导出当前学年。
- 可选筛选参数与后台列表一致，用于导出筛选结果。
- `export_xlsx(submissions, declarations, filter) -> Result<Vec<u8>>`。
- 工作表固定为 `申报明细`、`学生汇总`；附件列输出数量和下载标识，不把 `category_data` 原 JSON 直接写入单元格。

- [x] **Step 1: 编写 xlsx 内容和格式测试**

  读取生成文件，验证两张工作表、34 列明细字段、学生按“学号 + 姓名”分组、仅 `approved` 计入总分、日期和分值为正确类型、首行冻结和筛选存在。

- [x] **Step 2: 运行测试确认失败**

  运行：`cargo test --test export`

- [x] **Step 3: 实现字段展开、格式、列宽和下载响应**

  分值使用数字格式且最多两位小数；中文状态使用中文显示；文件名使用 `<学年>综测申报汇总.xlsx`。

- [x] **Step 4: 运行测试并提交**

  运行：`cargo test --test export`

  ```bash
  git add src/export src/routes/admin.rs src/routes/admin_submissions.rs src/routes/mod.rs tests/export.rs
  git commit -m "feat: add xlsx exports"
  ```

### Task 10: 安全加固、日志、部署、备份和验收

**Files:**
- Modify: `src/main.rs`, `src/error.rs`, `src/auth/mod.rs`, `src/routes/*`, `static/css/app.css`, `README.md`
- Create: `Dockerfile`, `docker-compose.yml`, `Caddyfile.example`, `scripts/backup.sh`
- Test: `tests/security.rs`, `tests/smoke.sh`

**Interfaces:**
- 统一 tracing 事件：启动、migration、提交成功、上传失败、管理员登录成功/失败、审核变更、Excel 导出和关键异常。
- `scripts/backup.sh`：备份 `/opt/zongce/data/app.db`、`/opt/zongce/uploads` 到 `/opt/zongce/backups`，保留最近至少 7 份数据库备份，并记录失败退出码。
- Compose 服务：`web`、`caddy`；挂载 `/opt/zongce/data`、`/opt/zongce/uploads`、`/opt/zongce/backups`。

- [x] **Step 1: 编写安全和部署冒烟检查**

  检查 SQL 参数化、模板转义、上传拒绝路径穿越、Cookie 属性、CSRF、管理员路由保护、错误页不泄露 SQL/路径/secret；启动容器后执行学生提交、查询修改、管理员审核和 xlsx 下载冒烟流程。

- [x] **Step 2: 运行全量测试确认当前缺口**

  运行：`cargo test --all-targets`。

- [x] **Step 3: 实现日志、限速/失败延迟、Docker、Caddy 和备份**

  release 镜像在开发机或 CI 构建；生产环境只运行编译好的镜像；Caddy 将 HTTP 重定向 HTTPS 并反代到 `web:3000`；README 写明初始化、环境变量、迁移、备份恢复和部署步骤。

- [x] **Step 4: 执行最终验收**

  运行：`cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-targets && docker compose config`

  在手机宽度和桌面宽度各完成一次完整流程，确认重启容器后 SQLite 和附件仍存在；确认每日备份脚本可运行且失败会写日志。

  ```bash
  git add Dockerfile docker-compose.yml Caddyfile.example scripts/backup.sh src static README.md tests
  git commit -m "chore: harden and document production deployment"
  ```

## 开发顺序与交付切片

Task 10 验收及 Task 1 入口补齐已完成，具体证据见 `docs/testing-deployment.md`、`progress.txt`。采用固定失败延迟；没有增加 P1 限速服务。最终 74 项 Rust 测试及 Compose `config --quiet` 通过，真实本地 HTTPS、浏览器、重启持久化和备份恢复均已演练；真实生产域名证书在上线时另行检查。

1. **可运行骨架：** Task 1-3 完成后，应用能启动、执行 migration、验证管理员/班级口令，并安全写入附件。
2. **学生纵向流程：** Task 4-6 完成后，学生可以从首页进入，提交七类成果或无材料声明，拿到修改码并在允许状态下修改。
3. **管理员纵向流程：** Task 7-8 完成后，管理员可以登录、筛选、查看附件、审核、核定分值和切换学年/口令。
4. **统计与上线：** Task 9-10 完成后，可导出 Excel，部署到 Caddy + Docker，具备日志、备份和验收证据。

P1 功能（分页、筛选导出、图片预览增强、磁盘占用、简单登录限速）只在全部 P0 验收通过后加入；P2 功能不进入本计划。

## 风险与处理

- **SQLite 并发写入：** 写操作使用短事务，必要时配置 busy timeout；单班级规模足以满足 v1。
- **上传占满磁盘：** 限制单文件/数量，后台或日志提供失败信息，备份脚本不覆盖源文件；P1 再补磁盘占用显示。
- **类别字段变化：** 保留 `category_data` JSON 和版本化 migration；新增字段时同步更新校验、模板和 Excel 映射。
- **修改后审核一致性：** 学生修改自动回 `pending`，保留审核备注并记录 `student_modified_after_review`。
- **敏感信息泄露：** 日志、URL、模板和下载响应均禁止输出密码、口令、修改码和内部路径。
- **低配服务器构建：** 在开发机或 CI 构建 release image，服务器只负责拉取和运行镜像。

## 需求覆盖检查

- 学生首页、班级口令、当前学年和截止时间：Task 5、Task 8。
- 七类成果及条件字段：Task 4。
- 日期、字段、附件服务端校验：Task 4、Task 5。
- 申报编号、修改码、成功页：Task 3、Task 5。
- 查询、附件权限、状态修改规则：Task 6。
- 管理员登录、筛选、审核、备注、分值：Task 7。
- 学年和班级口令设置：Task 8。
- 两张 Excel 工作表及格式：Task 9。
- migration、日志、错误页、安全、Docker、HTTPS、备份：Task 1、Task 2、Task 3、Task 10。
- 明确不做的多班级、多租户、自动判分、SPA、外部组件：全局约束和交付切片。
