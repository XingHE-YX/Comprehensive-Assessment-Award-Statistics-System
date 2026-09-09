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

- Optional select controls submit an empty string, unlike unchecked checkboxes. Normalize empty category values to absence before shared validation; required validators still reject missing fields.
- Browser file inputs submit an empty filename/empty body when no file is selected. Ignore only that empty placeholder so updates can preserve existing attachments without requiring another upload.
- Checking edit permission before parsing is insufficient when administrators can review concurrently. Condition the database write on editable status and acquire the write lock before counting or saving added attachments.
- An anchor click does not fire hashchange when the fragment is unchanged. Disclosure-opening actions also need a click handler so a manually collapsed editor can reopen.

## 2026-09-09 — Task 7

- Build failure E0277: axum-test response bytes use `Bytes`, which cannot be directly compared with a fixed-size byte array in this assertion. Compare byte slices when checking exact protected attachment content.
- Axum Path/Form/Query extraction can reject requests before handlers with English text and Rust type information. Capture route-local extractor rejections and map them to AppError; test malformed inputs as well as valid forms, with authorization checked before extraction.
- Rust floating-point iterator `sum()` uses negative zero as the empty identity. For a displayed non-negative score total, fold from positive `0.0` and cover no-approved-record/empty-dashboard cases so the UI shows `0.00`.
- A Playwright URL glob such as `**?saved=1` does not match across slash-separated paths. A browser regression timed out after a successful 303 because of that matcher; use a URL predicate for pathname and search parameters.
- Specification conflict resolved using AGENT.md priority: APP_FLOW.md requires invalid dashboard filters to be ignored with a visible notice, so BACKEND_STRUCTURE.md's old 400 rule was corrected. Literal keyword searches must also escape `%`, `_` and backslash in bound LIKE patterns.

- `docker compose config` could not execute because Docker is absent on this development machine. Keep that result separate from successful application checks; Compose syntax and deployment remain unverified until the runtime and Task 10 files exist.

## How to add a lesson

Record the date, the observed problem or decision, and the rule that should guide future work. Do not store secrets, personal data, upload contents, or temporary guesses.
