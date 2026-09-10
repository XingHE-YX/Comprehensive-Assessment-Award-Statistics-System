# 本地测试反馈 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use checkbox syntax.

**Goal:** 落实无材料列表、管理员修改码、CET-4/CET-6 与极简 UI 优化，并更新本地预览。

**Architecture:** 继续使用现有服务端渲染单体。声明保留独立实体，在视图层合并列表；新增可空修改码密文与版本列，认证哈希不变；CSS/Askama 负责视觉优化。

**Tech Stack:** 现有 Cargo 精确版本与锁文件保持不变；生产 SQLx SQLite 3.46.1；无新增运行时依赖。

**Spec:** `docs/superpowers/specs/2026-09-10-feedback-polish-design.md`，及用户最新明确要求。

## Global Constraints

- Rust 1.88.0, Axum 0.8.4, Askama 0.14.0, SQLx 0.8.6, tower-sessions 0.14.0；不新增 Node/前端构建链或生产服务。
- 学生内容最大 720px、管理员 1280px；断点 640px/1024px；正文 16px、控件圆角 8px，沿用 FRONTEND_GUIDELINES 的颜色与 4px 间距。
- 所有 SQL 参数绑定；写操作事务/CSRF；所有文本自动转义；秘密、密文和个人输入不得写日志、公共 URL 或 Excel。
- 用户最新“管理员也能查询修改码”覆盖旧的仅哈希/仅成功页规则，但不得明文持久化。
- 原有运行预览与测试数据必须保留；升级由主控制器在验证、备份后执行。未经管理员主动确认不重置任何已有修改码。

### Task 1: 无申报材料合并进管理员列表

**Files:** `src/services/admin.rs`, `src/routes/admin.rs`, `src/db/declarations.rs` (if needed), `templates/admin/index.html`, `tests/admin_flow.rs`, related APP_FLOW/BACKEND descriptions.

**Interfaces:** `DashboardData::load(pool, filter)` continues returning existing result counts and approved totals. Add merged display rows for `Submission` / `StudentDeclaration`, with helpers for kind, identity, submission number (optional), status label, score (optional), date and detail URL (optional). `DeclarationRepo::list(executor, filter)` already exists and uses identity/year predicates.

- [ ] Write a failing administrator test that saves only a declaration, logs in and finds a real table row with its identity, `无申报材料`, timestamp and no result-detail/review link. Add mixed ordering, literal name/student filters, historical year, same identity later submitting a result and duplicate upsert cases. Assert visible records and counters, not source text.
- [ ] Run `cargo test --test admin_flow` and observe the declaration-only table failure.
- [ ] Load declarations with existing predicates, combine display rows ordered by created_at descending with stable tie ordering, and render the mixed table. Keep result status/score counters based on submissions; declaration counter remains separate. Clarify table row count and declaration filtering. No fake submission records or identifiers; no XLSX schema change.
- [ ] Run targeted test and strict Clippy, inspect diff and commit. Report exact red/green evidence.

### Task 2: 管理员查看和手动重置修改码

**Files:** `migrations/0003_admin_edit_codes.sql`, `src/auth/edit_codes.rs`, `src/auth/mod.rs`, `src/domain/submission.rs`, `src/db/submissions.rs`, route wiring/student/query/admin submission handlers, `templates/admin/submission.html`, `tests/admin_edit_codes.rs`, appropriate existing fixtures and core privacy/configuration docs.

**Interfaces:** A non-Debug `EditCodeVault` exposes `new(secret: &[u8])`, `encrypt(submission_no, code)`, `decrypt(submission_no, ciphertext)`. Reuse `tower_sessions::cookie::{Key,Cookie,CookieJar}` PrivateJar's authenticated encryption with a versioned, per-submission cookie name as AAD; this is a server-only envelope and never a browser cookie. Derive valid key material from the configured persistent SESSION_SECRET using the existing key interface. Route builder supplies the vault to services (state or Extension); the old student-only helper may use its explicit development key.

```sql
ALTER TABLE submissions ADD COLUMN edit_code_ciphertext TEXT;
ALTER TABLE submissions ADD COLUMN edit_code_version INTEGER NOT NULL DEFAULT 0 CHECK (edit_code_version >= 0);
```

