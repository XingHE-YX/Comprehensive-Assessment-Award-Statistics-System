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

## 2026-09-10 — Task 8

- Deadline regression: the no-material declaration branch returned 303 after the admin set a past cutoff, while result submission correctly returned 422. Apply the shared deadline validation to both new-submission paths; existing editable submissions remain exempt as specified by PRD.
- Numbering regression: two academic-year records ending in the same calendar year each started at sequence 1, causing a unique-number database failure (500). Allocate under the write lock by the public four-digit year prefix across all records, not by the internal academic-year id.
- Editable display names are unsafe storage keys. A valid name such as a slash-separated academic year passed settings validation but caused attachment uploads to return 400. Use immutable `year-<id>` directories for new files; preserve protected cold lookup of legacy directories so renaming never loses access to historical files.
- Rechecking settings only before parsing an upload allows a concurrent activation/deadline/date edit to be bypassed. After multipart parsing, acquire the write lock and reload the active year before validation and persistence for both results and declarations. A real `Expect: 100-continue` request provides a deterministic regression boundary without sleeps.
- Build failure E0425 during the repository transaction refactor came from a broad replacement of `fetch_one(pool)` that also changed an unrelated count query to an out-of-scope `connection`. Restrict executor substitutions to the function being refactored and inspect the complete diff before building.

## 2026-09-10 — Task 9

- Build failure E0277 in workbook assertions: `&serde_json::Value` cannot be compared with an owned `String`; extract the cell's `as_str`/`as_f64` value first. Strict Clippy rejected the nested category fixture tuple as `type_complexity`; use a named test case alias. Run `cargo fmt` after editing assertions before the format gate.
- A proposed overlong `detail` test fixture failed SQLite's 4000-character CHECK before reaching XLSX generation. Use data actually accepted at the relevant boundary. Axum Query decoding also replaces invalid UTF-8 with a replacement character, so `%FF` does not itself cause an extractor rejection.
- Askama 0.14 escapes URL separators as decimal `&#38;`, not just `&amp;`. Passing the raw HTML attribute into an HTTP test turned the remaining filters into a URL fragment and falsely suggested a filtering bug. Decode the emitted entity and verify actual browser clicks.
- A real student multipart request can save category text beyond Excel's cell limit. Direct `write_string` then fails the entire export (32768 CJK characters reproduced through HTTP). At the XLSX boundary, cap text at 32767 UTF-16 units, reserve room for an explicit Chinese truncation notice and protected original-detail path, and keep database text complete. Unicode scalar counting alone misses emoji surrogate pairs.
- Validation accepts `award_detail` and `actual_award_rank` as legacy Other-award aliases, while the shared detail view previously displayed only `other_award`. An export's original-detail reference must expose the same complete value; normalize supported aliases before display/edit prefilling. Both aliases now have regression coverage.
- Year settings accept 0001-9999, while Excel's native 1900 date system cannot reliably display earlier dates. A configured 1800-1801 year and valid student submission reproduced export 500. Export pre-1900 obtained dates as readable ISO text and keep normal dates native.
- Multiplying each finite approved score by 100 can overflow (`1e308` reproduced), and rust_xlsxwriter writes infinity as text. Round the final total only when the scaling remains finite; explicitly reject non-finite numbers before writing so numeric columns do not silently change type.
- A multi-file documentation patch failed context verification because its task-section hunks were out of source order. Read the current task block, apply ordered contextual hunks and check the diff; never broadly replace repeated Step labels across tasks.
- LibreOffice headless emitted host Fontconfig cache-directory warnings but returned zero and re-saved all sample workbooks. Inspect the converted ZIP/XML and cell values before classifying tool warnings as workbook failures; the checked files preserved Chinese text, numeric types, filters and frozen panes.

## 2026-09-10 — Task 10

