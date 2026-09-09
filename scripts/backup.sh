#!/usr/bin/env bash
# Daily cold snapshot: pause all application writes so DB metadata and uploads
# refer to the same point in time. SQLite .backup also includes committed WAL.
set -Eeuo pipefail
umask 077

DATA_DIR=${DATA_DIR:-/opt/zongce/data}
UPLOAD_DIR=${UPLOAD_DIR:-/opt/zongce/uploads}
BACKUP_DIR=${BACKUP_DIR:-/opt/zongce/backups}
BACKUP_KEEP=${BACKUP_KEEP:-7}
PROJECT_DIR=${PROJECT_DIR:-/opt/zongce/app}
offline=false
if [[ ${1:-} == --offline && $# == 1 ]]; then
    offline=true
elif [[ $# != 0 ]]; then
    echo 'event=backup_failed stage=arguments' >&2
    exit 1
fi
stage=configuration
partial=
locked=false
restart_required=false
log_ready=false

log() {
    local entry
    entry="$(date -u +%Y-%m-%dT%H:%M:%SZ) $*"
    echo "$entry" >&2
    if $log_ready; then echo "$entry" >> "$BACKUP_DIR/backup.log"; fi
}

compose() { docker compose --project-directory "$PROJECT_DIR" -f "$PROJECT_DIR/docker-compose.yml" "$@"; }

finish() {
    local result=$?
    trap - EXIT INT TERM
    set +e
    if $restart_required; then
        if ! compose start web >/dev/null 2>&1; then
            result=1
            log 'event=backup_failed stage=service_restart'
        fi
    fi
    # Only the mktemp directory created by this invocation can be cleaned up.
    if [[ -n $partial && $partial == "$BACKUP_DIR"/.partial.* && -d $partial ]]; then rm -rf -- "$partial"; fi
    if $locked; then rmdir "$BACKUP_DIR/.backup-lock"; fi
    if [[ $result != 0 ]]; then log "event=backup_failed stage=$stage exit_code=$result"; fi
    exit "$result"
}
trap finish EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# Absolute, distinct, non-root paths keep cleanup constrained and SQL CLI quoting
# unambiguous. Resolve parent links before comparing containment relationships.
for directory in "$DATA_DIR" "$UPLOAD_DIR" "$BACKUP_DIR"; do
    if [[ $directory != /* || $directory == / || $directory == *[\'\"$'\n'$'\r']* || -L $directory ]]; then exit 1; fi
done
mkdir -p "$BACKUP_DIR" 2>/dev/null
BACKUP_DIR=$(cd "$BACKUP_DIR" && pwd -P)
[[ ! -L $BACKUP_DIR/backup.log ]] || exit 1
touch "$BACKUP_DIR/backup.log"
chmod 600 "$BACKUP_DIR/backup.log"
log_ready=true
[[ $BACKUP_KEEP =~ ^[0-9]+$ && ${#BACKUP_KEEP} -le 6 ]] || exit 1
BACKUP_KEEP=$((10#$BACKUP_KEEP))
[[ $BACKUP_KEEP -ge 7 ]] || exit 1
stage=source_validation
[[ -f $DATA_DIR/app.db && ! -L $DATA_DIR/app.db && -d $UPLOAD_DIR ]] || exit 1
DATA_DIR=$(cd "$DATA_DIR" && pwd -P)
UPLOAD_DIR=$(cd "$UPLOAD_DIR" && pwd -P)
for directory in "$DATA_DIR" "$UPLOAD_DIR" "$BACKUP_DIR"; do
    [[ $directory != *[\'\"$'\n'$'\r']* ]] || exit 1
done
for source in "$DATA_DIR" "$UPLOAD_DIR"; do
    [[ $BACKUP_DIR != "$source" && $BACKUP_DIR != "$source"/* && $source != "$BACKUP_DIR"/* ]] || exit 1
done
[[ $DATA_DIR != "$UPLOAD_DIR" && $DATA_DIR != "$UPLOAD_DIR"/* && $UPLOAD_DIR != "$DATA_DIR"/* ]] || exit 1
command -v sqlite3 >/dev/null 2>&1
stage=lock
mkdir "$BACKUP_DIR/.backup-lock" 2>/dev/null
locked=true
if ! $offline; then
    stage=service_status
    running=$(compose ps --status running --services 2>/dev/null)
    if [[ $'\n'$running$'\n' == *$'\nweb\n'* ]]; then
        # Set before stop so even a partially failed stop tries to restore service.
        restart_required=true
        stage=service_stop
        compose stop web >/dev/null 2>&1
    fi
fi
stage=snapshot
partial=$(mktemp -d "$BACKUP_DIR/.partial.XXXXXXXX")
sqlite3 "$DATA_DIR/app.db" ".timeout 5000" ".backup '$partial/app.db'" >/dev/null 2>&1
[[ $(sqlite3 "$partial/app.db" 'PRAGMA integrity_check;' 2>/dev/null) == ok ]]
cp -Rp "$UPLOAD_DIR" "$partial/uploads" 2>/dev/null
printf 'zongce-backup-v1\n' > "$partial/.complete"
snapshot="$BACKUP_DIR/snapshot-$(date -u +%Y%m%dT%H%M%SZ)-${partial##*.}"
mv "$partial" "$snapshot"
partial=
if $restart_required; then
    stage=service_restart
    compose start web >/dev/null 2>&1
    restart_required=false
fi
stage=retention
# Snapshot basenames are generated above. User folders without our completion
# marker never participate; failed partial snapshots never cause pruning.
count=0
while IFS= read -r candidate; do
    [[ -d $candidate && ! -L $candidate && -f $candidate/.complete ]] || continue
    [[ $(< "$candidate/.complete") == zongce-backup-v1 ]] || continue
    count=$((count + 1))
    if [[ $count -gt $BACKUP_KEEP && $candidate == "$BACKUP_DIR"/snapshot-* ]]; then
        rm -rf -- "$candidate"
    fi
done < <(ls -1dt "$BACKUP_DIR"/snapshot-*)
log 'event=backup_complete status=ok'
