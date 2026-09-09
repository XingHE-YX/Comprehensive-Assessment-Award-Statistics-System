# Task 10 安全与部署验收

本任务接通正式 `zongce-web` 入口，沿用现有学生、管理、设置和导出流程。日常启动、部署与恢复操作见 [README](../README.md)；本文件记录可重复的检查及本次验收边界。

## 自动检查

```sh
cargo +1.88.0 fmt --check
cargo +1.88.0 test --test health --test security
cargo +1.88.0 clippy --all-targets --all-features -- -D warnings
cargo +1.88.0 test --all-targets
python3 tests/backup.py
python3 tests/backup_failures.py
bash -n scripts/backup.sh tests/smoke.sh
docker compose config --quiet
```

Compose 检查前按 README 准备仅含公开配置的 `.env` 和含应用秘密的 `.env.production`。`--quiet` 验证配置但不打印展开后的凭据。测试数据库、上传内容及秘密文件均不得提交。

`health` 启动真实二进制，检查配置拒绝、stdin 哈希工具、健康接口及 SIGTERM 退出；`security` 检查安全 Cookie、CSRF、中文错误、声明长度与实际流式请求体上限、请求标识和日志脱敏。既有集成测试继续覆盖 SQL 参数绑定、模板转义、附件路径和权限、审核与导出。

`tests/backup.py` 调用真实脚本与 SQLite CLI，检查 WAL 中已提交数据、附件字节、恢复后查询、九次备份后的七套保留、坏数据库、缺少附件目录和不合法保留数量。`tests/backup_failures.py` 用受控 Docker 状态和命令故障注入，覆盖重启中/已停止容器、失败后服务恢复、路径脱敏与清理错误退出；其 SQLite 和文件操作仍为真实执行。备份只在完整快照发布成功后轮转；失败不得破坏已有完整副本。

## 真实 HTTP 与容器检查

仅在隔离验收环境中运行 `tests/smoke.sh`，它会创建合成申报并审核。通过环境变量提供 `BASE_URL`、`ADMIN_USERNAME`、`ADMIN_PASSWORD`、`CLASS_ACCESS_CODE`，不要把真实凭据直接写在命令行、脚本或输出中。可指定有效学年内的 `SMOKE_OBTAINED_DATE`。使用本地 CA 时提供 `SMOKE_CA_FILE`；证书和主机名验证仍然开启。

```sh
bash tests/smoke.sh
docker compose restart web
docker compose up -d --no-build --wait
```

冒烟覆盖提交 PDF、保护附件、凭据查询、学生修改、管理员审核和两张 Excel 工作表。可用 `SMOKE_STATE_FILE` 保存独占创建、权限 `0600` 的测试凭据文件。重启后创建新会话，使用此文件重新查询同一条记录，确认原附件字节、已通过状态和导出仍可用。内存 session 随重启失效，需要重新登录；业务数据应持续存在。

按 README 执行备份恢复演练：等待原服务停止，用同一快照的 `app.db` 和 `uploads` 成套恢复，保留恢复前目录，再启动并等待健康检查通过。`docker compose start` 返回并不表示应用已就绪；验收请求应在健康检查通过后发送。停服备份、恢复、重启和依赖该服务的浏览器检查应顺序执行。

## 本次验收记录（2026-09-10）

- 本机使用独立 Colima 虚拟机，Docker Engine/client `27.5.1`、Compose `2.35.1`、Caddy `2.9.1`；按 `linux/amd64` 构建并运行 Rust `1.88.0` release 镜像。原 Compose 在虚拟机 `/opt/zongce/app` 验证，数据库、附件、备份使用规范中的 `/opt/zongce` 宿主机目录。
- 两个服务健康检查通过；用 Caddy 本地 CA 校验 HTTPS `/healthz` 返回 `ok`，HTTP `/query` 返回 `308` 到 HTTPS。真实 HTTPS 冒烟完成全部步骤。
- 使用包含合成 query/header 标记的 404 请求，并停止 web 制造代理 502，检查应用与 Caddy 日志未包含测试密码、班级口令、修改码、Session Secret、学生姓名、内部存储路径或请求标记。
- web 容器重启后，新会话找回原已审核申报，附件字节一致，Excel 可下载。运行九次真实自动停启备份后保留七套快照；从最新快照恢复到新数据目录、保留旧目录后，相同查询、附件和导出检查通过。复制步骤故障注入返回非零，已有副本保留，web 恢复运行。
- 完整重启宿主虚拟机后，Docker `27.5.1` 和两个服务自动恢复，相同申报、附件与导出检查再次通过。另创建真实重启循环容器，先确认 Docker 状态为 `restarting`，再验证修正后的脚本能停稳该容器、保存完整数据库快照、恢复服务运行意图并清理锁；测试容器随后正常移除。
- 既有管理员、设置、学生及七类别导出浏览器套件通过临时入口适配器启动真实 main，顺序通过 `320×568`、`390×844`、`768×1024`、`1440×900`；平板管理员表单禁用 JavaScript。新增中文 403/404 页面也通过四尺寸检查，手机和桌面截图已检查。未发现页面横向溢出或控件重叠。
- 最终 Rust 全量检查为 74 项通过；格式、严格 Clippy、真实 WAL/恢复/保留检查、八种备份故障场景、脚本语法和 Compose 配置检查通过。审查发现的 Argon2 版本/盐约束、重启状态处理、错误脱敏与清理退出问题均已修正并通过定向复审。

本地 TLS 验收使用隔离 CA，没有申请实际生产域名证书。上线时仍需按 README 配置真实 DNS、开放 80/443，并检查公网证书签发。日常一致性备份会短暂停止 web；应安排维护窗口。浏览器工具和 Colima 都属于开发验收工具，不进入生产镜像。
