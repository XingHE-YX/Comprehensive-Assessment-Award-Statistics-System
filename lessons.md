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
- Matching an error enum by value and then formatting the original error can trigger a partial-move compile error; borrow guarded payloads with `ref` when the value is still needed afterward.

## 2026-09-08

- Keep category JSON keys in one public constants module and consume those constants from the validators; this prevents schema drift and keeps strict Clippy checks clean before route and export layers use the keys.
- Conditional category fields must be validated from the selected discriminator value, while unrelated category fields remain ignored so the shared submission payload can safely carry browser form data.

## 2026-09-09

- Axum 0.8 route captures use `{name}` syntax; the older `:name` form panics during router construction.
- Integration tests using `axum-test` must enable `save_cookies()` when the behavior spans CSRF and session-backed redirects.
- A successful receipt should clear its short-lived session values after rendering so the edit code is not replayable through repeated receipt requests.
- The `/access` route contract uses `400` for an incorrect class code (while credential-protected query/admin routes use `401`); keep route-specific status behavior aligned with `BACKEND_STRUCTURE.md`.
- `tower_sessions::Session::remove` cannot infer its deserialization type from a discarded result; use explicit types such as `remove::<String>(...)` when clearing typed session values.

## How to add a lesson

Record the date, the observed problem or decision, and the rule that should guide future work. Do not store secrets, personal data, upload contents, or temporary guesses.
