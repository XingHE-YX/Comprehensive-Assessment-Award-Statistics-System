# Lessons Learned

This file stores verified, reusable engineering lessons for the project. Add only concrete lessons that affect future implementation decisions.

## 2026-09-07

- The repository currently contains specifications only; implementation must begin from a Rust/Axum skeleton.
- The frontend is intentionally server-rendered and minimal. Do not introduce a JavaScript build chain for questionnaire interactions.
- Category-specific data is stored as JSON for development speed, but every export path must expand it into named Excel columns.
- Student edit access is credential-based rather than account-based. Hash the edit code and scope the verified session to one internal submission id.
- The active academic year is a database invariant: activation must close other active years in the same transaction.
- SQLx's `migrate!` macro requires enabling the `macros` feature when `default-features = false`; `SqlitePoolOptions` is exported from `sqlx::sqlite` in SQLx 0.8.
- The password-hash crate error does not implement `std::error::Error` in a way compatible with thiserror's `#[from]`; convert it to a message at the boundary instead of deriving an automatic source conversion.
- Stored attachment names must be validated as generated UUID-hex names with an allowed extension before any fallback disk scan; rejecting only path separators is insufficient for protected file access.
- A delegated implementation attempt can fail before producing code when the model service is unavailable (HTTP 503); record the failure and continue with local verification rather than treating it as a code defect.

## How to add a lesson

Record the date, the observed problem or decision, and the rule that should guide future work. Do not store secrets, personal data, upload contents, or temporary guesses.
