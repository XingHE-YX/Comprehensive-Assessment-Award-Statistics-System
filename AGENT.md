# AGENT.md

本文件是“综测成果申报系统（单班级版）”的项目级 AI 开发规则。任何 AI、代码生成器或自动化开发代理在本项目中工作时，都必须遵守本文件以及列出的规范文档。

## 1. 每次会话开始时的强制步骤

每次开始处理任务前，必须按以下顺序执行：

1. 读取项目根目录的 `progress.txt`。
2. 读取项目根目录的 `lessons.md`。
3. 如果其中任一文件不存在，立即创建空的结构化文件，再继续后续工作。
4. 读取与当前任务相关的规范文档；涉及架构、页面、技术栈、数据或实施顺序时，至少读取所有核心规范文档。
5. 检查工作区状态和当前文件结构；不得覆盖、删除或回滚用户已有改动。
6. 在开始编码前明确当前任务对应的规范条目、预计修改文件和验证方式。

每次会话结束前必须：

- 更新 `progress.txt`，记录已完成内容、当前状态、验证命令和下一步。
- 只有在确实获得了可复用经验、发现了规范缺口或修复了容易复现的问题时，才更新 `lessons.md`。
- 不在进度或经验文件中写入密码、班级口令、修改码、Session Secret、个人隐私或附件内容。

## 2. 必须参考的规范文档

以下文件是项目事实来源，冲突时优先级如下：用户最新明确要求 > `PRD.md` > `APP_FLOW.md` > `BACKEND_STRUCTURE.md` / `TECH_STACK.md` / `FRONTEND_GUIDELINES.md` > `IMPLEMENTATION_PLAN.md` > 旧计划或实现细节。

必须参考：

- [`PRD.md`](./PRD.md)：产品愿景、用户故事、功能范围、业务规则、成功标准和非目标。
- [`APP_FLOW.md`](./APP_FLOW.md)：所有页面、触发条件、导航路径、成功状态和错误状态。该文件正文使用英文，是页面行为基线。
- [`TECH_STACK.md`](./TECH_STACK.md)：确切依赖版本、运行时、容器、配置、命令和升级规则。
- [`FRONTEND_GUIDELINES.md`](./FRONTEND_GUIDELINES.md)：设计系统、颜色、字体、间距、组件、断点、可访问性和 CSS 约束。
- [`BACKEND_STRUCTURE.md`](./BACKEND_STRUCTURE.md)：模块边界、数据库 schema、路由合约、认证、授权、校验、存储和日志规则。
- [`IMPLEMENTATION_PLAN.md`](./IMPLEMENTATION_PLAN.md)：任务顺序、测试门禁、交付切片和最终验收矩阵。
- [`docs/superpowers/plans/2026-09-07-zongce-web-implementation-plan.md`](./docs/superpowers/plans/2026-09-07-zongce-web-implementation-plan.md)：更详细的模块目录、任务接口和部署风险补充。
- [`progress.txt`](./progress.txt)：当前实现进度和未完成事项；每次会话开始时必读。
- [`lessons.md`](./lessons.md)：已确认的工程经验和易错点；每次会话开始时必读。

外部原始需求文档为 `/Users/xingheluqi/Downloads/zongce-web-dev-requirements.md`。当项目规范没有覆盖某项需求时，回看该原始文档，不要凭空扩展范围。

## 3. 项目技术栈摘要

所有直接依赖必须使用 `TECH_STACK.md` 中的精确版本，并提交 `Cargo.lock`。

- Rust `1.88.0` stable，edition `2024`。
- Axum `0.8.4`，Askama `0.14.0`，Askama Axum integration `0.4.0`。
- Tokio `1.47.1`，Tower `0.5.2`，Tower HTTP `0.6.6`。
- SQLx `0.8.6`，SQLite `3.46.1`，启用 `runtime-tokio-rustls`、`sqlite`、`migrate`、`chrono`、`json`。
- Serde `1.0.219`、Serde JSON `1.0.140`、Chrono `0.4.41`、UUID `1.17.0`。
- Argon2 `0.5.3`、password-hash `0.5.0`、rand `0.9.1`。
- tower-sessions `0.14.0`、multer `3.1.0`、mime `0.3.17`、mime_guess `2.0.5`。
- rust_xlsxwriter `0.89.1` 用于 Excel；tracing `0.1.41` 和 tracing-subscriber `0.3.19` 用于日志。
- thiserror `2.0.12`、dotenvy `0.15.7`、urlencoding `2.1.3`。
- 前端为 Askama 服务端 HTML + 原生 ES2022 JavaScript + 单一手写 CSS；禁止 npm、Node 运行时和前端构建链。
- 部署使用 Docker Engine `27.5.1`、Docker Compose `2.35.1`、Caddy `2.9.1`、Ubuntu `24.04`。

