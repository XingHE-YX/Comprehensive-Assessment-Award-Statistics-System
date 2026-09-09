use std::sync::Arc;

use subtle::ConstantTimeEq;

use super::{AuthError, AuthResult, verify_secret};

// Intentionally omit Debug: credentials must not be included in logs.
pub struct AdminCredentials {
    username: String,
    password_hash: String,
}

impl AdminCredentials {
    pub fn new(username: String, password_hash: String) -> Self {
        Self {
            username,
            password_hash,
        }
    }

    pub async fn verify(self: Arc<Self>, username: String, password: String) -> AuthResult<bool> {
        tokio::task::spawn_blocking(move || {
            let username_matches: bool = self.username.as_bytes().ct_eq(username.as_bytes()).into();
            // Always verify the hash, including for an unknown username.
            let password_matches = verify_secret(&self.password_hash, &password);
            username_matches & password_matches
        })
        .await
        .map_err(|_| AuthError::Verification)
    }
}
