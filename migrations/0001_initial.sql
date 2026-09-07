CREATE TABLE academic_years (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    start_date TEXT NOT NULL,
    end_date TEXT NOT NULL,
    deadline TEXT,
    is_active INTEGER NOT NULL DEFAULT 0 CHECK (is_active IN (0, 1)),
    announcement TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK (length(name) BETWEEN 1 AND 40),
    CHECK (date(start_date) IS NOT NULL),
    CHECK (date(end_date) IS NOT NULL),
    CHECK (date(end_date) >= date(start_date)),
    CHECK (announcement IS NULL OR length(announcement) <= 4000)
);

CREATE TABLE submissions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    submission_no TEXT NOT NULL UNIQUE,
    academic_year_id INTEGER NOT NULL,
    student_name TEXT NOT NULL,
    student_no TEXT NOT NULL,
    category TEXT NOT NULL,
    result_name TEXT NOT NULL,
    obtained_date TEXT NOT NULL,
    detail TEXT,
    remark TEXT,
    category_data TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    review_note TEXT,
    approved_score REAL,
    edit_code_hash TEXT NOT NULL,
    student_modified_after_review INTEGER NOT NULL DEFAULT 0 CHECK (student_modified_after_review IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (academic_year_id) REFERENCES academic_years(id) ON DELETE RESTRICT,
    CHECK (length(submission_no) = 13 AND submission_no GLOB 'ZC[0-9][0-9][0-9][0-9]-[0-9][0-9][0-9][0-9][0-9][0-9]'),
    CHECK (length(student_name) BETWEEN 1 AND 50),
    CHECK (length(student_no) BETWEEN 1 AND 30),
    CHECK (length(result_name) BETWEEN 1 AND 200),
    CHECK (detail IS NULL OR length(detail) <= 4000),
    CHECK (remark IS NULL OR length(remark) <= 4000),
    CHECK (review_note IS NULL OR length(review_note) <= 4000),
    CHECK (json_valid(category_data) = 1 AND json_type(category_data) = 'object'),
    CHECK (status IN ('pending', 'approved', 'needs_revision', 'rejected')),
    CHECK (approved_score IS NULL OR (approved_score >= 0 AND round(approved_score, 2) = approved_score)),
    CHECK (student_modified_after_review IN (0, 1))
);

CREATE TABLE attachments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    submission_id INTEGER NOT NULL,
    original_name TEXT NOT NULL,
    stored_name TEXT NOT NULL UNIQUE,
    mime_type TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (submission_id) REFERENCES submissions(id) ON DELETE CASCADE,
    CHECK (length(original_name) BETWEEN 1 AND 255),
    CHECK (file_size BETWEEN 1 AND 10485760),
    CHECK (mime_type IN ('image/jpeg', 'image/png', 'application/pdf')),
    CHECK (stored_name NOT LIKE '%/%' AND stored_name NOT LIKE '%\\%')
);

CREATE TABLE student_declarations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    academic_year_id INTEGER NOT NULL,
    student_name TEXT NOT NULL,
    student_no TEXT NOT NULL,
    has_submission_material INTEGER NOT NULL DEFAULT 0 CHECK (has_submission_material = 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (academic_year_id) REFERENCES academic_years(id) ON DELETE RESTRICT,
    CHECK (length(student_name) BETWEEN 1 AND 50),
    CHECK (length(student_no) BETWEEN 1 AND 30)
);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK (length(key) BETWEEN 1 AND 100)
);
