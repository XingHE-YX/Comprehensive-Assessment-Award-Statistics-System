# 最终验收矩阵与审查报告

## 当前签署：本地 P0 验收通过

2026-09-10 更新，最终受测应用提交 `aa41e71`（审查范围 `ff3f516..aa41e71`）。用户反馈的极简 UI、无申报材料列表、管理员修改码查询和 CET-4/CET-6 已完成；F1/F2 通过专门回归后关闭，D1/D3 同步关闭。独立分项审查和整批最终审查无 Important/Critical 遗留问题。下方旧版失败证据作为历史记录保留，不代表当前状态。

| 当前验收项 | 修复与证据 | 结论 |
|---|---|---|
| F1 学生无 JavaScript 流程 | 服务端字段刷新有登录/CSRF 检查且不保存数据；真实浏览器完成声明、类别切换和修改。`submission_flow`/`query_flow` 覆盖字段保留、权限及无写入；`admin_edit_codes` 覆盖刷新期间重置撤权。启用 JS 时另测 Enter 提交。 | 关闭 |
| F2 校级荣誉规则 | 校级字段可选且有六项预置；非校级合法输入通过；历史自定义文本在查询/编辑后仍保留。`validation`/`query_flow` 与学生浏览器覆盖。 | 关闭 |
| 无申报材料列表 | 声明有独立真实行、身份、时间与标签，不伪造成果编号/分数/审核入口；混合排序、字面筛选、历史学年、重复声明及之后提交成果由 `admin_flow` 覆盖。 | 通过 |
| 管理员修改码 | 新码只在受保护管理员详情及一次性成功页显示。`admin_edit_codes` 覆盖加密/篡改/跨记录/错密钥、旧记录、CSRF/确认/版本条件重置、回滚、并发上传、旧会话/回执撤销、审核/附件不变及学生/Excel/日志无泄露。真实镜像迁移与重启另验。 | 通过 |
| CET 与 UI | 创建→查询→编辑→XLSX 保留数值分数，旧 CET-6/cet6_score 兼容；四尺寸截图核对无溢出/重叠。单 CSS、原生表单、既有颜色/间距令牌与断点不变；首页 UTC 和 CSS 规范偏差已修复。 | 通过，D1/D3 关闭 |
| 集成/部署 | 95 项 Rust、fmt、严格 Clippy、备份及八种故障检查、Compose 配置通过；实际 amd64 SQLite 3.46.1 镜像通过 CA 验证 HTTPS 冒烟、日志隐私、迁移与重启。当前本地预览经成套备份后升级，原配置、所有旧表列摘要及附件清单核对一致。 | 本地通过 |

最终 `admin_browser.cjs`、`settings_browser.cjs`、`export_browser.cjs`（含学生）按顺序在 320×568、390×844、768×1024、1440×900 运行通过。导出样例增加为 22 条成果与 4 条声明，固定两表 34/10 列保持不变。无 JS 学生流程为本次新增直接浏览器证据，不再由管理员无 JS 测试推断。

仍保留的边界：旧哈希原修改码无法还原，只有管理员明确确认才能重置；SESSION_SECRET 必须安全保留和备份，轮换需迁移密文。D2 有界内存附件缓冲仍存在，不宣称直接流式落盘。九备份留七套与恢复沿用此前真实演练，并通过本次备份回归及实际预览备份补验；没有重复声称本次执行九次。未执行公网证书上线或新的办公软件 GUI 验收。

可重复验证入口见 [学生流程](testing-student-flow.md)、[管理员流程](testing-admin-flow.md)、[导出](testing-export.md)、[部署](testing-deployment.md)；完整当前六类矩阵见 [实施计划](../IMPLEMENTATION_PLAN.md)。本地测试可继续，P1/P2 与公网部署作为独立后续工作。

## 历史审查：7ad62cf 未通过

日期：2026-09-10。受测代码：`7ad62cf6d1cef9310384d8f4f4f24a491d7d7acb`。

**结论：最终验收未通过，F1、F2 两项学生端功能缺口待修复。** 本轮完成验收与审查，记录现有实现的问题；修复后需要重新验收，当前不能签署“全部 P0 完成”。此前 Task 10 的安全、部署和备份检查仍有有效证据，项目总体状态以本报告为准。

