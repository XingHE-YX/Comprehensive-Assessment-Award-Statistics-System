# Administrator Flow Verification

Task 7 provides administrator routes through `routes::build_router_with_config(state, &config)`. It reads administrator credentials and session settings from the supplied `Config`. The older `build_router` entry point remains a student development/test helper with administrator login disabled. Production `main` wiring is still separate Task 1 work.

## Rust checks

```sh
cargo +1.88.0 fmt --check
cargo +1.88.0 test --test admin_flow
cargo +1.88.0 clippy --all-targets --all-features -- -D warnings
cargo +1.88.0 test --all-targets
```

The integration suite covers anonymous and student-session denial, uniform failed login, CSRF and session rotation, logout revocation, all filters and combinations, literal wildcard/injection-like keywords, active/historical/empty years, counts, approved-only totals including zero results, safe malformed-request errors, attachment authorization/absence, all review states, invalid scores/notes, atomic validation rejection, template escaping, and student-visible review results.

## Browser checks

Use the external Playwright 1.52.0 installation described in `docs/testing-student-flow.md` and installed Google Chrome. No Node dependency is added to the application or production image.

```sh
cargo +1.88.0 build --example admin_preview
ZONGCE_PLAYWRIGHT_MODULE=/path/to/external/node_modules/playwright node tests/admin_browser.cjs
```

The browser script starts the compiled preview on a random loopback port with an in-memory SQLite database and temporary uploads. It generates an ephemeral test password, passes it to the child process without logging it, and stops the preview afterward. If Cargo uses a custom target directory, set `ZONGCE_PREVIEW_BIN` to that directory's `debug/examples/admin_preview`.

Coverage at 320x568, 390x844, 768x1024 and 1440x900:

- Student submission with PNG/PDF attachments and query credentials.
- Student-session administrator denial; incorrect and correct admin login.
- Dashboard filters, status counters, zero/approved score totals, and narrow-screen table scrolling.
- Escaped detail content, protected PNG preview/PDF access, and anonymous attachment denial.
- Missing Approved score errors with retained form values, then Needs Revision review.
- Student amendment, preserved note/resubmission marker, final Approved review and student read-only result.
- Logout revokes detail and attachment access.

Administrator JavaScript is disabled in the tablet run to verify server-rendered forms remain usable. Screenshots are saved in `/tmp/zongce-admin-screenshots` (override with `ZONGCE_SCREENSHOTS`); the script also checks page overflow and JavaScript errors. Student seven-category regression instructions remain in `docs/testing-student-flow.md`.

At every viewport the feedback checks also compare the administrator's disclosed edit code with the synthetic student receipt, explicitly confirm a reset after review/export, reject the old code and student scope, and query with the replacement code while retaining status and attachments. A separate declaration appears under the status filter with its real identity and “无申报材料”, without a submission number or review link.

Settings is available after Task 8. Task 9 enables current-year and filtered Excel downloads; the administrator suite now checks filtered/empty/current-year XLSX content and logout revocation. See `testing-export.md`. Docker/Compose validation requires Task 10 files and an installed Docker runtime; this task does not validate deployment.


## Academic-year and class-code settings (Task 8)

```sh
cargo +1.88.0 test --test settings
cargo +1.88.0 build --example admin_preview
ZONGCE_PLAYWRIGHT_MODULE=/path/to/external/node_modules/playwright node tests/settings_browser.cjs
```

The settings suite covers administrator/CSRF protection; malformed input; empty settings; inactive creation; date ordering, duplicate names and text limits; optional UTC deadlines; atomic/idempotent activation, missing-target rollback, concurrent activation and an actual held SQLite write lock; immediate access-code replacement with hash-only persistence; historical queries and attachments; and numbering across years with the same ending year. It also verifies names with path separators, rename followed by new/additional uploads, and reading files with a fresh storage instance. The existing storage suite covers cold lookup of legacy display-name directories.

The in-flight settings regression uses a real HTTP connection with `Expect: 100-continue`. It waits for the server to start reading the body, changes settings through the admin routes, then sends the multipart body. Results/declarations must not persist into a newly inactive or closed year, and results must respect an updated date range. No timing sleeps or mocked database are used.

The browser script starts/stops an isolated admin preview and exercises create, activate, invalid date errors, prefilled editing, deadline closure/clearing, historical student/admin detail and attachment access, blank-code errors, and old/new class-code verification at all four prescribed viewport sizes. Tablet admin forms run with JavaScript disabled. It checks secret non-echo, duplicate IDs, JavaScript errors and page overflow. Screenshots default to `/tmp/zongce-settings-screenshots`; override with `ZONGCE_SCREENSHOTS`. Use `ZONGCE_PREVIEW_BIN` when the compiled preview is in a custom Cargo target directory.

Task 9 consumes the preserved historical records, and the settings browser suite now downloads the renamed historical year after activating a replacement. Docker Compose and production startup remain the previously recorded Task 10/Task 1 work.
