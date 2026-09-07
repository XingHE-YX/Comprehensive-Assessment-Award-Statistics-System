use std::{env, net::SocketAddr, path::PathBuf};

use password_hash::PasswordHash;
use thiserror::Error;

pub const DEFAULT_MAX_BODY_BYTES: usize = 115_343_360;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppEnvironment {
    Development,
    Production,
}

#[derive(Clone)]
pub struct Config {
    pub environment: AppEnvironment,
    pub bind_addr: SocketAddr,
    pub admin_username: String,
    pub admin_password_hash: String,
    pub session_secret: String,
    pub database_url: String,
    pub upload_dir: PathBuf,
    pub cookie_secure: bool,
    pub max_body_bytes: usize,
    pub rust_log: String,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("缺少必需的环境变量：{0}")]
    Missing(&'static str),
    #[error("环境变量 {name} 配置无效：{reason}")]
    Invalid {
        name: &'static str,
        reason: &'static str,
    },
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        dotenvy::dotenv().ok();

        let environment = match required("APP_ENV")?.as_str() {
            "development" => AppEnvironment::Development,
            "production" => AppEnvironment::Production,
            _ => {
                return Err(ConfigError::Invalid {
                    name: "APP_ENV",
                    reason: "只能是 development 或 production",
                });
            }
        };
        let bind_addr = required("BIND_ADDR")?
            .parse()
            .map_err(|_| ConfigError::Invalid {
                name: "BIND_ADDR",
                reason: "必须是 IP 地址和端口",
            })?;
        let admin_username = required("ADMIN_USERNAME")?;
        let admin_password_hash = required("ADMIN_PASSWORD_HASH")?;
        validate_argon2id_hash(&admin_password_hash)?;
        let session_secret = required("SESSION_SECRET")?;
        validate_session_secret(&session_secret)?;
        let database_url = required("DATABASE_URL")?;
        if !database_url.starts_with("sqlite:") {
            return Err(ConfigError::Invalid {
                name: "DATABASE_URL",
                reason: "必须使用 sqlite: URL",
            });
        }
        let upload_dir = PathBuf::from(required("UPLOAD_DIR")?);
        let cookie_secure = optional_bool("COOKIE_SECURE")?
            .unwrap_or(matches!(environment, AppEnvironment::Production));
        let max_body_bytes = optional_usize("MAX_BODY_BYTES")?.unwrap_or(DEFAULT_MAX_BODY_BYTES);
        let rust_log = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_owned());

        Ok(Self {
            environment,
            bind_addr,
            admin_username,
            admin_password_hash,
            session_secret,
            database_url,
            upload_dir,
            cookie_secure,
            max_body_bytes,
            rust_log,
        })
    }
}

fn required(name: &'static str) -> Result<String, ConfigError> {
    let value = env::var(name).map_err(|_| ConfigError::Missing(name))?;
    let value = value.trim();
    if value.is_empty() {
        return Err(ConfigError::Missing(name));
    }
    Ok(value.to_owned())
}

fn optional_bool(name: &'static str) -> Result<Option<bool>, ConfigError> {
    match env::var(name) {
        Ok(value) => match value.trim() {
            "true" => Ok(Some(true)),
            "false" => Ok(Some(false)),
            _ => Err(ConfigError::Invalid {
                name,
                reason: "必须是 true 或 false",
            }),
        },
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(ConfigError::Invalid {
            name,
            reason: "必须是 UTF-8 文本",
        }),
    }
}

fn optional_usize(name: &'static str) -> Result<Option<usize>, ConfigError> {
    match env::var(name) {
        Ok(value) => {
            let value = value
                .trim()
                .parse::<usize>()
                .map_err(|_| ConfigError::Invalid {
                    name,
                    reason: "必须是正整数",
                })?;
            if value == 0 {
                return Err(ConfigError::Invalid {
                    name,
                    reason: "必须大于 0",
                });
            }
            Ok(Some(value))
        }
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(ConfigError::Invalid {
            name,
            reason: "必须是 UTF-8 文本",
        }),
    }
}

fn validate_argon2id_hash(value: &str) -> Result<(), ConfigError> {
    let hash = PasswordHash::new(value).map_err(|_| ConfigError::Invalid {
        name: "ADMIN_PASSWORD_HASH",
        reason: "必须是 Argon2id PHC 哈希",
    })?;
    if hash.algorithm.as_str() != "argon2id" {
        return Err(ConfigError::Invalid {
            name: "ADMIN_PASSWORD_HASH",
            reason: "必须是 Argon2id PHC 哈希",
        });
    }
    Ok(())
}

fn validate_session_secret(value: &str) -> Result<(), ConfigError> {
    let unpadded = value.trim_end_matches('=');
    let padding = value.len().saturating_sub(unpadded.len());
    let valid_character = |byte: u8| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/');

    if value.len() % 4 != 0
        || padding > 2
        || unpadded.is_empty()
        || !unpadded.bytes().all(valid_character)
    {
        return Err(ConfigError::Invalid {
            name: "SESSION_SECRET",
            reason: "必须是有效的 Base64 文本",
        });
    }

    let decoded_length = (value.len() / 4) * 3 - padding;
    if decoded_length < 32 {
        return Err(ConfigError::Invalid {
            name: "SESSION_SECRET",
            reason: "解码后至少需要 32 字节",
        });
    }

    Ok(())
}
