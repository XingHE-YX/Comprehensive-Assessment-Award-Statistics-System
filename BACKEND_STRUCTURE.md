# 后端结构文档

## 1. Runtime architecture

The backend is one Rust binary using Axum and Askama. Caddy terminates TLS and proxies to `web:3000`. The binary owns HTML rendering, SQLite access, upload authorization, and workbook generation. `AppState` contains the SQLite pool, parsed configuration, session layer, and attachment storage service.

```text
HTTP request
  -> middleware: request id, tracing, session, CSRF, admin guard where required
  -> route handler
  -> validation/service
  -> repository transaction
  -> Askama HTML or XLSX response
```

## 2. Module boundaries

```text
src/main.rs                 startup, migrations, router, graceful shutdown
src/config.rs               environment parsing and limits
src/error.rs                AppError and public error responses
src/state.rs                AppState
src/domain/*                typed business values and serde category models
src/validation/*            server-side validation only
src/services/*              coordinate validation, repository transactions, and storage cleanup
src/auth/*                  Argon2id, codes, sessions, CSRF
src/db/*                    SQLx repositories and transactions
src/storage/*               safe paths and streamed file IO
src/routes/*                HTTP extraction, service calls, redirects/templates
src/export/*                fixed XLSX column mapping and formatting
```

Route handlers must not contain raw SQL, password hashing, path concatenation, or category validation rules.

## 3. Database schema

SQLite is opened with foreign keys enabled, WAL mode, and a busy timeout. All timestamps are UTC RFC 3339 text. IDs are 64-bit integer primary keys internal to the database; public access uses `submission_no`.

### `academic_years`

| Column | Type | Null | Constraints |
|---|---|---:|---|
| `id` | INTEGER | no | primary key |
| `name` | TEXT | no | unique, 1-40 chars |
| `start_date` | TEXT | no | ISO date |
| `end_date` | TEXT | no | ISO date, >= start |
| `deadline` | TEXT | yes | UTC RFC 3339 |
| `is_active` | INTEGER | no | 0/1, only one true |
| `announcement` | TEXT | yes | max 4000 chars |
| `created_at` | TEXT | no | UTC timestamp |
| `updated_at` | TEXT | no | UTC timestamp |

Unique partial index: `academic_years_one_active` on `is_active` where `is_active=1`.

### `submissions`

| Column | Type | Null | Constraints |
|---|---|---:|---|
| `id` | INTEGER | no | primary key |
| `submission_no` | TEXT | no | unique, `^ZC[0-9]{4}-[0-9]{6}$` |
| `academic_year_id` | INTEGER | no | FK to `academic_years.id` |
| `student_name` | TEXT | no | trimmed, 1-50 chars |
| `student_no` | TEXT | no | trimmed, 1-30 chars |
| `category` | TEXT | no | fixed seven-value enum |
| `result_name` | TEXT | no | 1-200 chars |
| `obtained_date` | TEXT | no | ISO date |
| `detail` | TEXT | yes | max 4000 chars |
| `remark` | TEXT | yes | max 4000 chars |
| `category_data` | TEXT | no | valid JSON object |
| `status` | TEXT | no | pending/approved/needs_revision/rejected |
| `review_note` | TEXT | yes | max 4000 chars |
| `approved_score` | REAL | yes | >=0, max 2 decimals |
| `edit_code_hash` | TEXT | no | Argon2id PHC string |
| `edit_code_ciphertext` | TEXT | yes | versioned authenticated envelope; legacy rows remain NULL |
| `edit_code_version` | INTEGER | no | default 0, non-negative, incremented atomically on reset |
| `student_modified_after_review` | INTEGER | no | 0/1 |
| `created_at` | TEXT | no | UTC timestamp |
| `updated_at` | TEXT | no | UTC timestamp |

Foreign key is `ON DELETE RESTRICT` for the academic year. Indexes cover year/status/date, student number, name, and category.

### `attachments`

| Column | Type | Null | Constraints |
|---|---|---:|---|
| `id` | INTEGER | no | primary key |
| `submission_id` | INTEGER | no | FK `ON DELETE CASCADE` |
| `original_name` | TEXT | no | max 255 chars, display only |
| `stored_name` | TEXT | no | unique random filename, no path separators |
| `mime_type` | TEXT | no | image/jpeg, image/png, application/pdf |
| `file_size` | INTEGER | no | 1..10485760 |
| `created_at` | TEXT | no | UTC timestamp |

