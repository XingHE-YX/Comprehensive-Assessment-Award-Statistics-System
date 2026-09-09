# 技术栈与版本基线

版本基线日期：2026-09-07。所有直接依赖必须在 `Cargo.toml` 中写精确版本号，禁止使用 `*`、范围约束或未锁定的 CDN 资源；提交 `Cargo.lock`。下面版本是 v1.0 的唯一允许基线，升级必须单独评审并更新本文档。

## Runtime

- Rust toolchain: `1.88.0` stable, edition `2024`。
- Target: `x86_64-unknown-linux-gnu`。
- Runtime OS: Ubuntu `24.04`。
- Container base: `debian:bookworm-slim` runtime image; builder `rust:1.88.0-bookworm`。
- Web server: Axum `0.8.4`。
- Async runtime: Tokio `1.47.1`, features `macros`, `rt-multi-thread`, `signal`, `net`。
- Database engine: SQLite `3.46.1` via SQLx `0.8.6`; enable foreign keys, WAL, and a 5-second busy timeout at connection setup.
- Container engine: Docker Engine `27.5.1`。
- Compose specification: Docker Compose plugin `2.35.1` using `compose.yaml` schema `3.9` compatibility fields only where required by the plugin.
- Reverse proxy/TLS: Caddy `2.9.1`。

## Rust dependencies

```toml
[dependencies]
axum = { version = "0.8.4", features = ["multipart", "macros"] }
askama = "0.14.0"
askama_axum = "0.4.0"
tokio = { version = "1.47.1", features = ["macros", "rt-multi-thread", "signal", "net"] }
tower = "0.5.2"
tower-http = { version = "0.6.6", features = ["fs", "trace", "request-id", "timeout"] }
tower-sessions = "0.14.0"
sqlx = { version = "0.8.6", default-features = false, features = ["runtime-tokio-rustls", "sqlite", "migrate", "chrono", "json"] }
serde = { version = "1.0.219", features = ["derive"] }
serde_json = "1.0.140"
chrono = { version = "0.4.41", features = ["serde"] }
uuid = { version = "1.17.0", features = ["v4", "fast-rng"] }
rand = "0.9.1"
argon2 = "0.5.3"
password-hash = "0.5.0"
subtle = "2.6.1"
multer = "3.1.0"
mime = "0.3.17"
mime_guess = "2.0.5"
rust_xlsxwriter = "0.89.1"
tracing = "0.1.41"
tracing-subscriber = { version = "0.3.19", features = ["env-filter", "fmt", "json"] }
thiserror = "2.0.12"
dotenvy = "0.15.7"
urlencoding = "2.1.3"

[dev-dependencies]
axum-test = "17.2.0"
tempfile = "3.20.0"
tokio = { version = "1.47.1", features = ["macros", "rt-multi-thread", "test-util"] }
```

`sqlx` queries use compile-time checked `query!`/`query_as!` only after migrations are available to the build; dynamic filter predicates use `QueryBuilder` with bound parameters. `rust_xlsxwriter` is the only workbook writer. No ORM, GraphQL server, Redis client, frontend framework, CSS framework, or analytics SDK is allowed.

## Frontend assets and tools

- HTML: Askama `0.14.0` templates compiled in the Rust binary.
- JavaScript: browser-native ES2022, no npm, bundler, transpiler, or runtime dependency.
- CSS: one checked-in `static/css/app.css`; no remote stylesheet.
- Icons: inline text or CSS-safe symbols only; no icon CDN in v1.
- Browser test: Playwright `1.52.0` may be installed outside the production image for smoke tests.
- XLSX test inspection: Python `3.9.6` (standard library `zipfile`/`xml.etree`/`json`, development checks only). `cargo test --test export` invokes `python3` to inspect generated ZIP/XML independently of the writer. No Python application runtime or pip dependency is introduced.

## HTTP and data formats

- HTML routes use `GET` for reads and `POST` for mutations.
- HTTP protocol: HTTP/1.1 between Caddy and the app; Caddy may serve HTTP/2 to browsers; no HTTP/3 requirement in v1.
- HTML standard: WHATWG HTML Living Standard as implemented by Chromium `134+`, Safari `18+`, and Firefox `136+`.
- JavaScript standard: ECMAScript 2022 (ES13); no transpilation target is required.
- Forms use `application/x-www-form-urlencoded`; submission forms use `multipart/form-data`.
- File responses set `Content-Type` from the database MIME and `Content-Disposition` with a sanitized original filename.
- Dates use ISO `YYYY-MM-DD`; timestamps use UTC RFC 3339 in HTML and SQLite text storage.
- JSON appears only in the internal `category_data` column and session payloads; no public JSON API is required for v1.

## Configuration

Required environment variables:

```text
APP_ENV=development|production
BIND_ADDR=0.0.0.0:3000
ADMIN_USERNAME=<non-empty>
ADMIN_PASSWORD_HASH=<argon2id PHC string>
SESSION_SECRET=<at least 32 random bytes, base64 encoded>
DATABASE_URL=sqlite:/data/app.db
UPLOAD_DIR=/uploads
```

Optional: `RUST_LOG` (default `info`; only a level is accepted and dependency logs remain disabled), `COOKIE_SECURE` (default true in production; false is rejected in production), `MAX_BODY_BYTES` (positive, default 115343360 for ten 10 MiB files plus form overhead). Standard padded Base64 is required for `SESSION_SECRET`; malformed trailing content is rejected. Config values and error sources never appear in logs.

The production binary reads dotenv configuration, migrates/seeds SQLite, uses the configured upload directory and serves the existing router until graceful SIGINT/SIGTERM shutdown. `--hash-secret` reads one secret line from stdin and prints an Argon2id PHC without requiring runtime configuration. `GET /healthz` returns `ok` only while the database readiness query succeeds.

Deployment uses `.env.production` with Compose `env_file.format: raw` so PHC `$` characters are literal (values must not be quoted). A separate `.env` contains only DOMAIN and optional ZONGCE_IMAGE interpolation values; Caddy receives no application credentials. The runtime image runs as UID/GID 10001; mandatory host mounts are `/opt/zongce/data`, `/opt/zongce/uploads`, `/opt/zongce/backups`. See README for release build, daily cold snapshots and full restore procedures.

## Commands

```bash
rustup toolchain install 1.88.0
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo run
docker compose config
docker compose up -d
```

## Version policy

Security patch updates require a lockfile refresh, full test run, and changelog entry. Minor/major updates require checking Axum extractor behavior, Askama template compilation, SQLx migration behavior, session cookie serialization, and Excel output compatibility before adoption.
