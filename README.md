# 综测成果申报系统

这是一个使用 Rust、Axum、Askama 和 SQLite 构建的单班级综测成果申报系统。前端采用服务端渲染和原生 CSS/JavaScript，不需要 Node.js 或前端构建工具。

## 本地启动

1. 安装 Rust 1.88.0。
2. 创建本地目录：`mkdir -p data uploads`。
3. 复制 `.env.example` 为 `.env`，并填写所有必需环境变量。
4. `ADMIN_PASSWORD_HASH` 必须是 Argon2id PHC 哈希；`SESSION_SECRET` 必须是至少 32 字节随机数据的 Base64 编码。不要将实际配置提交到 Git。
5. 运行 `cargo +1.88.0 run`。
6. 访问 `http://127.0.0.1:3000/healthz`，应返回 `ok`。

## 配置

| 变量 | 必填 | 说明 |
| --- | --- | --- |
| `APP_ENV` | 是 | `development` 或 `production` |
| `BIND_ADDR` | 是 | 监听地址，例如 `127.0.0.1:3000` |
| `ADMIN_USERNAME` | 是 | 管理员用户名 |
| `ADMIN_PASSWORD_HASH` | 是 | Argon2id PHC 密码哈希 |
| `SESSION_SECRET` | 是 | 至少 32 字节随机数据的 Base64 编码 |
| `DATABASE_URL` | 是 | SQLite URL，例如 `sqlite:data/app.db` |
| `UPLOAD_DIR` | 是 | 附件存储目录 |
| `RUST_LOG` | 否 | 日志级别，默认 `info` |
| `COOKIE_SECURE` | 否 | Cookie 是否仅 HTTPS，生产默认 `true` |
| `MAX_BODY_BYTES` | 否 | 请求体上限，默认 `115343360` |

应用启动时会启用 SQLite 外键、WAL 模式和 5 秒忙等待，并执行已存在的数据库迁移。附件目录不会作为静态文件目录公开。

## 验证

```bash
cargo +1.88.0 fmt --check
cargo +1.88.0 test --test health
cargo +1.88.0 clippy --all-targets --all-features -- -D warnings
```