## 六类验收结果

| 验收领域 | 结论 | 本轮证据 |
|---|---|---|
| Student submit | 未通过：F1、F2 | 常规 JS 流程、七类成果及条件分支、无材料声明、JPG/JPEG/PNG/PDF 通过；无 JS 声明阻断，非校级荣誉合法输入被拒绝。110 例独立 HTTP 验收中 109 例符合预期，1 例为 F2。 |
| Student edit | 状态/授权核心项通过；共用类别校验受 F2 影响 | 错误修改码、单条 session 范围、四种状态、回到待审核、保留备注、原学年规则、附件累加、审核并发和事务回滚通过。 |
| Admin | 通过 | 登录退出、CSRF 与权限、五项筛选、统计、详情与附件、四状态审核、分值、学年创建/编辑/激活、历史保留及口令更新通过。 |
| Export | 通过 | 两张表、34 列明细/10 列汇总、七类展开、仅 Approved 汇总、字符串编号、空结果/历史/筛选、格式、公式惰性及受保护附件标识通过。 |
| Security | 核心项通过 | Argon2id、Cookie、session、CSRF、附件路径与授权、声明/实际请求体上限、中文错误与日志脱敏通过；上传缓冲实现差异见 D2。 |
| Deployment | 本地验收通过 | Docker/Compose/Caddy/SQLx 版本、HTTPS 与 308、持久化、新会话恢复、九次备份保留七套、成套恢复通过。公网域名证书属于实际上线检查。 |

## 待修复的验收阻断项

### F1：关闭 JavaScript 后无法提交无成果声明

依据：`FRONTEND_GUIDELINES.md` 第 4 节要求服务端 HTML 无 JavaScript 可用，`APP_FLOW.md` 将 JavaScript 定位为渐进增强；无成果声明是 PRD 的 P0 功能。

位置：`templates/student/form.html:21` 初始隐藏声明确认；`:24` 以下成果区域仍启用；`:26`、`:28`、`:30`、`:61` 保持成果名称、日期、类别及附件必填。只有 `static/js/submission-form.js:9` 的切换逻辑能解除这些限制。

复现：在禁用 JavaScript 的真实浏览器中通过班级口令，填写姓名和学号，选择“本学年无成果，提交声明”，点击提交。实际观察：确认区仍隐藏、附件仍必填，四个成果控件无效，`POST /submit` 次数为 0，页面停留在填写页。该现象在实际 SQLite 3.46.1 应用镜像复现。

修复验收要求：声明确认及身份字段能在无 JS 情况下独立提交；无关成果/附件控件不能阻止声明。应补充无 JS 的服务端切换或独立声明入口，并回归普通 JS 流程。

### F2：非校级荣誉被错误要求填写校级类别，且预置选项缺失

依据：`PRD.md` 第 5 节要求校级荣誉预置选项；项目指定原始需求第 6.3 节补充“若为校级，可选校级荣誉类别”，列出优秀团务工作者、魅力团支书、优秀共青团干部、优秀共青团员、五四奖章、其他校级荣誉六项（含“其他”）。

位置：`src/validation/category.rs:98` 无条件调用 `required_text`；`src/routes/fields.rs:101` 定义必填自由文本，未配置选项或级别条件。创建和修改共用这套校验。

复现：向真实应用提交其他必填字段和 JPG 附件均合法的国家级荣誉，不提供 `school_honor_category`。按需求应接受，实际返回 422 并显示“校级荣誉类别为必填项”。浏览器实际控件是 `INPUT`、`required=true`、预置选项数 0。110 例 HTTP 矩阵中的唯一失配是这个合法输入被拒绝。

修复验收要求：按表彰级别处理校级类别的适用性及可选性，提供上述预置选择，复用到创建/修改，并验证历史已存文本仍可查看。国家级、省部级等无关类别不应被强制补填校级字段。

## 本轮验证及覆盖

