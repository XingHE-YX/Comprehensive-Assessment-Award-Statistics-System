use std::{env, net::SocketAddr, path::PathBuf};

// Configuration contains secrets; deliberately do not derive Debug.
#[derive(Clone)]
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
        let password_hash = password_hash::PasswordHash::new(&admin_password_hash)
            .map_err(|_| ConfigError::Invalid("ADMIN_PASSWORD_HASH"))?;
        // PHC syntax/Params parsing alone does not validate the algorithm's
        // version or decoded salt. Match the checks used by Argon2 verification.
        let mut salt_buffer = [0_u8; 64];
        let valid_salt = password_hash
            .salt
            .and_then(|salt| salt.decode_b64(&mut salt_buffer).ok())
            .is_some_and(|salt| salt.len() >= argon2::MIN_SALT_LEN);
        if password_hash.algorithm.as_str() != "argon2id"
            || !valid_salt
            || password_hash.hash.is_none()
            || argon2::Params::try_from(&password_hash).is_err()
            || password_hash
                .version
                .map(argon2::Version::try_from)
                .transpose()
                .is_err()
        {
            return Err(ConfigError::Invalid("ADMIN_PASSWORD_HASH"));
        }
        let session_secret = decode_base64(&required("SESSION_SECRET")?)
            .ok_or(ConfigError::Invalid("SESSION_SECRET"))?;
        if session_secret.len() < 32 {
            return Err(ConfigError::Invalid("SESSION_SECRET"));
        }
        let database_url = required("DATABASE_URL")?;
        if !database_url.starts_with("sqlite:") {
            return Err(ConfigError::Invalid("DATABASE_URL"));
        }
        let upload_dir = PathBuf::from(required("UPLOAD_DIR")?);
        let cookie_secure = env::var("COOKIE_SECURE")
            .map(|value| parse_bool(&value).ok_or(ConfigError::Invalid("COOKIE_SECURE")))
            .unwrap_or(Ok(app_env == "production"))?;
        if app_env == "production" && !cookie_secure {
            return Err(ConfigError::Invalid("COOKIE_SECURE"));
        }
        let max_body_bytes = env::var("MAX_BODY_BYTES")
            .map(|value| {
                value
                    .parse()
                    .map_err(|_| ConfigError::Invalid("MAX_BODY_BYTES"))
            })
            .unwrap_or(Ok(115_343_360))?;
        if max_body_bytes == 0 {
            return Err(ConfigError::Invalid("MAX_BODY_BYTES"));
        }
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
    // Strict RFC 4648 standard alphabet/padding: never silently ignore a suffix.
    if value.is_empty() || value.len() % 4 != 0 {
        return None;
    }
    let unpadded = value.trim_end_matches('=');
    let padding = value.len() - unpadded.len();
    if padding > 2 || unpadded.len() % 4 == 1 {
        return None;
    }
    let mut output = Vec::with_capacity(value.len() * 3 / 4);
    let mut accumulator = 0_u32;
    let mut bits = 0_u8;
    for byte in unpadded.bytes() {
        let digit = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
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
    if accumulator != 0 {
        return None;
    }
    Some(output)
}