- Request-limit regression: stacking Tower's RequestBodyLimit around Axum's DefaultBodyLimit made a genuinely oversized multipart body surface as 400 because the nested stream error was no longer classified as 413. Keep the declared Content-Length precheck separate and let DefaultBodyLimit bound actual Form/Multipart reads; preserve FormRejection/MultipartError status when converting to AppError. Test both bodies without a length header and declared oversized uploads.
- A per-thread tracing subscriber in one concurrently executed integration test intermittently missed request span fields even though a single-test run passed. Use one capture subscriber for that integration-test process and random per-request sentinels; assert generated request ids and matched route templates while checking no sentinel, CSRF or raw query values reach output. Production filtering must also exclude dependency targets, not merely sanitize the application's own request span.
- Existing administrator/export tests asserted complete plain-text error bodies. Moving to the specified Chinese HTML pages intentionally changes those bytes; preserve their status/business assertions and upgrade the tests to assert HTML Content-Type, the safe public message and absence of internal values.
- Compose interpolation can treat `$` characters in PHC hashes as variables. Keep private values in an env_file with `format: raw` (Compose 2.35.1), without surrounding quotes; keep only public DOMAIN/image settings in the separate interpolation `.env`. `docker compose config --quiet` validates without printing credentials.
- SQLite WAL data is not necessarily in the main database file. A live WAL fixture reproduced why plain copying app.db is insufficient. The backup script pauses application writes, uses SQLite .backup, copies matching uploads, checks integrity and only then publishes/rotates complete snapshots. Its failure cleanup must restart a service that was running before the backup.

- Task 10 review reproduction: a generated Argon2id PHC modified from v=19 to v=999 passed PHC/Params parsing and reached storage initialization. Validate `argon2::Version` separately and decode the salt to enforce Argon2's eight-byte minimum; PHC's generic salt syntax has a weaker minimum. Tests keep both supported versions and minimum-length valid salts accepted.
- Task 10 backup review: Compose's running-only listing omits restarting containers even though the restart policy may launch a writer during copying. Inspect the actual container state, stop both running/restarting cases and verify stopped state before snapshotting; restore the original running intent. The regression models Docker's documented state transitions while retaining real SQLite and filesystem operations; real Docker acceptance remains a separate controller check.
- Fault injection reproduced private path leakage from touch/chmod/mktemp/mv/rmdir, and ignored lock-removal failure could report success while blocking all future backups. Suppress raw utility stderr across normal/trap paths, use a dedicated descriptor for controlled events, propagate partial/lock cleanup failures, and emit success only after cleanup completes. Tests inject failures after web is stopped and verify service recovery, nonzero exit and path-free stderr/logs.

- Task 10 environment checks: a fresh Colima image had a dangling resolver symlink, so image pulls failed with DNS connection-refused despite a running Docker daemon. Check the guest resolver against DHCP configuration, repair the isolated test environment and verify image pulls; distinguish this from application/container build failures. Skill helper scripts also lacked executable bits; invoke them through Bash with explicit output paths rather than changing shared plugin files.
- Parallel Playwright processes on this host printed completed cases but two did not exit. Re-running the same production-entry-point adapters sequentially exited successfully; require both assertions and normal exit before recording a pass. Keep backup/restart fault injection separate from browser requests: overlapping them reproduced a transient 502. Likewise, wait for service health after Compose start before testing restored data; an immediate request reproduced 502 while startup was still in progress.
- Python 3.9 on this macOS host did not load the isolated Caddy trust root through SSL_CERT_FILE; curl with the same explicit CA succeeded. Supply an explicit SSL context through SMOKE_CA_FILE for local HTTPS smoke tests, preserving certificate and hostname verification.

- Final Task 10 review found a production baseline mismatch: SQLx's locked libsqlite3-sys 0.30.1 bundles SQLite 3.46.0, despite the required 3.46.1 engine. Installing a distribution sqlite3 CLI does not change a statically bundled application's engine. Compile checksum-pinned official SQLite for production, preserve driver-needed flags (especially COLUMN_METADATA and UNLOCK_NOTIFY), use the driver's supported static-link controls, and gate the exact release binary on SQLx SELECT sqlite_version(). The new application gate rejected the native 3.46.0 binary when asked for 3.46.1 and accepted it only when explicitly checking 3.46.0; startup tests compare the event with an independent SQLx connection instead of assuming the native engine version.

## 2026-09-10 — Final acceptance review

