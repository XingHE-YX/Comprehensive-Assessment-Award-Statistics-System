mod admin;
mod edit_codes;
mod password;
pub use admin::AdminCredentials;
pub use edit_codes::EditCodeVault;

use std::convert::TryFrom;

use chrono::{Duration, Utc};
use sqlx::SqlitePool;
use subtle::ConstantTimeEq;
use tower_sessions::{
    Expiry, MemoryStore, Session, SessionManagerLayer,
    cookie::{Key, SameSite},
    service::PrivateCookie,
};

use crate::{db::SubmissionRepo, domain::Submission};

pub use password::{
    EDIT_CODE_ALPHABET, generate_edit_code, generate_submission_no, hash_secret, verify_secret,
};

pub const ADMIN_AUTHENTICATED_KEY: &str = "admin_authenticated";
pub const ADMIN_AUTHENTICATED_AT_KEY: &str = "admin_authenticated_at";
pub const STUDENT_YEAR_ID_KEY: &str = "student_year_id";
pub const STUDENT_EXPIRES_AT_KEY: &str = "student_expires_at";
pub const RECEIPT_SUBMISSION_NO_KEY: &str = "receipt_submission_no";
pub const RECEIPT_EDIT_CODE_KEY: &str = "receipt_edit_code";
pub const RECEIPT_EXPIRES_AT_KEY: &str = "receipt_expires_at";
pub const VERIFIED_SUBMISSION_ID_KEY: &str = "verified_submission_id";
pub const VERIFIED_EXPIRES_AT_KEY: &str = "verified_expires_at";
pub const CSRF_SESSION_KEY: &str = "csrf_token";
const VERIFIED_SCOPE_KEY: &str = "verified_submission_scope";
const RECEIPT_SCOPE_KEY: &str = "receipt_scope";

#[derive(serde::Serialize, serde::Deserialize)]
struct VerifiedScope {
    submission_id: i64,
    version: i64,
}

