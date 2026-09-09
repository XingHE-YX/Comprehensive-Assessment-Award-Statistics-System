use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use zongce_web::auth::{generate_edit_code, hash_secret};

fn command(dir: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_zongce-web"));
    command
        .current_dir(dir)
        .env_clear()
        .env("APP_ENV", "development")
        .env("BIND_ADDR", "127.0.0.1:0")
        .env("ADMIN_USERNAME", "test-admin")
        .env(
            "ADMIN_PASSWORD_HASH",
            hash_secret(&generate_edit_code()).unwrap(),
        )
        .env(
            "SESSION_SECRET",
            "QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVowMTIzNDU2Nzg=",
        )
        .env("DATABASE_URL", "sqlite::memory:")
        .env("UPLOAD_DIR", dir.join("uploads"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

#[test]
fn startup_rejects_missing_and_malformed_configuration_without_leaking_values() {
    let dir = tempfile::tempdir().unwrap();
    for (key, value) in [
        ("APP_ENV", None),
        ("ADMIN_PASSWORD_HASH", Some("private-invalid-hash")),
        (
            "SESSION_SECRET",
            Some("QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVowMTIzNDU2Nzg=private-trailer"),
        ),
        ("MAX_BODY_BYTES", Some("0")),
        ("BIND_ADDR", Some("private-invalid-bind")),
        ("COOKIE_SECURE", Some("not-a-boolean")),
        ("SESSION_SECRET", Some("QQ==")),
        ("DATABASE_URL", Some("postgres://private-host")),
        ("APP_ENV", Some("unknown-private-environment")),
    ] {
        let mut cmd = command(dir.path());
        match value {
            Some(value) => {
                cmd.env(key, value);
            }
            None => {
                cmd.env_remove(key);
            }
        }
        let output = cmd.output().unwrap();
        assert!(!output.status.success(), "accepted invalid {key}");
        let logs = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(logs.contains(key), "safe configuration key missing: {logs}");
        if let Some(value) = value.filter(|v| v.len() > 10) {
            assert!(!logs.contains(value));
        }
    }
}

struct Process(std::process::Child);

#[test]
fn invalid_argon2_versions_and_short_salts_fail_before_runtime_initialization() {
    let dir = tempfile::tempdir().unwrap();
    let valid = hash_secret(&generate_edit_code()).unwrap();
    let short_salt = valid
        .split('$')
        .enumerate()
        .map(|(index, part)| if index == 4 { "YWJjZA" } else { part })
        .collect::<Vec<_>>()
        .join("$");
    for invalid in [valid.replace("v=19", "v=999"), short_salt] {
        let output = command(dir.path())
            .env("ADMIN_PASSWORD_HASH", &invalid)
            .env("UPLOAD_DIR", "/dev/null/unavailable")
            .output()
            .unwrap();
        assert!(!output.status.success());
        let logs = String::from_utf8(output.stdout).unwrap();
        assert!(
            logs.contains("configuration_invalid"),
            "invalid Argon2 PHC reached runtime initialization: {logs}"
        );
        assert!(logs.contains("ADMIN_PASSWORD_HASH"));
        assert!(!logs.contains("upload_directory"));
        assert!(!logs.contains(&invalid));
    }
}

#[test]
fn supported_argon2_versions_and_minimum_salt_pass_configuration() {
    use argon2::{Algorithm, Argon2, Params, PasswordHasher, Version};
    let dir = tempfile::tempdir().unwrap();
    let salt = password_hash::SaltString::encode_b64(b"salt1234").unwrap();
    for version in [Version::V0x10, Version::V0x13] {
        let hash = Argon2::new(Algorithm::Argon2id, version, Params::default())
            .hash_password(generate_edit_code().as_bytes(), &salt)
            .unwrap()
            .to_string();
        let output = command(dir.path())
            .env("ADMIN_PASSWORD_HASH", hash)
            .env("UPLOAD_DIR", "/dev/null/unavailable")
            .output()
            .unwrap();
        let logs = String::from_utf8(output.stdout).unwrap();
        assert!(
            logs.contains("upload_directory"),
            "supported PHC was rejected: {logs}"
        );
        assert!(!logs.contains("configuration_invalid"));
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn hash_command_reads_stdin_and_requires_no_configuration() {
    let dir = tempfile::tempdir().unwrap();
    let secret = generate_edit_code();
    let mut child = Command::new(env!("CARGO_BIN_EXE_zongce-web"))
        .arg("--hash-secret")
        .current_dir(dir.path())
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    writeln!(child.stdin.take().unwrap(), "{secret}").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let hash = String::from_utf8(output.stdout).unwrap();
    assert!(hash.starts_with("$argon2id$"));
    assert!(zongce_web::auth::verify_secret(hash.trim(), &secret));
    assert!(!String::from_utf8_lossy(&output.stderr).contains(&secret));
}

#[test]
fn production_entry_point_serves_then_shuts_down_on_sigterm() {
    let dir = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let mut child = Process(
        command(dir.path())
            .env("BIND_ADDR", address.to_string())
            .env("RUST_LOG", "trace")
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut stream = loop {
        if let Ok(stream) = TcpStream::connect(address) {
            break stream;
        }
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "application exited instead of serving"
        );
        assert!(Instant::now() < deadline, "application never listened");
        std::thread::sleep(Duration::from_millis(20));
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream
        .write_all(b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.ends_with("ok"));
    assert!(dir.path().join("uploads").is_dir());
    assert!(
        Command::new("kill")
            .args(["-TERM", &child.0.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(Instant::now() < deadline, "graceful shutdown timed out");
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut logs = String::new();
    child
        .0
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut logs)
        .unwrap();
    for event in ["startup", "migration", "request_complete", "shutdown"] {
        assert!(logs.contains(event), "{logs}");
    }
    let database_event = logs
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|event| event["fields"]["event"] == "database_ready")
        .expect("startup must report the engine queried through the application SQLx pool");
    let version = database_event["fields"]["sqlite_version"].as_str().unwrap();
    let actual_version = tokio::runtime::Runtime::new().unwrap().block_on(async {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let version: String = sqlx::query_scalar("SELECT sqlite_version()")
            .fetch_one(&pool)
            .await
            .unwrap();
        pool.close().await;
        version
    });
    assert_eq!(
        version, actual_version,
        "startup must identify the actual linked engine"
    );
    let parts: Vec<_> = version.split('.').collect();
    assert_eq!(parts.len(), 3);
    assert!(parts.iter().all(|part| part.parse::<u32>().is_ok()));
    for forbidden in [
        "SELECT",
        "CREATE TABLE",
        "sqlx",
        "tower_sessions",
        dir.path().to_str().unwrap(),
    ] {
        assert!(!logs.contains(forbidden), "{logs}");
    }
}
