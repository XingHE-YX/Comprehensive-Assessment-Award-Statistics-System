use tower_sessions::cookie::{Cookie, CookieJar, Key};

use super::{AuthError, AuthResult};

/// Server-only authenticated envelopes. Never sent as browser cookies or logged.
/// Deliberately has no Debug implementation.
pub struct EditCodeVault {
    key: Key,
}

impl EditCodeVault {
    pub fn new(secret: &[u8]) -> AuthResult<Self> {
        if secret.len() < 32 {
            return Err(AuthError::InvalidSessionSecret);
        }
        Ok(Self {
            key: Key::derive_from(secret),
        })
    }

    pub fn encrypt(&self, submission_no: &str, code: &str) -> String {
        let name = Self::name(submission_no);
        let mut jar = CookieJar::new();
        jar.private_mut(&self.key)
            .add(Cookie::new(name.clone(), code.to_owned()));
        // PrivateJar::add always adds the supplied cookie under the same name.
        let encrypted = jar
            .get(&name)
            .expect("private jar contains the added envelope");
        format!("v1:{}", encrypted.value())
    }

    pub fn decrypt(&self, submission_no: &str, ciphertext: &str) -> Option<String> {
        let value = ciphertext.strip_prefix("v1:")?;
        CookieJar::new()
            .private(&self.key)
            .decrypt(Cookie::new(Self::name(submission_no), value.to_owned()))
            .map(|cookie| cookie.value().to_owned())
    }

    fn name(submission_no: &str) -> String {
        format!("zongce.edit-code.v1.{submission_no}")
    }
}