### `student_declarations`

| Column | Type | Null | Constraints |
|---|---|---:|---|
| `id` | INTEGER | no | primary key |
| `academic_year_id` | INTEGER | no | FK to academic years |
| `student_name` | TEXT | no | trimmed |
| `student_no` | TEXT | no | trimmed |
| `has_submission_material` | INTEGER | no | v1 always 0 |
| `created_at` | TEXT | no | UTC timestamp |
| `updated_at` | TEXT | no | UTC timestamp |

Unique index: `(academic_year_id, student_no, student_name)`.

Migration 0004 adds nullable `deleted_at` to submissions/declarations. Declaration identity uniqueness now applies only to non-deleted rows. Normal record/authentication/export queries and conditional review/reset/student writes exclude deleted rows. Recovery clears the timestamp and preserves business data; identity conflicts return 409 and roll back the batch.

`academic_year_students` stores `(academic_year_id, student_no)` as its primary key, `student_name` and `created_at`; its year FK cascades. Parsing/identity validation live in `services/roster`. The session roster draft has a random id, validated students and one-hour expiry. New-year creation requires the matching draft id and inserts the year/roster in one transaction. Startup no longer seeds a year. Settings activation rejects empty rosters.

`pending_attachment_deletions(stored_name PRIMARY KEY, created_at)` persists cleanup work. Permanent year deletion queues random attachment names, deletes children, then deletes the selected year transactionally. Post-commit cleanup removes files through protected storage lookup, including legacy layouts; missing files count as cleaned. Failures stay queued for subsequent cleanup/startup.

### `settings`

| Column | Type | Null | Constraints |
|---|---|---:|---|
| `key` | TEXT | no | primary key |
| `value` | TEXT | no | secret hashes or non-secret text |
| `updated_at` | TEXT | no | UTC timestamp |

Required keys: `class_access_code_hash`; optional keys: `class_name`, `site_title`.

## 4. Public route contracts

| Method | Path | Request | Response |
|---|---|---|---|
| GET | `/` | none | 200 Home HTML |
| POST | `/access` | form `access_code`, CSRF | 303 `/submit` or 400 |
| GET | `/submit` | student session | 200 form |
| POST | `/submit` | multipart form + CSRF | 303 success, 422 validation; 200 for non-persisting field refresh |
| GET | `/success/:submission_no` | receipt session | 200 success HTML |
| GET | `/query` | none | 200 query form |
| POST | `/query` | form `submission_no`, `edit_code`, CSRF | 303 detail or 401 |
| GET | `/query/:submission_no` | verified student session | 200 detail |
| POST | `/query/:submission_no/update` | multipart + CSRF | 303 detail, 403/422; 200 for authorized field refresh |
| GET | `/submissions/:submission_no/attachments/:id` | verified student or admin session | protected bytes |
| GET | `/healthz` | none | 200 `ok` only |

HTML errors: 400 invalid request, 401 invalid credential, 403 unauthorized, 404 missing resource, 413 body/file too large, 422 validation, 500 internal error.

## 5. Admin route contracts

Login is public and requires CSRF on POST. All other implemented routes below require an admin session and CSRF on POST. Anonymous GET/HEAD requests to protected admin pages redirect to `/admin/login` with 303; unauthorized writes return 403 before parsing the request.

