use std::{env, net::SocketAddr, path::PathBuf};

#[derive(Debug, Clone)]
pub struct Config {
    pub app_env: String,
    pub bind_addr: SocketAddr,
    pub admin_username: String,
    pub admin_password_hash: String,
    pub session_secret: Vec<u8>,
    pub database_url: String,
    pub upload_dir: PathBuf,
    pub cookie_secure: bool,
    pub max_body_bytes: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("缺少必需环境变量：{0}")]
    Missing(&'static str),
    #[error("环境变量无效：{0}")]
    Invalid(&'static str),
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let app_env = required("APP_ENV")?;
        if !matches!(app_env.as_str(), "development" | "production") {
            return Err(ConfigError::Invalid("APP_ENV"));
        }
        let bind_addr = required("BIND_ADDR")?
            .parse()
            .map_err(|_| ConfigError::Invalid("BIND_ADDR"))?;
        let admin_username = required("ADMIN_USERNAME")?;
        let admin_password_hash = required("ADMIN_PASSWORD_HASH")?;
        let session_secret = decode_base64(&required("SESSION_SECRET")?)
            .ok_or(ConfigError::Invalid("SESSION_SECRET"))?;
        if session_secret.len() < 32 {
            return Err(ConfigError::Invalid("SESSION_SECRET"));
        }
        let database_url = required("DATABASE_URL")?;
        let upload_dir = PathBuf::from(required("UPLOAD_DIR")?);
        let cookie_secure = env::var("COOKIE_SECURE")
            .map(|value| parse_bool(&value).ok_or(ConfigError::Invalid("COOKIE_SECURE")))
            .unwrap_or(Ok(app_env == "production"))?;
        let max_body_bytes = env::var("MAX_BODY_BYTES")
            .map(|value| {
                value
                    .parse()
                    .map_err(|_| ConfigError::Invalid("MAX_BODY_BYTES"))
            })
            .unwrap_or(Ok(115_343_360))?;
        Ok(Self {
            app_env,
            bind_addr,
            admin_username,
            admin_password_hash,
            session_secret,
            database_url,
            upload_dir,
            cookie_secure,
            max_body_bytes,
        })
    }
}

fn required(name: &'static str) -> Result<String, ConfigError> {
    env::var(name)
        .map_err(|_| ConfigError::Missing(name))
        .and_then(|value| {
            if value.trim().is_empty() {
                Err(ConfigError::Invalid(name))
            } else {
                Ok(value)
            }
        })
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "1" | "true" | "TRUE" | "yes" => Some(true),
        "0" | "false" | "FALSE" | "no" => Some(false),
        _ => None,
    }
}

fn decode_base64(value: &str) -> Option<Vec<u8>> {
    let mut output = Vec::with_capacity(value.len() * 3 / 4);
    let mut accumulator = 0_u32;
    let mut bits = 0_u8;
    for byte in value.bytes() {
        if byte == b'=' {
            break;
        }
        let digit = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        };
        accumulator = (accumulator << 6) | u32::from(digit);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((accumulator >> bits) as u8);
            accumulator &= (1 << bits) - 1;
        }
    }
    Some(output)
}
