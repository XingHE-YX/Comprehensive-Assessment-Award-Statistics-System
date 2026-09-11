CREATE TABLE academic_year_students (
    academic_year_id INTEGER NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    student_no TEXT NOT NULL CHECK (length(student_no) BETWEEN 1 AND 30),
    student_name TEXT NOT NULL CHECK (length(student_name) BETWEEN 1 AND 50),
    created_at TEXT NOT NULL,
    PRIMARY KEY (academic_year_id, student_no)
);

ALTER TABLE submissions ADD COLUMN deleted_at TEXT;
ALTER TABLE student_declarations ADD COLUMN deleted_at TEXT;
DROP INDEX student_declarations_identity;
CREATE UNIQUE INDEX student_declarations_identity ON student_declarations
    (academic_year_id, student_no, student_name) WHERE deleted_at IS NULL;
CREATE INDEX submissions_deleted_at ON submissions(deleted_at);
CREATE INDEX declarations_deleted_at ON student_declarations(deleted_at);

-- Retried after commit and on startup; metadata deletion never exposes orphan files.
CREATE TABLE pending_attachment_deletions (
    stored_name TEXT PRIMARY KEY,
    created_at TEXT NOT NULL
);
