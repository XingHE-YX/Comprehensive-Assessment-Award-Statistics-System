#!/usr/bin/env bash
# Writes synthetic acceptance records. Run only against an isolated acceptance
# deployment with an open year and its class code; credentials stay in env/stdin.
set -Eeuo pipefail
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
exec python3 "$script_dir/support/smoke.py"
