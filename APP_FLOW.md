# Application Flow

Version: 1.0
Language rule: This document intentionally uses English only.

## Global Navigation

The public navigation contains Home, Submit, Query/Edit, and a discreet Admin Login link. A student must pass the class access code before opening Submit. An administrator must pass the admin session guard before opening any `/admin/*` route. The application uses server-rendered HTML and normal form submissions; JavaScript only controls conditional fields, client hints, and progressive enhancement.

## Page: Home (`GET /`)

Trigger: The visitor opens the site root.

Steps:

1. Load the single active academic year, its date range, optional deadline, announcement, and class name.
2. Render the six required notices: one result per submission, repeat for multiple results, submit the highest level for duplicates, scholarships are normally excluded, final recognition is decided by review, and evidence must be clear and authentic.
3. Render a class access code field and a Submit button.
4. Render a Query/Edit link that leads to `/query` without requiring the class access code.

Success state: The active year is shown with a primary action to continue.

Empty state: If no active year exists, show “Submission is currently closed” and keep Query/Edit and Admin Login available.

Error state: Database failure returns the generic 500 page and logs the internal error without exposing SQL or paths.

## Page: Class Access (`POST /access`)

Trigger: The visitor submits the access code form on Home.

Steps:

1. Validate a non-empty code against the Argon2id hash in `settings`.
2. Apply a short failure delay after an invalid attempt.
3. On success, create a student access session containing only a random session identifier and the active academic year id.
4. Redirect to `/submit` using HTTP 303.

Success state: The student sees the submission form.

Error state: Keep the form visible and show “The class access code is incorrect.” Do not distinguish a missing setting from a wrong code.

## Page: Submit (`GET /submit`)

Trigger: A valid student access session opens `/submit`.

Steps:

1. Verify that the session is present and an active year still exists.
2. Render name, student number, and “Has a result to submit?” radio buttons.
3. If the answer is No, show the no-material declaration confirmation and no file input.
4. If the answer is Yes, show common result fields, category select, seven conditional sections, and a multi-file input.
5. The browser script hides irrelevant category sections and marks hidden inputs disabled.

Success state: The page is ready for one result or one no-material declaration.

Error state: A missing session redirects to Home with an access prompt. If no active year exists, show the closed state.

## Page: Submit Result (`POST /submit`)

Trigger: The student submits a result form with multipart data.

Steps:

1. Parse all text fields and multipart parts with a hard request body limit.
2. Re-check the student session, active year, deadline, date range, common fields, category enum, conditional category fields, file count, file sizes, MIME types, and extensions.
3. Acquire the database write lock after parsing, reload the active year and repeat the session-year, deadline and date-range checks before persistence. Hash a generated edit code, allocate a unique submission number by its ending-year prefix, and save an authenticated recovery envelope bound to that number alongside the hash in the transaction.
4. Stream each file to `UPLOAD_DIR/year-<academic_year_id>/<submission_no>/<random-name>`, then insert attachment metadata in the same transaction. Remove partial files if the transaction fails.
5. Commit and log the submission number only.
6. Redirect to `/success/<submission_no>` with a short-lived receipt session.

Success state: The Success page displays the result name, submission number, plain-text edit code, and links to submit another result or query/edit.

Error state: Render the form again with field-level Chinese errors. Do not show raw parser, SQL, filesystem, or Rust errors.

## Page: Submit No-Material Declaration (`POST /submit`)

Trigger: The student selects No and confirms.

Steps:

1. Validate name and student number.
2. Acquire the database write lock, reload the active year, check session scope and the current deadline, then upsert the active year and student identity in `student_declarations` in that transaction.
3. Do not create a submission or attachment.
4. Redirect to a confirmation page with a link to submit a result later.

Success state: The declaration is visible to administrators in the active year summary.

Error state: Invalid identity or inactive year returns an inline Chinese validation error.

## Page: Success (`GET /success/:submission_no`)

Trigger: The student follows the redirect after a successful result submission.

Steps:

1. Consume the short-lived receipt scope as one number/code/version value. Verify its number, credential version and code against the current record; a reset invalidates an outstanding receipt.
2. Display the submission number and the plain edit code once in a prominent, copyable block.
3. Display the result name and current status as Pending.
4. Provide “Submit another result” and “Query/Edit” links.

Success state: The receipt is available without exposing the stored hash.

Error state: A missing or expired receipt session shows a generic not-found message; it never reveals whether a submission number exists.

## Page: Query (`GET /query`, `POST /query`)

