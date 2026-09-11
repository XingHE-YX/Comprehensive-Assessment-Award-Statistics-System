# 综测成果申报系统

单班级、多个学年的服务端渲染应用。学生使用班级口令申报，每项成果独立获得编号和修改码；管理员审核并导出两张工作表。Rust 1.88.0 / Axum 0.8.4 / SQLite / Askama；生产仅运行 `web` 和 Caddy 2.9.1，无前端构建链。

生产镜像的 SQLx 引擎固定为 SQLite **3.46.1**，由 Dockerfile 下载官方源码、核对 SHA256 后构建并静态链接。当前锁定的 Rust 驱动在未配置外部链接的本机开发构建中附带 **3.46.0**；这不是生产基线，也不能通过安装 `sqlite3` 命令行程序来改变。版本与编译选项说明见 [TECH_STACK.md](TECH_STACK.md)。

## 本地启动与首次初始化

安装 Rust 1.88.0，检查版本和依赖基线见 [TECH_STACK.md](TECH_STACK.md)。本地开发另需 Python 3（标准库用于工作簿与运维验收）、SQLite CLI；生产应用本身不依赖 Python。

```bash
rustup toolchain install 1.88.0
cargo build --locked
mkdir -p data uploads
cp .env.example .env
chmod 600 .env
```

编辑 `.env`，替换管理员用户名、密码哈希与 Session Secret。不要把明文密码写入环境文件、命令历史或 Git。以下 Bash 交互命令隐藏密码输入；生成结果是供配置使用的哈希，仍需保密：

```bash
read -r -s -p '管理员密码: ' zongce_password
printf '\n'
printf '%s\n' "$zongce_password" | target/debug/zongce-web --hash-secret
unset zongce_password
openssl rand -base64 32
```

将哈希填入 `ADMIN_PASSWORD_HASH`（本地 dotenv 文件建议使用单引号包住完整 PHC），将最后一条命令结果填入 `SESSION_SECRET`。密钥必须是严格标准 Base64、解码后至少 32 字节。然后执行：

```bash
cargo run --locked
curl --fail http://127.0.0.1:3000/healthz
```

健康检查返回 `ok`，同时检查数据库连接；故障返回安全的 500 页面。启动自动运行 migrations，不自动创建学年。首次到 `/admin/login` 登录，在“学年设置”下载姓名＋学号模板并导入名单，校验通过后填写学年信息、创建并激活，同时设置班级口令。现有学年保留；空名单学年须先进入“管理名单”补充学生再激活。无需手工写 SQL；班级口令只保存 Argon2id 哈希，修改码另保存认证加密密文。

名单支持 `.xlsx`、UTF-8 CSV 或从 Excel 粘贴两列（含“姓名”“学号”表头）。模板将学号设为文本，避免前导零和长学号丢失。每次限 2 MiB、5000 行；重复记录跳过，同一学号对应不同姓名时整批不保存。已开放学年仍可追加名单，不覆盖原名单，也不延长截止时间。学生新增成果、无材料声明及修改申报均须匹配对应学年的姓名和学号。

申报列表支持单条和批量删除（每次最多 500 条），经确认进入“回收站”。所有审核状态及无材料声明均可删除，删除后退出统计/后续导出，学生不能查询、修改或下载附件；恢复保留原审核状态、分数和附件。同一声明已重新提交时恢复会提示冲突。删除整个学年则须输入完整名称并明确确认，永久清理该学年的名单、申报及附件（含回收站），可重新创建同名学年；其他学年及系统配置保留。删除当前学年会关闭申报入口。失败的物理附件清理保存在数据库队列中，后续清理和启动时重试。

启动日志中的 `database_ready` 事件包含从应用 SQLx 连接查询得到的 `sqlite_version`，不暴露数据。`scripts/check-sqlite-engine.sh` 接受应用二进制绝对路径和预期版本，用临时凭据/数据库启动该二进制并检查事件，然后优雅退出。Dockerfile 强制对最终 release 产物执行此检查，预期值为 3.46.1；它不会把外部 CLI 的版本当成应用引擎证据。

若需要本机与生产引擎一致，先用 Dockerfile 相同 SHA256 和 CFLAGS 为本机编译 SQLite 3.46.1，安装到独立目录（示例 `/opt/sqlite-3.46.1`），然后：