| Method | Path | Request | Response |
|---|---|---|---|
| GET | `/admin/login` | none | 200 login HTML |
| POST | `/admin/login` | `username`, `password`, CSRF | 303 `/admin` or 401 |
| POST | `/admin/logout` | CSRF | 303 `/admin/login` |
| GET | `/admin` | query filters | 200 dashboard HTML |
| GET | `/admin/submissions/:id` | internal numeric id | 200 detail HTML |
| POST | `/admin/submissions/:id/review` | `status`, `review_note`, `approved_score`, CSRF | 303 detail or 422 |
| POST | `/admin/submissions/:id/edit-code/reset` | `confirm_reset=yes`, expected `edit_code_version`, CSRF | 303 detail, 400 invalid confirmation, 409 stale version |
| GET | `/admin/settings` | none | 200 settings HTML |
| POST | `/admin/years` | year fields, CSRF | 303 settings or 422 |
| POST | `/admin/years/:id` | year fields, CSRF | 303 settings or 422 |
| POST | `/admin/years/:id/activate` | CSRF | 303 settings or 409 |
| POST | `/admin/settings/class-code` | `class_access_code`, CSRF | 303 settings or 422 |
| GET | `/admin/export.xlsx` | year and optional filters | XLSX bytes |
| GET | `/admin/roster/template.xlsx` | admin session | two-column XLSX template |
| POST | `/admin/roster/prepare` | multipart file or pasted roster, CSRF | 303 settings or 422 |
| GET/POST | `/admin/years/:id/students` | read / append multipart roster, CSRF | roster HTML / 303 or 422 |
| GET/POST | `/admin/years/:id/delete` | summary / full name, confirmation, CSRF | HTML / 303 or 400/409 |
| POST | `/admin/records/delete/confirm` | selected keys, CSRF | confirmation HTML |
| POST | `/admin/records/delete` | selected keys, confirmation, CSRF | 303 recycle or 400/409 |
| GET | `/admin/recycle` | optional year filter | recycle HTML |
| POST | `/admin/records/restore` | selected keys, CSRF | 303 recycle or 400/409 |

Record keys are validated `s:<id>` or `d:<id>` values in `selected:<key>=yes` form fields; table names come only from a Rust enum. Batches require 1-500 existing records and commit atomically. Import is limited to 2 MiB/5000 rows, bounded ZIP expansion and worksheet coordinates before rectangular allocation. XLSX identities must be text to avoid numeric corruption. Student creation/amendment checks both trimmed identity values against the correct year's roster while holding the database write lock.

Dashboard filters are `academic_year_id`, `name`, `student_no`, `category`, and `status`. An omitted year defaults to the active year; an explicitly empty year selects all years. Without an active year, the default includes historical years. Following APP_FLOW.md, invalid enum/year values and overlong or control-character keywords are ignored with a visible non-blocking Chinese notice (200). Other valid filters still apply. Names (maximum 50 characters) and student numbers (maximum 30 characters) use literal substring matching, with SQL LIKE metacharacters escaped and values bound. Malformed path, query, and form extraction errors return a stable Chinese 400 response.

The dashboard table merges selected result submissions and no-material declarations by `created_at DESC`; exact timestamp ties place result rows first and then use descending ids within each record kind. Declaration rows expose their real name, student number and creation time, but no synthetic submission number, score, or result-detail/review URL. The table caption counts both record kinds. Result total/status counters and approved score totals still use only selected submissions, while the declaration counter remains separate. No-material declarations have no category/status, so their rows and counter apply only year/name/student-number filters, as stated beside the counters.

### Academic-year and class-code settings

`GET /admin/settings?edit=<id>` prefills the selected year; omitting `edit` shows the create form. All mutation routes above use POST and redirect with a fixed success notice. Validation failures return 422 with retained non-secret fields, field errors and a summary. Missing years return 404; malformed extraction returns a safe Chinese 400. SQLite busy/locked settings writes return an inline 409 and do not claim a save.

Year names are trimmed, unique and 1-40 characters, with no control characters. Dates must be valid `YYYY-MM-DD` values (years 0001-9999), with end >= start. Announcement text is optional, trimmed and limited to 4000 characters. Deadline is optional and independent of the achievement date range; past deadlines are allowed to close submissions. The native datetime-local input is labeled UTC, and the server interprets it as UTC. Explicit RFC 3339 offsets are also accepted and normalized to UTC; editing preserves seconds and fractional precision. An empty deadline removes the cutoff.

New years are inactive. Editing metadata never changes activation. Activation acquires SQLite's write lock, closes the previous active year and activates the target in one transaction; a missing target rolls the whole operation back. Historical submissions, declarations, attachments and review data are retained for listing, query and the Task 9 export implementation.

Class access codes contain 1-256 characters, cannot be entirely whitespace or contain control characters, and preserve the exact entered value. Hashing runs on a blocking worker before a transactional settings update. Neither the submitted code nor the hash is rendered. The new hash applies to the next access verification; existing short-lived submission sessions retain their original scope until expiry or a year switch.