## 4. 文件命名和存放位置约定

### 根目录

- `PRD.md`、`APP_FLOW.md`、`TECH_STACK.md`、`FRONTEND_GUIDELINES.md`、`BACKEND_STRUCTURE.md`、`IMPLEMENTATION_PLAN.md`：项目规范。
- `AGENT.md`：本项目 AI 执行规则。
- `progress.txt`：短文本进度记录。
- `lessons.md`：短文本工程经验记录。
- `Cargo.toml`、`Cargo.lock`：Rust 依赖和锁定版本。
- `.env.example`：环境变量名称和示例占位符，不得包含真实密钥。
- `Dockerfile`、`docker-compose.yml`、`Caddyfile.example`：部署配置。

### Rust 源码

- `src/main.rs`：启动、配置初始化、migration、路由和优雅退出。
- `src/config.rs`：环境变量解析和运行时限制。
- `src/state.rs`：`AppState` 共享状态。
- `src/error.rs`：`AppError`、状态码映射和中文错误响应。
- `src/domain/`：领域类型和固定枚举，不放 HTTP 或 SQL。
- `src/validation/`：所有服务端校验；创建和修改共用。
- `src/auth/`：Argon2id、随机码、session、CSRF 和权限辅助函数。
- `src/db/`：SQLx repository 和事务，不在 route handler 中写原始 SQL。
- `src/storage/`：上传流、随机文件名、路径安全、受保护读取。
- `src/routes/`：请求提取、调用服务、模板渲染、重定向；不得承载业务规则。
- `src/export/`：固定 Excel 列映射、格式和下载响应。

Rust 文件使用 `snake_case.rs`，模块目录使用小写复数或领域名；类型使用 `UpperCamelCase`，函数、变量和字段使用 `snake_case`，常量使用 `SCREAMING_SNAKE_CASE`。一个文件应只有一个清晰职责。

### 模板、静态资源和数据

- `templates/layout.html` 放公共布局；按 `templates/student/`、`templates/admin/`、`templates/errors/` 分目录。
- `static/css/app.css` 是唯一正式 CSS 入口。
- `static/js/submission-form.js` 只处理成果类别条件字段、客户端提示和渐进增强。
- `migrations/NNNN_description.sql` 使用递增编号；所有数据库结构变更必须 migration。
- `scripts/` 只放可审查的运维脚本，如 `scripts/backup.sh`。
- 测试放在 `tests/`，文件按行为命名，例如 `submission_flow.rs`、`admin_flow.rs`、`export.rs`。
- 生产数据库位于 `/data/app.db`，上传根目录为 `/uploads`；不把上传文件放入 `static/`、源码目录或容器临时层。

## 5. 必须遵循的编码模式

### 架构与请求处理

- 使用服务端渲染单体架构；HTTP 读取使用 GET，写操作使用 POST/PUT，不用 GET 修改数据。
- route handler 只做输入提取、认证检查、调用验证/服务/repository、选择模板或重定向。
- 业务规则集中在 domain/service/validation 层，创建和修改必须复用同一套服务端校验。
- 数据库访问统一使用 SQLx 参数绑定；动态筛选使用 `QueryBuilder` 绑定参数；不拼接用户输入进 SQL。
- 多表写入使用事务；提交记录和附件元数据必须保持一致；事务失败时清理临时文件。
- migration 必须可重复应用；不得通过手工修改生产 SQLite 文件改变 schema。

### 数据与安全

- `academic_years` 同时只能有一个 `is_active=1`；历史学年数据保留。
- `category_data` 只保存结构化 JSON；Excel 必须展开为独立列，禁止直接输出整段 JSON。
- 修改码、班级口令和管理员密码只保存 Argon2id 哈希；明文修改码仅在成功页显示。
- 附件使用随机安全文件名，数据库保存原始名、存储名、MIME、大小和时间；上传目录不得公开静态访问。
- 附件访问必须通过管理员 session 或已验证的“申报编号 + 修改码” session。
- 所有写表单使用 CSRF token；session Cookie 使用 `HttpOnly`、`SameSite=Lax`，生产环境 `Secure`。
- 模板自动转义；用户文本不得通过 HTML 字符串拼接渲染。
- 日志使用 tracing；只记录请求 id、路由、申报 id/编号、状态等必要字段，不记录秘密和原始敏感输入。
- 错误统一转换为稳定的中文提示；不把 SQL 错误、panic、文件系统路径或 Rust 类型信息返回给用户。