```bash
export LIBSQLITE3_SYS_USE_PKG_CONFIG=1 SQLITE3_STATIC=1
export SQLITE3_LIB_DIR=/opt/sqlite-3.46.1/lib
export SQLITE3_INCLUDE_DIR=/opt/sqlite-3.46.1/include
export PKG_CONFIG_PATH=/opt/sqlite-3.46.1/lib/pkgconfig
export CARGO_TARGET_DIR=target/sqlite-3.46.1
cargo build --locked --release
bash scripts/check-sqlite-engine.sh "$PWD/target/sqlite-3.46.1/release/zongce-web" 3.46.1
```

本机需 C 编译工具、make、pkg-config；编译不同引擎时使用独立 target，避免混淆已有开发产物。生产仍以 Dockerfile 和其中实际 release 引擎门禁为准，Cargo.toml/Cargo.lock 不需变更。

`Ctrl-C` 或 `SIGTERM` 会停止接收新连接、等待已接收请求结束并关闭连接池。会话采用内存存储，重启后需重新登录/验证；数据库、已提交成果和附件不受影响，学生用保存的编号和修改码再次查询。成功页修改码只显示一次。

管理员可在申报详情的“学生修改码”中查看新提交记录的修改码。旧记录只有哈希，原码无法恢复，升级不会自动重置；学生保存的原码仍可使用。若需要替换，展开“生成新修改码”，明确勾选失效确认后提交。旧码、已验证的学生会话及未查看的旧成功页凭据立即失效，学生须用新码再次查询；申报内容、审核状态/备注/分值和附件不变。重复或过期的重置表单会提示刷新，不会覆盖刚生成的凭据。修改码不显示在普通学生详情、列表或 Excel，也不写入日志或 URL。

`SESSION_SECRET` 同时保护修改码密文，必须持久保存，并与数据库备份对应的受保护配置一起安全备份。现有备份脚本只备份数据库和附件，配置密钥需单独保密备份。保持原密钥升级和重启；密钥轮换需要使用旧、新密钥迁移密文，不能只改环境变量。丢失或改变密钥会使旧密文无法读取，页面会安全说明；学生保存的有效码仍可通过哈希验证。不要为恢复查看能力而自动重置旧记录。

## 配置与日志

| 变量 | 约定 |
| --- | --- |
| `APP_ENV` | 必填，`development` / `production` |
| `BIND_ADDR` | 必填，容器为 `0.0.0.0:3000` |
| `ADMIN_USERNAME` | 必填非空用户名 |
| `ADMIN_PASSWORD_HASH` | 必填有效 Argon2id PHC，禁止明文 |
| `SESSION_SECRET` | 必填至少 32 随机字节的标准 Base64 |
| `DATABASE_URL` | 必填 SQLite URL；容器 `sqlite:/data/app.db`；父目录需预先建立 |
| `UPLOAD_DIR` | 必填；容器 `/uploads`，启动创建目录 |
| `COOKIE_SECURE` | 默认生产 true，开发 false；生产拒绝 false |
| `MAX_BODY_BYTES` | 正整数，默认 115343360（10 个 10 MiB 文件及表单开销） |
| `RUST_LOG` | 默认 info，仅接受 off/error/warn/info/debug/trace；其他格式回退 info |

所有请求分配服务端生成的 `x-request-id`，日志只记录匹配路由模板和必要状态/申报标识；不会信任请求头传来的 id，或记录原始 URL、查询参数、Cookie、密码、修改码、个人表单输入、SQL、绝对存储路径。依赖库日志固定关闭，`RUST_LOG=trace` 也不能开启 SQLx/session 的敏感诊断。启动失败仅记录配置变量名或失败阶段。统一中文错误页覆盖提取失败、403/404/413/500，包含 no-store；请求体同时检查声明长度和实际流式读取上限。管理员、班级口令和查询失败均有固定 100ms 延迟，这不是限速系统。

## 容器部署

目标主机 Ubuntu 24.04、Docker Engine 27.5.1、Docker Compose 2.35.1。默认镜像平台为 `linux/amd64`。在开发机或 CI 构建 release 镜像，生产机器只加载/拉取成品镜像：

```bash
docker build --platform linux/amd64 -t zongce-web:1.0.0 .
docker save -o zongce-web-1.0.0.tar zongce-web:1.0.0
```

将镜像归档及部署文件（`docker-compose.yml`、`Caddyfile.example`、`scripts/`）放到服务器 `/opt/zongce/app`。镜像使用固定 Rust builder、Debian bookworm-slim runtime，内含 curl 健康探测；应用以 UID/GID 10001 运行。不要将 `.env`、数据或附件放进镜像构建上下文。