### Excel export

`GET /admin/export.xlsx` uses the same administrator guard and normalized filters as the dashboard. Invalid filter values redirect with 303 to `/admin` with the same filter inputs so its visible warning is shown before a download. Default/current/historical/all-year and literal keyword semantics match the list. The filtered dashboard link includes an explicit academic-year id (or an empty all-year value), preserving the displayed scope across later activation changes.

`services/export` reads years, submissions, declarations and selected attachment metadata in one read transaction. Repository queries share bound filter predicates; attachments are fetched in one query without reading files. After the snapshot is released, `spawn_blocking` invokes `src/export/xlsx.rs`. There are no database writes, new migrations or new runtime dependencies.

The workbook contains the fixed 34-column detail and 10-column summary schema documented in `docs/testing-export.md`. Summary identities are the union of selected results and declarations, grouped by `(student_no, student_name)` across the selected years. Declaration filtering ignores category/status. Counts use selected submissions; the total includes only Approved scores and rounds to two decimals without overflowing large finite scores during rounding. All-year export intentionally merges identical student identities across years; the detail sheet retains each record's year. Non-applicable conditional fields remain empty, identifiers use string cells, dates/timestamps use date serials (UTC), and scores use numeric cells with `0.00` formatting. User strings never create spreadsheet formulas or hyperlinks.

Dates before 1900 fall back to ISO text for Excel compatibility. Overlong text cells are capped at 32,767 UTF-16 units including a visible truncation notice and a protected administrator-detail source path; complete original data remains in the database and canonical/legacy award fields are available on the detail page. Non-finite stored numeric values are rejected with the generic export error instead of becoming misleading text cells.

Responses use the XLSX MIME, `no-store`, `nosniff`, a fixed ASCII fallback filename and a UTF-8 filename parameter. Display-name path separators and reserved filename characters become underscores. Attachment cells list original names and the existing protected relative download routes; they never include storage names, disk paths or secrets. Workbook generation failures return `导出失败，请稍后重试` (500) and log only safe route/status/stage information. Empty exports retain both formatted headers, frozen first rows and auto-filters.

## 6. Authentication and authorization

### Admin

`ADMIN_USERNAME` is compared in constant-time where practical; `ADMIN_PASSWORD_HASH` is an Argon2id PHC string. A successful login stores only `admin_authenticated=true` and a session creation timestamp. Every admin route runs `require_admin`. The cookie is `HttpOnly`, `SameSite=Lax`, and `Secure` in production. Login failures use one generic message and a bounded delay. Password verification runs on a blocking worker and is performed even for an unknown username. Successful login rotates the session id and CSRF token; logout flushes the session.

### Student class access

The class access code is stored as `class_access_code_hash` using Argon2id. Successful access creates a short-lived session scoped to the active academic year. It is not an identity credential and grants only submission form access.

### Submission edit code

An 8-10 character code is generated from `ABCDEFGHJKLMNPQRSTUVWXYZ23456789`. Creation persists the Argon2id verifier and authenticated recovery ciphertext in the same transaction. `EditCodeVault` uses the already-enabled cookie 0.18.2 PrivateJar AES256-GCM implementation. The versioned cookie name binds the submission number as authenticated associated data; this is a server-only envelope, never an HTTP cookie. Its key is deterministically derived using `Key::derive_from` from the validated persistent SESSION_SECRET, independently of existing session-cookie key construction. No additional dependency is required. The vault and plaintext-bearing structures have no Debug output.

Only the single-use receipt and administrator-protected, no-store detail can display the code. The admin decrypts the envelope and verifies it against the current hash, so tampering, record swaps, wrong keys and old envelopes produce an unavailable message. Ordinary student details, lists, XLSX, URLs and application logs exclude codes and recovery material. The receipt stores its number/code/version together, is consumed on access, and must match the current version and verifier before display.

Query verification stores the internal submission id and credential version together, with a short expiry. Every student detail/update/attachment request checks both against the loaded record; legacy scopes default to version zero. Student persistence additionally conditions its first UPDATE on the expected version and editable status under SQLite's write lock before counting or saving files. A query session established late after a reset retains its old version and cannot bypass revocation.

