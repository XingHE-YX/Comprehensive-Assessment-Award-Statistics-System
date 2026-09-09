#!/usr/bin/env bash
# Read-only startup diagnostic for the exact application binary. All credentials
# are ephemeral build fixtures; no production environment or public route needed.
set -Eeuo pipefail
umask 077
binary=${1:?pass an absolute application binary path}
expected=${2:?pass the required SQLite version}
[[ $binary == /* && $expected =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]
check_dir=$(mktemp -d)
process_id=
cleanup() {
    local result=$?
    trap - EXIT
    if [[ -n $process_id ]]; then kill -TERM "$process_id" 2>/dev/null || true; wait "$process_id" || true; fi
    # Remove only the private mktemp directory made by this invocation.
    rm -rf -- "$check_dir"
    exit "$result"
}
trap cleanup EXIT
fixture_secret=$(head -c 32 /dev/urandom | base64 | tr -d '\n')
fixture_hash=$(printf '%s\n' "$fixture_secret" | "$binary" --hash-secret)
cd "$check_dir"
env -i APP_ENV=development BIND_ADDR=127.0.0.1:0 ADMIN_USERNAME=build-check \
    ADMIN_PASSWORD_HASH="$fixture_hash" SESSION_SECRET="$fixture_secret" \
    DATABASE_URL="sqlite:$check_dir/app.db" UPLOAD_DIR="$check_dir/uploads" RUST_LOG=info \
    "$binary" > "$check_dir/startup.log" 2>&1 &
process_id=$!
ready=false
for _ in {1..100}; do
    if grep -F '"event":"startup"' "$check_dir/startup.log" >/dev/null; then ready=true; break; fi
    kill -0 "$process_id" 2>/dev/null || break
    sleep 0.1
done
if ! $ready || ! grep -F '"event":"database_ready"' "$check_dir/startup.log" | grep -F "\"sqlite_version\":\"$expected\"" >/dev/null; then
    echo 'SQLite engine gate failed: SQLx application version does not match the required baseline' >&2
    exit 1
fi
kill -TERM "$process_id"
wait "$process_id"
process_id=
echo "SQLx application SQLite version verified: $expected"