```bash
sudo install -d -m 750 -o 10001 -g 10001 /opt/zongce/data /opt/zongce/uploads /opt/zongce/backups
cd /opt/zongce/app
docker load -i zongce-web-1.0.0.tar
cp .env.example .env.production
chmod 600 .env.production
```

若只传部署文件，也一并传 `.env.example`。编辑 `.env.production` 中的真实管理员哈希、Session Secret；生产环境使用 `APP_ENV=production`、`COOKIE_SECURE=true`。Compose 固定生产 bind/database/uploads/cookie 值。**该文件以 `format: raw` 读取，哈希和其他值不要加引号，保留原始 `$` 字符。** 不要对该文件执行 `source`，不要在公开输出中运行展开配置的 `docker compose config`。

另建只含 `DOMAIN=你的公共域名` 和可选 `ZONGCE_IMAGE=你的镜像引用` 的 `/opt/zongce/app/.env`，不要把秘密放进这个 Compose 插值文件。Caddy 仅接收 DOMAIN，管理员哈希与 Session Secret 只传给 web。将域名 DNS 指向服务器并开放 TCP 80/443；不用 `http://` 前缀，Caddy 自动申请证书并把 HTTP 重定向 HTTPS。

```bash
docker compose config --quiet
docker compose up -d --no-build
docker compose ps
curl --fail https://你的公共域名/healthz
```

只有 Caddy 映射公网端口，web 的 3000 端口仅容器网络可达。固定挂载 `/opt/zongce/data:/data`、`/opt/zongce/uploads:/uploads`、`/opt/zongce/backups:/backups`；Caddy 证书/config 使用命名卷并跨容器重启保留。勿使用 `docker compose down -v` 删除证书卷。web 使用只读根文件系统、去除 capabilities 和 no-new-privileges，数据库和附件通过宿主持久化目录写入；备份挂载只读。两个服务均 `restart: unless-stopped`，宿主机启用 Docker 开机启动。Caddy 禁用访问日志，并从错误日志删除原始 request/内部堆栈字段；容器 JSON 日志每份 10 MiB、保留 3 份。

上线前先完成下面的备份恢复演练。更新镜像前执行备份，再加载新镜像并 `docker compose up -d --no-build`；migrations 启动时自动执行。数据库 schema 变更后回退旧应用前，先评估是否必须恢复更新前整套快照。

## 每日备份

主机需 Bash、SQLite CLI 和 Docker Compose。执行 `sudo /opt/zongce/app/scripts/backup.sh`：脚本获取互斥锁，读取 web 容器的明确状态，短暂停止 running 或 restarting 的 web，并确认容器已经停止，再用 SQLite `.backup`（包含已提交 WAL）和 uploads 创建同一时点的完整快照。未知/暂停等状态安全失败，不制作快照。通过 `integrity_check` 后原子发布 `snapshot-*`，恢复原本有运行意图的 web，然后轮转，默认至少保留最近 7 份完整数据库及其附件。不存在把 WAL 模式主数据库直接 `cp` 后当成完整备份的步骤。

每天备份有短暂维护窗口（期间 Caddy 可返回 502）；上传量大时窗口会增长。失败返回非零并写 stderr 和 `/opt/zongce/backups/backup.log`，不轮转未完成快照；即使备份失败，也尝试恢复原本运行的 web。原本已停止的 web 保持停止。若服务恢复失败，日志明确 `stage=service_restart`，需运维处理。日志、快照默认仅属主可读写。`SIGKILL` 或主机断电无法触发清理，遗留 `.backup-lock` 时确认没有备份进程后再用 `rmdir` 移除锁；不要并行执行多个备份。

可覆盖 `DATA_DIR`、`UPLOAD_DIR`、`BACKUP_DIR`、`PROJECT_DIR`，均默认规定的 `/opt/zongce` 路径；`BACKUP_KEEP` 允许增加，不能低于 7。拒绝根目录、重叠目录和顶层软链接。`--offline` 只适用于调用者已经确保应用和其他写入者停止的恢复演练，不要用于正在接收请求的生产库。

root crontab 示例（`sudo crontab -e`）：

```cron
PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
15 3 * * * /opt/zongce/app/scripts/backup.sh
```

或者用 systemd 替代 cron。`/etc/systemd/system/zongce-backup.service`：

