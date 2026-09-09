use chrono::Datelike;
use password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng};
use rand::{Rng, rng};

use crate::domain::AcademicYear;

pub const EDIT_CODE_ALPHABET: &str = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

#[derive(Debug, thiserror::Error)]
pub enum PasswordError {
    #[error("secret hash failed: {0}")]
    Hash(String),
}

pub fn hash_secret(plain: &str) -> Result<String, PasswordError> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(argon2::Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .map_err(|error| PasswordError::Hash(error.to_string()))?
        .to_string())
}

pub fn verify_secret(hash: &str, plain: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    argon2::Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok()
}

pub fn generate_edit_code() -> String {
    let alphabet = EDIT_CODE_ALPHABET.as_bytes();
    let mut random = rng();
    (0..10)
        .map(|_| alphabet[random.random_range(0..alphabet.len())] as char)
        .collect()
}

pub fn generate_submission_no(year: &AcademicYear, sequence: u64) -> String {
    format!("ZC{:04}-{sequence:06}", year.end_date.year())
}