- [ ] Write red tests: new submitted code is available only to logged-in admin, equals receipt, differs from database hash/ciphertext, remains available after a router restart with same secret, and is absent in ordinary student detail/XLSX/log capture. Tampering, wrong keys and another record's ciphertext must not reveal a code or internal error.
- [ ] Write red legacy/reset tests: migration leaves old hash and data untouched, administrator sees unavailable-original explanation; GET cannot reset; unauthorized/missing-CSRF/unconfirmed POST cannot reset. Confirmed reset atomically changes hash/cipher/version, old code and old verified-student sessions fail, new code works, and status/note/score/files remain equal. Handle stale/concurrent reset and receipt behavior safely.
- [ ] Run `cargo test --test admin_edit_codes` to demonstrate missing capability, then implement envelope, migration and persistence. Keep new-submission hash+cipher metadata in the same transaction; move touched SQL into repository helpers. Add version-aware student authorization on detail/update/attachment (legacy/default version 0 for compatibility). Avoid adding plaintext credentials to domain Debug output or session URLs.
- [ ] Add administrator code disclosure and explicit reset form with clear invalidation confirmation, using protected routes and no-store responses. Legacy/undecryptable data is handled honestly and safely; no automatic reset during upgrade.
- [ ] Update AGENT/PRD/APP_FLOW/BACKEND/README to describe encrypted recovery, unavailable legacy originals, key backup/rotation effects and manual reset. Run auth/query/admin/code/security/export tests and strict Clippy; commit with report.

### Task 3: CET 与极简 UI、表单回退收尾

**Files:** `static/css/app.css`, `templates/layout.html`, student/admin templates, `static/js/submission-form.js`, relevant native JS, `src/routes/fields.rs`, `src/routes/views.rs`, `src/validation/category.rs`, `src/export/xlsx.rs`, student route extraction if fallback needs it, `tests/validation.rs`, browser regression scripts and relevant behavior tests.

- [ ] Write red acceptance for combined CET type and score through create → query → edit → XLSX (numeric score), and legacy CET-6 edit prefilling. New dropdown/score label are exactly `CET-4/CET-6`; retain the legacy `cet6_score` key and accept old CET-6 values. Avoid migrations that rewrite category JSON.
- [ ] Write red cases for no-JS declaration and national award without school-honor field. Implement accessible server-rendered fallback with shared validation, optional school-only preset field and legacy value preservation; no missing-CSRF bypass. Add UTC label to home deadline.
- [ ] Refine single stylesheet/shared templates per design: clean header and context, distinct but non-nested white form sections, consistent spacing, clear radio selection, restrained counts/filter/table groups, aligned review panels and code disclosure. Keep labels, semantic forms, keyboard focus, 8px radii and token-only colors. Remove nonstandard field spacing and avoid !important. No arbitrary visual assets.
- [ ] Update existing browser tests to exercise the new visible labels and declaration rows. Verify ordinary student/admin/settings/export flows plus new features at 320×568,390×844,768×1024,1440×900; include JavaScript-disabled declaration/admin forms and screenshot inspection. Tests must assert behavior rather than freeze CSS text.
- [ ] Run fmt, relevant tests, strict Clippy, then full tests. Update frontend guidelines only to document compatible component refinements, not to erase unmet requirements. Commit with report.

### Task 4: 集成验收并更新现有预览

- [ ] Complete scoped reviews and whole-change review; fix confirmed regressions with covering tests.
- [ ] Build exact amd64 Docker image (existing SQLite 3.46.1 gate), run full HTTPS and new-feature smoke in isolated data, verify migrations preserve old records and attachments, and confirm password/cipher/log boundaries.
- [ ] Pause manual preview only for a controlled backup and replacement. Back up its named-volume SQLite and uploads as a pair, retain the original runtime.env/SESSION_SECRET, named volumes, localhost:3000 and LAN SSH forward. Do not reset any user data or credentials. Recreate application with new image, await health, test read-only visibility of prior records and fresh representative synthetic flow separately.
- [ ] Update progress.txt/lessons.md, current acceptance findings and plan checkboxes with observed results; commit/push the authorized delivery to the existing repository, preserve root runtime notes and unrelated task1 worktree. Return preview URL and concise behavior summary including the legacy-code limitation.
