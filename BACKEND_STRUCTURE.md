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
| POST | `/submit` | multipart form + CSRF | 303 success, 422 validation |
| GET | `/success/:submission_no` | receipt session | 200 success HTML |
| GET | `/query` | none | 200 query form |
| POST | `/query` | form `submission_no`, `edit_code`, CSRF | 303 detail or 401 |
| GET | `/query/:submission_no` | verified student session | 200 detail |
| POST | `/query/:submission_no/update` | multipart + CSRF | 303 detail, 403/422 |
| GET | `/submissions/:submission_no/attachments/:id` | verified student or admin session | protected bytes |
| GET | `/healthz` | none | 200 `ok` only |

HTML errors: 400 invalid request, 401 invalid credential, 403 unauthorized, 404 missing resource, 413 body/file too large, 422 validation, 500 internal error.

## 5. Admin route contracts

All routes below require an admin session and CSRF on POST.

| Method | Path | Request | Response |
|---|---|---|---|
| GET | `/admin/login` | none | 200 login HTML |
| POST | `/admin/login` | `username`, `password`, CSRF | 303 `/admin` or 401 |
| POST | `/admin/logout` | CSRF | 303 `/admin/login` |
| GET | `/admin` | query filters | 200 dashboard HTML |
| GET | `/admin/submissions/:id` | internal numeric id | 200 detail HTML |
| POST | `/admin/submissions/:id/review` | `status`, `review_note`, `approved_score`, CSRF | 303 detail or 422 |
| GET | `/admin/settings` | none | 200 settings HTML |
| POST | `/admin/years` | year fields, CSRF | 303 settings or 422 |
| POST | `/admin/years/:id` | year fields, CSRF | 303 settings or 422 |
| POST | `/admin/years/:id/activate` | CSRF | 303 settings or 409 |
| POST | `/admin/settings/class-code` | `class_access_code`, CSRF | 303 settings or 422 |
| GET | `/admin/export.xlsx` | year and optional filters | XLSX bytes |

Dashboard filters are `academic_year_id`, `name`, `student_no`, `category`, and `status`. Unknown enum values are rejected with 400.

## 6. Authentication and authorization

### Admin

`ADMIN_USERNAME` is compared in constant-time where practical; `ADMIN_PASSWORD_HASH` is an Argon2id PHC string. A successful login stores only `admin_authenticated=true` and a session creation timestamp. Every admin route runs `require_admin`. The cookie is `HttpOnly`, `SameSite=Lax`, and `Secure` in production. Login failures use one generic message and a bounded delay.

### Student class access

The class access code is stored as `class_access_code_hash` using Argon2id. Successful access creates a short-lived session scoped to the active academic year. It is not an identity credential and grants only submission form access.

### Submission edit code

An 8-10 character code is generated from `ABCDEFGHJKLMNPQRSTUVWXYZ23456789`, hashed with Argon2id, and shown only on the receipt page. Query verification creates a session containing the internal submission id and expiry. Every detail/update/attachment request checks that session id against the target record.

### CSRF

Every state-changing form receives a per-session random CSRF token. The token is submitted as a hidden field and compared in constant time. GET never mutates data. SameSite cookies are defense in depth, not the sole CSRF control.

## 7. Validation and storage rules

Validation is shared by create and update paths. Date, category, conditional fields, attachment count/size/type, score, and text lengths are checked server-side. Uploads stream to a temporary file, validate byte count and declared/guessed MIME, then atomically move to the year/submission directory with a random filename. Static file serving never mounts `UPLOAD_DIR`.

Student updates validate dates against the record's original academic year. Following PRD section 6, the deadline applies to new submissions only. Existing attachments count toward the 1-10 limit and do not need to be re-uploaded. Student update transactions condition their first write on editable status, then recount attachments before saving additional files. Private student HTML and attachment responses use `Cache-Control: no-store`; attachments also use `X-Content-Type-Options: nosniff`.

## 8. Error and logging contract

`AppError` maps internal errors to stable Chinese messages and status codes. Logs use tracing fields `request_id`, `route`, `submission_id`, `submission_no`, and `status`; never log passwords, codes, session secrets, raw multipart values, or absolute storage paths.
