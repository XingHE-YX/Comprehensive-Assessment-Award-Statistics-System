# Student Flow Verification

Task 6 uses the existing Axum router and a temporary preview example. Production configuration and startup remain separate Task 1 work.

## Rust Checks

```sh
cargo +1.88.0 fmt --check
cargo +1.88.0 test --test query_flow --test auth
cargo +1.88.0 clippy --all-targets --all-features -- -D warnings
cargo +1.88.0 test --all-targets
```

## Browser Checks

Start the preview from the repository root:

```sh
cargo +1.88.0 run --example student_preview
```

The process prints its loopback URL and a test-only class code. It uses an in-memory database and a temporary upload directory. Stopping it removes the temporary uploads. Do not use this example for real student data.

Install Playwright 1.52.0 outside the repository (no frontend build dependencies are added), and use an installed Google Chrome:

```sh
browser_tools_dir=$(mktemp -d /tmp/zongce-browser.XXXXXX)
npm install --prefix "$browser_tools_dir" playwright@1.52.0 --no-audit --no-fund
ZONGCE_PLAYWRIGHT_MODULE="$browser_tools_dir/node_modules/playwright" node tests/student_browser.cjs http://127.0.0.1:PORT
```

Replace PORT with the preview's actual port. The test generates synthetic submissions and runs the submission, receipt, query, edit, attachment and declaration flows at 320x568, 390x844, 768x1024 and 1440x900. Desktop coverage includes all seven categories and conditional article/certificate branches. Screenshots are written to /tmp/zongce-student-screenshots; set ZONGCE_SCREENSHOTS to another output directory as needed.

The regression suite also checks repeated opening of the edit disclosure, preserved category values, unchanged attachment count on text-only edits, JavaScript errors, and horizontal overflow. Rust tests cover wrong credentials, CSRF, session scope, read-only review states, original-year validation, attachment ownership/count limits, database failure cleanup and a review that occurs after the student record was loaded.

The feedback regression adds the combined CET option and score, national awards without school-only fields, all six school-honor choices, and hidden/disabled fallback buttons when JavaScript runs. At each viewport a JavaScript-disabled context completes a declaration plus category/conditional refresh, CET submission, query, and edit refresh. Text survives refresh; uploaded files are chosen afterward. Route tests verify that refresh writes neither records nor files and still enforces CSRF, editable status, and credential version. Legacy CET scores and custom school-honor text retain editable values.

Docker/Compose verification is deferred to Task 10. Docker is not installed on the current development machine.
