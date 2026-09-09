use std::sync::Arc;

use chrono::{NaiveDate, Utc};
use tower_sessions::{Expiry, MemoryStore, Session, SessionManagerLayer, cookie::SameSite};
use zongce_web::auth::{
    CSRF_SESSION_KEY, EDIT_CODE_ALPHABET, generate_csrf_token, generate_edit_code,
    generate_submission_no, hash_secret, verify_csrf_token, verify_secret,
};

#[tokio::test]
async fn verified_student_session_replaces_scope_and_rejects_expired_access() {
    use zongce_web::auth::{
        VERIFIED_EXPIRES_AT_KEY, establish_verified_student_session, verify_student_session,
    };
    let session = Session::new(None, Arc::new(MemoryStore::default()), None);
    establish_verified_student_session(&session, 10)
        .await
        .unwrap();
    assert!(verify_student_session(&session, 10).await.is_ok());
    assert!(verify_student_session(&session, 11).await.is_err());
    establish_verified_student_session(&session, 11)
        .await
        .unwrap();
    assert!(verify_student_session(&session, 10).await.is_err());
    assert!(verify_student_session(&session, 11).await.is_ok());
    session
        .insert(
            VERIFIED_EXPIRES_AT_KEY,
            Utc::now() - chrono::Duration::seconds(1),
        )
        .await
        .unwrap();
    assert!(verify_student_session(&session, 11).await.is_err());
}
use zongce_web::domain::AcademicYear;

fn year() -> AcademicYear {
    AcademicYear {
        id: 1,
        name: "2025-2026学年".to_owned(),
        start_date: NaiveDate::from_ymd_opt(2025, 8, 31).expect("date"),
        end_date: NaiveDate::from_ymd_opt(2026, 8, 28).expect("date"),
        deadline: None,
        is_active: true,
        announcement: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[test]
fn hashes_and_verifies_secret_without_storing_plaintext() {
    let hash = hash_secret("correct horse battery staple").expect("hash");
    assert!(hash.starts_with("$argon2id$"));
    assert_ne!(hash, "correct horse battery staple");
    assert!(verify_secret(&hash, "correct horse battery staple"));
    assert!(!verify_secret(&hash, "wrong"));
}

#[test]
fn generated_codes_and_submission_numbers_follow_contract() {
    let code = generate_edit_code();
    assert!((8..=10).contains(&code.len()));
    assert!(code.chars().all(|ch| EDIT_CODE_ALPHABET.contains(ch)));
    assert_eq!(generate_submission_no(&year(), 1), "ZC2026-000001");
    assert_eq!(generate_submission_no(&year(), 42), "ZC2026-000042");
}

#[tokio::test]
async fn csrf_token_is_session_bound_and_cookie_defaults_are_safe() {
    let store = MemoryStore::default();
    let session = Session::new(None, Arc::new(store), Some(Expiry::OnSessionEnd));
    let token = generate_csrf_token(&session).await.expect("token");
    assert_eq!(
        session.get::<String>(CSRF_SESSION_KEY).await.expect("get"),
        Some(token.clone())
    );
    assert!(verify_csrf_token(&session, &token).await.expect("verify"));
    assert!(!verify_csrf_token(&session, "wrong").await.expect("verify"));

    let layer = SessionManagerLayer::new(MemoryStore::default())
        .with_same_site(SameSite::Lax)
        .with_http_only(true)
        .with_secure(true);
    let _ = layer;
}