Migration 0003 leaves legacy hashes, reviews and attachments untouched with NULL ciphertext/version zero. There is no automatic reset or recovery of one-way hashes. Manual administrator reset requires POST, CSRF, explicit invalidation confirmation and the version from the form. One conditional write updates hash, ciphertext and incremented version atomically, preserving every business field and timestamp. Competing/stale reset forms return safe 409; old codes, verified sessions and outstanding receipts are revoked. All response errors remain generic.

SESSION_SECRET must remain stable across restarts and be securely backed up with the data's recovery configuration. Rotating the key requires an explicit envelope migration using the old and new keys; changing or losing it alone makes stored envelopes unreadable, while saved student codes continue to verify against their hashes. Upgrade and page reads never silently replace those credentials.

### CSRF

Every state-changing form receives a per-session random CSRF token. The token is submitted as a hidden field and compared in constant time. GET never mutates data. SameSite cookies are defense in depth, not the sole CSRF control.

## 7. Validation and storage rules

Both multipart forms accept `form_action=refresh` for the server-rendered fallback. It is excluded from category JSON. Refresh verifies CSRF and session scope; editing additionally reloads the record and verifies the current credential version and editable status after parsing. Refresh retains submitted text but never saves records, declarations, or uploaded files. Normal submissions use the shared validation and transactional safeguards below.

School-honor category is optional for all awards; school-level forms offer six presets plus preserved historical custom text. Certification accepts both `CET-4/CET-6` and legacy `CET-6`, using the unchanged `cet6_score` key. Rendering normalizes the edit dropdown without rewriting JSON; export handles both certificate types with numeric score cells.

Validation is shared by create and update paths. Date, category, conditional fields, attachment count/size/type, score, and text lengths are checked server-side. Uploads stream to a temporary file, validate byte count and declared/guessed MIME, then atomically move to `UPLOAD_DIR/year-<academic_year_id>/<submission_no>/` with a random filename. New uploads use immutable year identifiers so editable names (including slashes) never become paths. Legacy display-name directories remain readable through the existing protected stored-name lookup; no file migration or public path is required. Static file serving never mounts `UPLOAD_DIR`.

New submission and declaration writes acquire the database write lock after multipart parsing, reload the active year, check the session's year and validate current settings before persistence. Settings cannot change between that validation and commit. Submission sequence allocation uses the four-digit public ending-year prefix across all academic-year records, so years with the same end year cannot create duplicate numbers.

Student updates validate dates against the record's original academic year. Following PRD section 6, the deadline applies to new submissions only. Existing attachments count toward the 1-10 limit and do not need to be re-uploaded. Student update transactions condition their first write on editable status, then recount attachments before saving additional files. Private student/admin HTML and attachment responses use `Cache-Control: no-store`; attachments also use `X-Content-Type-Options: nosniff`.

## 8. Error and logging contract

`AppError` maps internal errors to stable Chinese HTML messages and status codes using the shared layout, with a fixed fallback on template failure. Sources (including SQL/storage/session errors) are never rendered or formatted into logs. Framework extractor/static errors are normalized too. Form and multipart body-limit failures preserve 413; declared Content-Length is checked before extraction and DefaultBodyLimit bounds actual reads, including bodies without Content-Length.

Logs use tracing fields `request_id`, `route`, `submission_id`, `submission_no`, and `status`; never log passwords, codes, session secrets, raw multipart values, or absolute storage paths. Each response carries a newly generated UUID request id; client ids are ignored. Request spans use MatchedPath templates, never raw paths, queries or headers. Startup/migration, submission/update, upload failure, administrator login success/failure, review changes, export and safe application errors emit structured events. Only application targets pass the production logging filter; SQLx statement logging and dependency logs remain disabled even at trace level. Caddy access logging is not enabled, and its error formatter removes request and internal trace fields.

`GET /healthz` invokes the database repository readiness query and returns plain `ok` on success or the safe 500 HTML on failure. Production startup wires Config into the existing session/credential router and configured attachment storage; SIGINT/SIGTERM drains requests and closes the pool. Memory sessions expire on process restart, while SQLite and uploads persist in the required host mounts. Daily backup pauses application writes, makes a SQLite .backup plus matching uploads, checks integrity, atomically publishes the complete snapshot and restores previously running web service before retention (at least seven copies).