// Plaintext credentials must never acquire Debug output.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ReceiptScope {
    pub submission_no: String,
    pub edit_code: String,
    pub version: i64,
    expires_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminUser {
    pub authenticated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("会话操作失败")]
    Session(#[from] tower_sessions::session::Error),
    #[error("数据库操作失败")]
    Database(#[from] sqlx::Error),
    #[error("凭据无效")]
    InvalidCredentials,
    #[error("未找到有效会话")]
    MissingSession,
    #[error("会话密钥无效")]
    InvalidSessionSecret,
    #[error("凭据验证暂时不可用")]
    Verification,
}

pub type AuthResult<T> = Result<T, AuthError>;

pub async fn establish_admin_session(session: &Session) -> AuthResult<()> {
    session.cycle_id().await?;
    session.remove::<String>(CSRF_SESSION_KEY).await?;
    session.insert(ADMIN_AUTHENTICATED_KEY, true).await?;
    session
        .insert(ADMIN_AUTHENTICATED_AT_KEY, Utc::now())
        .await?;
    Ok(())
}

pub async fn require_admin(session: &Session) -> AuthResult<AdminUser> {
    let authenticated = session
        .get::<bool>(ADMIN_AUTHENTICATED_KEY)
        .await?
        .unwrap_or(false);
    if !authenticated {
        return Err(AuthError::MissingSession);
    }
    let authenticated_at = session
        .get::<chrono::DateTime<Utc>>(ADMIN_AUTHENTICATED_AT_KEY)
        .await?
        .ok_or(AuthError::MissingSession)?;
    Ok(AdminUser { authenticated_at })
}

pub async fn establish_student_session(session: &Session, academic_year_id: i64) -> AuthResult<()> {
    session
        .insert(STUDENT_YEAR_ID_KEY, academic_year_id)
        .await?;
    session
        .insert(STUDENT_EXPIRES_AT_KEY, Utc::now() + Duration::hours(2))
        .await?;
    Ok(())
}

pub async fn student_year_id(session: &Session) -> AuthResult<Option<i64>> {
    if !session_is_valid(session, STUDENT_EXPIRES_AT_KEY).await? {
        return Ok(None);
    }
    Ok(session.get(STUDENT_YEAR_ID_KEY).await?)
}

pub async fn establish_receipt_session(
    session: &Session,
    submission_no: &str,
    edit_code: &str,
) -> AuthResult<()> {
    session
        .insert(
            RECEIPT_SCOPE_KEY,
            ReceiptScope {
                submission_no: submission_no.to_owned(),
                edit_code: edit_code.to_owned(),
                version: 0,
                expires_at: Utc::now() + Duration::minutes(15),
            },
        )
        .await?;
    Ok(())
}

pub async fn receipt_edit_code(session: &Session) -> AuthResult<Option<String>> {
    Ok(session
        .get::<ReceiptScope>(RECEIPT_SCOPE_KEY)
        .await?
        .filter(|receipt| receipt.expires_at > Utc::now())
        .map(|receipt| receipt.edit_code))
}

pub async fn receipt_submission_no(session: &Session) -> AuthResult<Option<String>> {
    Ok(session
        .get::<ReceiptScope>(RECEIPT_SCOPE_KEY)
        .await?
        .filter(|receipt| receipt.expires_at > Utc::now())
        .map(|receipt| receipt.submission_no))
}

pub async fn take_receipt(session: &Session) -> AuthResult<Option<ReceiptScope>> {
    Ok(session
        .remove::<ReceiptScope>(RECEIPT_SCOPE_KEY)
        .await?
        .filter(|receipt| receipt.expires_at > Utc::now()))
}

pub async fn establish_verified_student_session(
    session: &Session,
    submission_id: i64,
) -> AuthResult<()> {
    establish_verified_student_session_at_version(session, submission_id, 0).await
}

pub async fn establish_verified_student_session_at_version(
    session: &Session,
    submission_id: i64,
    version: i64,
) -> AuthResult<()> {
    session
        .insert(
            VERIFIED_SCOPE_KEY,
            VerifiedScope {
                submission_id,
                version,
            },
        )
        .await?;
    session
        .insert(VERIFIED_EXPIRES_AT_KEY, Utc::now() + Duration::minutes(30))
        .await?;
    Ok(())
}

pub async fn verify_student_session(session: &Session, submission_id: i64) -> AuthResult<()> {
    verify_student_session_at_version(session, submission_id, 0).await
}

pub async fn verify_student_session_at_version(
    session: &Session,
    submission_id: i64,
    version: i64,
) -> AuthResult<()> {
    if !session_is_valid(session, VERIFIED_EXPIRES_AT_KEY).await? {
        return Err(AuthError::MissingSession);
    }
    // Old in-memory scopes are valid only at version zero; a reset revokes them.
    let current = match session.get::<VerifiedScope>(VERIFIED_SCOPE_KEY).await? {
        Some(scope) => scope,
        None => VerifiedScope {
            submission_id: session
                .get::<i64>(VERIFIED_SUBMISSION_ID_KEY)
                .await?
                .ok_or(AuthError::MissingSession)?,
            version: 0,
        },
    };
    if current.submission_id != submission_id || current.version != version {
        return Err(AuthError::MissingSession);
    }
    Ok(())
}

async fn session_is_valid(session: &Session, expiry_key: &str) -> AuthResult<bool> {
    Ok(session
        .get::<chrono::DateTime<Utc>>(expiry_key)
        .await?
        .is_some_and(|expires_at| expires_at > Utc::now()))
}

pub async fn verify_student_access(
    pool: &SqlitePool,
    submission_no: &str,
    edit_code: &str,
) -> AuthResult<Submission> {
    let submission = SubmissionRepo::find_by_no(pool, submission_no)
        .await?
        .ok_or(AuthError::InvalidCredentials)?;
    if !verify_secret(&submission.edit_code_hash, edit_code) {
        return Err(AuthError::InvalidCredentials);
    }
    Ok(submission)
}

pub async fn generate_csrf_token(session: &Session) -> AuthResult<String> {
    if let Some(token) = session.get::<String>(CSRF_SESSION_KEY).await? {
        return Ok(token);
    }
    let token = uuid::Uuid::new_v4().simple().to_string();
    session.insert(CSRF_SESSION_KEY, &token).await?;
    Ok(token)
}

pub async fn verify_csrf_token(session: &Session, submitted: &str) -> AuthResult<bool> {
    let Some(expected) = session.get::<String>(CSRF_SESSION_KEY).await? else {
        return Ok(false);
    };
    Ok(expected.as_bytes().ct_eq(submitted.as_bytes()).into())
}

pub fn session_layer(
    secret: &[u8],
    secure: bool,
) -> AuthResult<SessionManagerLayer<MemoryStore, PrivateCookie>> {
    let mut key_material = Vec::with_capacity(64);
    if secret.len() >= 64 {
        key_material.extend_from_slice(&secret[..64]);
    } else if secret.len() >= 32 {
        key_material.extend_from_slice(secret);
        key_material.extend_from_slice(secret);
    } else {
        return Err(AuthError::InvalidSessionSecret);
    }
    let key =
        Key::try_from(key_material.as_slice()).map_err(|_| AuthError::InvalidSessionSecret)?;
    Ok(SessionManagerLayer::new(MemoryStore::default())
        .with_name("zongce.sid")
        .with_http_only(true)
        .with_same_site(SameSite::Lax)
        .with_secure(secure)
        .with_expiry(Expiry::OnSessionEnd)
        .with_private(key))
}