- A passing JavaScript-enabled declaration test did not cover progressive enhancement. With JavaScript disabled, the confirmation remains hidden and required result/date/category/file controls block the form before any POST. Test the rendered control state and actual request emission for no-JS branches, not only direct HTTP submission handlers or no-JS administrator forms.
- Other Award's legal positive fixture always supplied school_honor_category, masking a rule error. A valid national award without this school-only optional field returned 422; its UI also lacked the prescribed choices. Derive category applicability from the requirements and test removal of non-applicable fields independently from required-field rejection cases. Do not treat the existing validator's behavior as the expected contract.
- A temporary HTTP report parser initially looked only for field-error nodes and missed the declaration's Chinese summary-only confirmation error. The 422 response was correct; support both field and summary messages before labeling a validation response defective. The corrected finite matrix recorded 109 expected results and the one genuine school-honor mismatch across 110 cases.
- An extra trailing blank line in a generated acceptance table was caught only after staging: plain git diff --check excludes untracked files. Review generated artifacts with git diff --cached --check after staging the exact delivery paths as well.

## 2026-09-10 — Local network preview

- Colima's printed ssh-config used a colima- profile alias, while the actual Lima ssh.config file used lima-colima-. Passing the former alias with the latter file skipped its Host block and attempted DNS/port 22, producing a closed connection. Inspect the actual Host stanza or ssh -G before creating a forward; using the matching alias reached the configured loopback VM port. A dedicated forward bound to the Wi-Fi address exposes the preview locally without recreating the container or resetting sessions.

## 2026-09-10 — Local feedback and acceptance fixes

- A new declaration test failed to compile with E0599 because AcademicYearRepo exposes insert, not create. Check actual repository interfaces before claiming a meaningful red test; compilation from a nonexistent fixture helper is not evidence of the missing behavior. Two implementation-provider HTTP 503 responses were infrastructure failures; preserved edits and test evidence allowed another worker to continue.
- An unchanged session GET need not emit Set-Cookie. A restart fixture incorrectly discarded the still-valid original cookie; preserve its cookie jar and distinguish session mutation from ordinary reads.
- Adding ciphertext/version fields enlarged Submission enough for strict Clippy to flag DashboardRow as large_enum_variant. Box the submission payload while preserving row behavior. Another Clippy nonminimal_bool finding was resolved with named applicability predicates rather than suppressing the lint.
- New certificate fixtures changed XLSX row positions, and JSON may render an Excel numeric value as 525.0 instead of 525. Assert the intended row identity and numeric type/value, not incidental fixture positions or numeric string spelling.
- A hidden, disabled submit button can still become a form's implicit default and swallow Enter in a text input. For JavaScript-enhanced forms, change fallback refresh buttons to type=button as well as hiding/disabling them; exercise real keyboard submission in the browser.
- Correcting school-honor applicability can erase legacy non-school text that the old validator forced users to enter. Preserve historical values through prefilling, field refresh and editing, and test this separately from the ordinary school-only field behavior.
- Recoverable edit codes require the original SESSION_SECRET as well as the database backup. Keep nullable ciphertext for old hash-only records, explain that originals are unavailable, and require an explicit version-checked reset. Recheck the credential version after multipart parsing under the write lock so concurrent resets revoke outstanding student writes and refreshes.
- Apply-patch context/duplicate-target rejections did not change application behavior. Inspect current context and combine ordered hunks for the same file; verify successful changes before running build gates.

## 2026-09-10 — Rosters and deletion

- A required roster changes both normal submission and no-material declaration fixtures; update tests to prepare real allowed identities instead of bypassing the new check. Changing a student's name now also requires a matching roster identity. Assertions counting raw name occurrences must exclude new accessible checkbox labels.
- Soft-deleted declarations need a partial unique index on live identities. Keep the matching WHERE clause on UPSERT conflict targets; this permits a new declaration without implicitly restoring a deleted record and makes conflicting recovery fail atomically.
- Removing startup year seeding is necessary when deleting the last year is a supported action. Otherwise a restart can silently recreate an apparently deleted academic year.
- Bounded XLSX archive size alone does not bound a sparse worksheet's rectangular allocation. Validate worksheet cell coordinates before Calamine constructs the range, and require text identity cells to avoid silently losing leading zeros or long-number precision.
- The host Docker CLI lacked Buildx and fell back to the legacy builder, which failed resolving cached content. Running an isolated official Buildx 0.20.1 binary against the existing Colima socket used the valid BuildKit cache and completed the unchanged production Dockerfile and SQLite version gate.

## How to add a lesson

Record the date, the observed problem or decision, and the rule that should guide future work. Do not store secrets, personal data, upload contents, or temporary guesses.