### 测试与验证

- 新行为先写失败测试，再实现最小代码，再运行定向测试。
- 验证顺序至少为：`cargo fmt --check`、定向 `cargo test`、`cargo clippy --all-targets --all-features -- -D warnings`；交付前运行全量测试和 `docker compose config`。
- 页面行为测试覆盖手机窄屏和桌面宽屏；提交、查询/修改、管理员审核、Excel 导出必须有端到端验证。
- 任何修改路由、数据库、认证、上传或 Excel 的变更，都必须同步检查对应规范文档和验收矩阵。

## 6. 设计系统令牌引用

UI 实现必须以 [`FRONTEND_GUIDELINES.md`](./FRONTEND_GUIDELINES.md) 为准，不自行引入第二套设计系统。CSS 顶部定义以下令牌，组件只能引用令牌而不是散落颜色值：

```css
--color-bg: #F8FAFC;
--color-surface: #FFFFFF;
--color-surface-muted: #F1F5F9;
--color-border: #CBD5E1;
--color-border-strong: #94A3B8;
--color-text: #0F172A;
--color-text-muted: #475569;
--color-primary: #2563EB;
--color-primary-hover: #1D4ED8;
--color-primary-soft: #DBEAFE;
--color-success: #15803D;
--color-success-soft: #DCFCE7;
--color-warning: #B45309;
--color-warning-soft: #FEF3C7;
--color-danger: #B91C1C;
--color-danger-soft: #FEE2E2;
--color-focus: #0EA5E9;
```

固定设计规则：4px 间距刻度；手机页面内边距 16px、桌面 24px；学生内容最大宽度 720px、管理员 1280px；断点为 640px 和 1024px；控件圆角 8px；正文 16px；中文字体栈使用系统无衬线字体；字距始终为 0；状态必须同时使用文字和颜色表达。

本项目不使用 shadcn/ui 包，因为前端规范禁止 Node 构建链。需要的 Field、Notice、Button、StatusBadge、DataTable 等组件使用 Askama partial + 原生 HTML/CSS 实现，并遵循同一组令牌。

## 7. 明确禁止的操作

- 禁止引入 React、Vue、Leptos、Yew、Node.js、npm、Vite、Webpack 或任何前端构建链。
- 禁止引入 Redis、PostgreSQL、消息队列、对象存储、微服务、多租户或额外运行时组件。
- 禁止擅自实现 P2 功能：多班级、多管理员、自动判分、规则配置中心、消息通知等。
- 禁止在代码、测试、日志、`.env`、README 或提交信息中写入真实密码、班级口令、修改码、Session Secret、生产域名密钥或个人隐私。
- 禁止将附件放入静态公开目录、使用可猜测文件名、信任原始文件名作为路径或直接暴露数据库自增 id 作为学生凭证。
- 禁止跳过服务端校验、只依赖前端校验、绕过 CSRF、关闭模板转义或使用未经参数绑定的 SQL。
- 禁止把 `category_data` 原 JSON 作为 Excel 唯一内容；必须按固定列展开。
- 禁止用 GET 执行登录、审核、删除、修改状态、修改口令或其他写操作。
- 禁止用 `unwrap()`/`expect()` 处理用户输入、数据库、上传、网络和生产配置错误；必须转换为 `AppError` 或记录后返回安全错误页。
- 禁止修改无关文件、删除用户已有改动、执行 `git reset --hard`、`git checkout --` 或宽范围递归删除。
- 禁止在没有先阅读 `progress.txt`、`lessons.md` 和相关规范文档的情况下开始编码。
- 禁止宣称“已完成”而未运行与变更风险匹配的验证命令。

## 8. 工作交付规则

每个任务开始时说明：依据的规范文件、目标文件、业务影响和测试方式。每个任务完成时说明：实际修改、验证命令及结果、未完成事项和风险。若发现规范冲突，暂停相关实现并以最新用户要求为准，同时在 `progress.txt` 记录决策；不得默默选择一个解释。