Trigger: The student opens Query/Edit or submits credentials.

Steps:

1. Render submission number and edit code fields.
2. On POST, normalize surrounding whitespace and verify the edit code against the stored hash.
3. Create a short-lived verified-student session containing the submission id and credential version together. A reset after verification cannot turn a late session write into access to the new credential version.
4. Redirect to `/query/<submission_no>`.

Success state: The detail page shows the submission and allowed actions.

Error state: Wrong credentials always return “The submission number or edit code is incorrect.” The response does not reveal which field failed.

## Page: Student Detail (`GET /query/:submission_no`)

Trigger: A verified-student session opens the matching submission.

Steps:

1. Verify the session submission id and credential version equal the current record. Never disclose the code on this page.
2. Render common fields, category fields, status, review note, score, timestamps, and attachment links.
3. Render an Edit button only for Pending or Needs Revision.
4. Render a read-only notice for Approved or Rejected.

Success state: The student can inspect only their verified submission and attachments.

Error state: Session mismatch returns 403; missing record returns 404 without leaking identifiers.

## Page: Student Update (`POST /query/:submission_no/update`)

Trigger: The student submits the editable detail form.

Steps:

1. Verify the student session, CSRF token, and current editable status.
2. Re-run common fields, category, date, and upload validation against the submission's original academic year. The deadline limits new submissions only; existing Pending and Needs Revision records remain editable after the deadline or year deactivation.
3. Replace common/category values and add new attachments in a transaction. Keep existing attachments; an update may add none, and the combined total must remain 1-10. Condition the initial write on both editable status and the verified credential version, then count/save attachments under that write lock. An administrator reset during multipart parsing rejects the old update before files are saved.
4. Set status to Pending, preserve the review note, set `student_modified_after_review=true`, and update `updated_at`.
5. Redirect to the detail page.

Success state: The page shows Pending and a “resubmitted for review” notice.

Error state: Approved and Rejected updates are rejected with 403; invalid form data is returned with field errors.

## Page: Admin Login (`GET/POST /admin/login`)

Trigger: The visitor opens the Admin Login link.

Steps:

1. Render username and password fields plus a CSRF token.
2. On POST, compare the configured username and Argon2id password hash.
3. Add a failure delay for invalid credentials and use one generic error message.
4. On success, create an admin session and redirect to `/admin`.

Success state: The administrator sees the submissions list.

Error state: Invalid credentials return the same message for unknown username and wrong password. All other admin routes redirect to Login or return 403 according to route policy.

## Page: Admin Dashboard (`GET /admin`)

Trigger: A valid admin session opens the dashboard.

Steps:

1. Load active year, selected result submissions, no-material declarations, counts by result status, and approved score total.
2. Apply optional filters: academic year, name keyword, student number keyword, category, and status. An omitted year defaults to the active year, an empty year selects all years, and no active year defaults to all historical years. Name and student number keywords match literal substrings.
3. Merge result and declaration display rows, sort by `created_at DESC` with a stable result/declaration and record-id tie order, and render the compact table. Declaration rows show their real identity and `无申报材料`, without a result number, score, or detail/review link.
4. Provide detail links for result rows plus Settings, Export, and Logout.

Success state: The table row count includes both result and declaration rows. Result total/status counters and the approved score total remain result-only; the declaration counter remains separate. No-material declarations apply only academic year, name, and student number because they have no category or review status, so declaration rows and their counter remain visible under category/status filters when their identity filters match. The dashboard explains this beside the counts.

Error state: Invalid filter values are ignored with a visible non-blocking message; database failure returns 500.

## Page: Admin Submission Detail (`GET /admin/submissions/:id`)

Trigger: The administrator selects a row.

Steps:

1. Load submission, category data, attachments, and audit marker.
2. Render all fields and image previews where safe; PDF files use protected open/download links.
3. Render status select, review note textarea, score input, and CSRF token.
4. Offer an accessible native disclosure for the current edit code, decrypting the record-bound envelope and checking it against the current verifier. This protected response is no-store. Codes never enter ordinary lists, student details, exports, logs or URLs.
5. For legacy records without ciphertext, explain that the original code cannot be recovered. For damaged, stale or wrong-key ciphertext, show a safe unavailable message. Reading or upgrading never resets a credential; a saved valid student code still works.

Success state: The administrator can review the complete record.

Error state: Unknown id returns 404; attachment failure shows an attachment-level error while preserving the record page.

## Action: Admin Reset Edit Code (`POST /admin/submissions/:id/edit-code/reset`)