- `cargo +1.88.0 fmt --check`、`cargo +1.88.0 test --all-targets`（74 项）、严格 Clippy、脚本语法和 `git diff --check` 均通过。
- `python3 tests/backup.py` 验证真实 WAL、附件字节、七套保留、恢复和失败；`python3 tests/backup_failures.py` 的八种服务状态/命令故障场景通过。
- `admin_browser.cjs`、`settings_browser.cjs`、`export_browser.cjs`（含学生回归）通过临时适配器启动实际 amd64 应用镜像，顺序通过 320×568、390×844、768×1024、1440×900。平板管理员表单禁用 JavaScript；这不代表学生无 JS 分支也通过。
- 桌面学生样例覆盖七类、两种文章和四种证书分支；实际导出 17 条已修改成果与 4 条声明，ZIP/XML 检查表名、列、中文、类型和分值。移动端与桌面截图已检查，未发现页面溢出或控件重叠。
- 独立 HTTP 矩阵从已成功的分支样例逐个删字段：13 个合法分支基线、48 个类别逐字段缺失用例，以及身份、日期、长度、枚举、证书数值、CSRF、声明、无/空附件、扩展名/MIME、10 MiB/10 文件边界及超限，共 110 例。109 例符合预期，F2 为唯一失配；明细见 [HTTP 验收表](acceptance/2026-09-10-http-matrix.md)。不存在把同一输入中的多个错误当作独立覆盖的推断。
- 补充真实 JPG/JPEG 上传与查询后下载：两种扩展名均保存成功，两个受保护下载均为 200、`image/jpeg`，字节一致；PNG 和 PDF 由既有浏览器/HTTP 套件覆盖。
- Docker Engine/client 27.5.1、Compose 2.35.1、Caddy 2.9.1；运行容器 SQLx `database_ready.sqlite_version` 为 3.46.1。Compose `config --quiet`、本地 CA 验证的 HTTPS 完整提交/修改/审核/XLSX 冒烟和 HTTP 308 通过。
- 本轮读取原持久化申报后，运行九次真实自动停启备份，确认保留七套；从最新完整快照恢复并等待健康检查，通过新会话读取本轮申报、原附件与 Excel。受测宿主目录为规范规定的 `/opt/zongce/data`、`uploads`、`backups`，仅使用隔离 VM 内的合成数据。
- 学生、管理员/导出、安全/部署三项独立源码审查均已完成。安全、部署及管理员/导出未发现新的已确认阻断；学生审查结论与实际复现一致。

证据入口：`tests/{submission_flow,query_flow,admin_flow,settings,export,auth,storage,validation,health,security}.rs`，三个浏览器套件，两个备份检查及 `tests/smoke.sh`。例如 `approved_and_rejected_submissions_are_read_only`、`pending_and_needs_revision_can_be_resubmitted_with_review_note_preserved`、`settings_changed_during_body_upload_are_rechecked_before_persistence`、`summary_unions_declarations_groups_both_identity_fields_and_sums_only_approved` 直接对应矩阵关键要求。

## 非阻断项与证据边界

- D1：`templates/student/home.html:2` 用 UTC 时钟数字显示截止时间但没有标明 UTC。设置与详情已有 UTC 标签；应统一显示约定。本轮没有发现服务端截止判定错误。
- D2：`src/routes/submission_form.rs:26` 先把有大小限制的附件读入 `Vec`，之后写临时磁盘文件并原子保存；这与 BACKEND 第 7 节“直接流式写临时文件”的描述有差异。上限、权限与失败清理验证通过，本轮未观察到服务故障，不能把当前实现描述成直接流式落盘。
- D3：`static/css/app.css:61` 的 6px 间距及 `:97` 的 `!important` 与设计细则不完全一致；作为样式规范待整理项记录，未引起本轮宽度/布局失败。
- 本轮未申请公网证书，也未在真实生产服务器操作。实际域名 DNS/证书签发应在选定服务器上线时验证。内存 session 随重启失效，恢复检查使用新会话。
- 本轮 Excel 检查为实际浏览器下载与独立 ZIP/XML 校验；LibreOffice 打开/重存证据沿用 Task 9 对未改变的导出实现所做的记录，不宣称本轮重新进行了办公软件 GUI 验收。
- 历史“先红后绿”的执行步骤不能由今天的绿测试倒推。计划复选框只按现有源码/仓库测试证据同步，Task 4 的类别契约与相关固定回归仍需完善。

下一步：修复 F1、F2，固化对应回归，重跑受影响学生流程及矩阵，再作 P0 最终签署；P1/P2 功能不进入这轮收尾。
