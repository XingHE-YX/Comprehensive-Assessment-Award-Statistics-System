CREATE UNIQUE INDEX academic_years_one_active
    ON academic_years (is_active)
    WHERE is_active = 1;

CREATE INDEX submissions_year_status_created
    ON submissions (academic_year_id, status, created_at DESC);

CREATE INDEX submissions_student_no
    ON submissions (student_no);

CREATE INDEX submissions_student_name
    ON submissions (student_name);

CREATE INDEX submissions_category
    ON submissions (category);

CREATE INDEX attachments_submission
    ON attachments (submission_id);

CREATE UNIQUE INDEX student_declarations_identity
    ON student_declarations (academic_year_id, student_no, student_name);