The administrator checks an explicit confirmation that the previous code and verified student sessions will stop working, then submits the protected CSRF form with the displayed credential version. A conditional transaction replaces hash and authenticated ciphertext and increments the version together; stale/concurrent forms return a safe 409 and cannot overwrite a later reset. Missing confirmation, CSRF or version returns 400; unauthorized writes return 403. GET does not reset anything.

After success, redirect to the protected detail page to view the current new code. Preserve all submission content, review status, note, score, timestamps and attachments. The student must verify again using the new code. Keep the persistent SESSION_SECRET with protected backups; key rotation requires migrating existing envelopes. Losing or changing that key makes old envelopes unreadable but does not change their verification hashes.

## Page: Admin Review (`POST /admin/submissions/:id/review`)

Trigger: The administrator saves a review.

Steps:

1. Verify admin session and CSRF token.
2. Validate status enum and score format (non-negative, maximum two decimals); require a score for Approved.
3. Update status, note, score, and `updated_at` atomically.
4. Log the transition with submission id and new status, never credentials.
5. Redirect back to detail with a success notice.

Success state: The new status, note, and score are visible immediately.

Error state: Invalid score or missing Approved score returns the form with a Chinese error.

## Page: Admin Settings (`GET /admin/settings`)

Trigger: The administrator opens Settings.

Steps:

1. List all academic years and identify the sole active year.
2. Allow create (`POST /admin/years`), edit (`POST /admin/years/:id`), and activate (`POST /admin/years/:id/activate`) operations through separate POST forms. The optional `edit` query on Settings prefills the editor. New years are inactive; metadata edits preserve activation. Activation is transactional and retains the previous active year if it fails. Names, dates, optional deadline and announcement are editable. The deadline input explicitly uses UTC and can be cleared; it is independent of the achievement date range.
3. Allow class access code replacement (`POST /admin/settings/class-code`); require 1-256 characters, reject entirely blank/control-character values, hash the exact entered value before writing, and never display the old or new plain code or stored hash after submission. The next access verification uses the replacement hash; existing short-lived sessions keep their scope.
4. Keep historical submissions available after year changes.

Success state: Settings changes apply to the next request without restart.

Error state: Invalid dates/order, duplicate year name, invalid deadline, overlong text or blank code returns inline Chinese field errors with a summary. Retain non-secret editor values; always clear the code input. A concurrent database lock returns an inline conflict notice without changing settings.

## Page: Admin Export (`GET /admin/export.xlsx`)

Trigger: The administrator clicks Export Current Year in the shared navigation, or Export Filtered Results beside the dashboard table.

Steps:

1. Verify the admin session and use the dashboard's year/name/student-number/category/status validation. An omitted year defaults to the active year, an explicitly empty year selects all years, and no active year defaults to all historical years. The filtered link carries the displayed year explicitly, so switching the active year does not change its scope.
2. Read academic years, selected submissions, declarations and attachment metadata in one database snapshot. Declarations apply only year/name/student-number filters because they have no category/status.
3. Generate `申报明细` (34 fixed columns) and `学生汇总` (10 fixed columns) off the asynchronous request workers. Group the union of submissions and declarations by student number plus name; only Approved scores contribute to totals. Dates/timestamps and scores are native spreadsheet values, while student numbers and other identifiers remain text. Timestamps use UTC. Category fields are expanded into their named columns; supplemental fields with no dedicated column are retained as labeled prose in Detail.
4. Return an `.xlsx` response with a sanitized UTF-8 filename `<academic year>综测申报汇总.xlsx`, an ASCII fallback, `Cache-Control: no-store` and `X-Content-Type-Options: nosniff`. All-year exports use `全部学年` as the filename prefix. Attachment cells contain original names and protected relative download identifiers, never public file paths or credentials.

Success state: The workbook downloads and opens in Excel or LibreOffice.

Compatibility state: Dates before 1900 remain readable ISO text. Text beyond Excel's 32,767-unit cell limit has an explicit truncation notice and a protected administrator-detail path for the full original. The database text remains unchanged. Non-finite stored numeric values produce the generic generation error instead of text in a numeric column.

Error state: Empty results still produce headers, filters and frozen first rows in a valid workbook. Invalid filter values redirect back to the dashboard with the original filter values, where the existing visible notice explains that invalid conditions were ignored; the administrator can then export the displayed selection. Extraction errors return a safe Chinese 400. Generation failure returns a generic Chinese 500 and logs only the route/status/failure stage, without cell contents, credentials or internal paths.