```ini
[Unit]
Description=Zongce consistent database and upload backup
After=docker.service
Requires=docker.service
[Service]
Type=oneshot
User=root
ExecStart=/opt/zongce/app/scripts/backup.sh
```

`/etc/systemd/system/zongce-backup.timer`：

```ini
[Unit]
Description=Daily Zongce backup
[Timer]
OnCalendar=*-*-* 03:15:00
Persistent=true
[Install]
WantedBy=timers.target
```

执行 `sudo systemctl daemon-reload`、`sudo systemctl enable --now zongce-backup.timer`，用 `journalctl -u zongce-backup.service` 查看退出结果。cron/timer 二选一。主机磁盘中的副本不能防整机丢失；按学校运维策略另存完整快照及受保护配置到独立介质。

## 恢复和上线前演练

先在隔离目录运行 `python3 tests/backup.py`，它实际建立 WAL 数据库、备份 9 次并验证保留 7 份，恢复中文数据和附件；损坏数据库/缺目录必须失败。容器验收还需用真实应用重启后重新查询并下载附件。

正式恢复应在维护窗口选择一个 `.complete` 内容为 `zongce-backup-v1` 的完整快照，数据库和 uploads 必须来自同一个目录。先再做一次当前状态备份（若当前库已损坏，保留其目录供排查），然后 `docker compose stop web`。以下在 root shell 操作，`zongce_snapshot` 必须替换为已检查的具体快照绝对路径：

```bash
zongce_snapshot=/opt/zongce/backups/snapshot-替换为已验证快照
test "$(cat "$zongce_snapshot/.complete")" = zongce-backup-v1
test "$(sqlite3 "$zongce_snapshot/app.db" 'PRAGMA integrity_check;')" = ok
zongce_restore=$(mktemp -d /opt/zongce/restore.XXXXXXXX)
mkdir "$zongce_restore/data"
cp -p "$zongce_snapshot/app.db" "$zongce_restore/data/app.db"
cp -Rp "$zongce_snapshot/uploads" "$zongce_restore/uploads"
chown -R 10001:10001 "$zongce_restore/data" "$zongce_restore/uploads"
mv /opt/zongce/data "$zongce_restore/previous-data"
mv /opt/zongce/uploads "$zongce_restore/previous-uploads"
mv "$zongce_restore/data" /opt/zongce/data
mv "$zongce_restore/uploads" /opt/zongce/uploads
docker compose up -d --no-build --force-recreate web
```

新数据目录不含旧 `app.db-wal`/`app.db-shm`，避免把旧日志混入恢复库。旧 data/uploads 保留在恢复临时目录中，确认验收成功后由运维按保留策略处理；任一步失败时停止后续步骤，使用 `previous-*` 恢复原目录并重启 web。检查 `/healthz`，重新管理员登录，学生编号+修改码查询、附件内容、审核状态与 Excel 都应与所选快照一致。会话不会随快照恢复；重新设置环境配置时保持必要秘密的保密性。Caddy 命名卷中的证书由 Caddy 管理，不在应用数据快照中。

## 验证

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
python3 tests/backup.py
python3 tests/backup_failures.py
bash -n scripts/backup.sh tests/smoke.sh
docker compose config --quiet
```

在隔离的已初始化站点上，设置 `BASE_URL`、`ADMIN_USERNAME`、`ADMIN_PASSWORD`、`CLASS_ACCESS_CODE` 环境变量，再执行 `bash tests/smoke.sh`。它写入标记为“部署验收样例”的数据，实际完成成果上传、附件授权、凭证查询、修改、审核和两表 XLSX 下载；不要对正式数据集重复运行。`SMOKE_OBTAINED_DATE` 可指定当前学年内日期；不指定时取表单显示的首个日期。若设置 `SMOKE_STATE_FILE`，脚本会独占新建 0600 凭证文件供重启恢复复验，属于秘密文件，不能提交或打印。

使用 Caddy 本地 CA 的隔离 TLS 验收可设置 `SMOKE_CA_FILE` 为导出的 CA 证书文件；脚本显式加载该信任根，仍验证证书和主机名，不提供关闭 TLS 验证的选项。公共域名使用默认系统信任库即可。

手机/桌面完整浏览器与工作簿回归见 [学生流程](docs/testing-student-flow.md)、[管理员与设置](docs/testing-admin-flow.md)、[Excel](docs/testing-export.md)。浏览器测试依赖安装在仓库外；生产镜像不包含 Node、Playwright 或 Python。
