# Lessons Learned

This file stores verified, reusable engineering lessons for the project. Add only concrete lessons that affect future implementation decisions.

## 2026-09-07

- The repository currently contains specifications only; implementation must begin from a Rust/Axum skeleton.
- The frontend is intentionally server-rendered and minimal. Do not introduce a JavaScript build chain for questionnaire interactions.
- Category-specific data is stored as JSON for development speed, but every export path must expand it into named Excel columns.
- Student edit access is credential-based rather than account-based. Hash the edit code and scope the verified session to one internal submission id.
- The active academic year is a database invariant: activation must close other active years in the same transaction.

## How to add a lesson

Record the date, the observed problem or decision, and the rule that should guide future work. Do not store secrets, personal data, upload contents, or temporary guesses.
