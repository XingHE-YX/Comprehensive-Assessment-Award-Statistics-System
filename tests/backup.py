#!/usr/bin/env python3
"""Run the real backup script against an isolated WAL database; no Docker needed."""
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import tempfile

SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "backup.sh"

with tempfile.TemporaryDirectory(prefix="zongce-backup-test-") as temporary:
    root = Path(temporary)
    data, uploads, backups = (root / name for name in ("data", "uploads", "backups"))
    data.mkdir()
    uploads.mkdir()
    evidence = uploads / "year-1" / "ZC2026-000001" / "evidence.pdf"
    evidence.parent.mkdir(parents=True)
    evidence.write_bytes(b"%PDF-1.4 backup-test")
    db = sqlite3.connect(data / "app.db")
    db.execute("PRAGMA journal_mode=WAL")
    db.execute("PRAGMA wal_autocheckpoint=0")
    db.execute("CREATE TABLE evidence (id INTEGER PRIMARY KEY, name TEXT)")
    db.execute("INSERT INTO evidence VALUES (1, '恢复样例')")
    db.commit()
    env = dict(os.environ, DATA_DIR=str(data), UPLOAD_DIR=str(uploads), BACKUP_DIR=str(backups))

    def run(ok=True, **overrides):
        result = subprocess.run(["bash", str(SCRIPT), "--offline"], env=dict(env, **overrides), capture_output=True, text=True)
        assert (result.returncode == 0) == ok, result.stderr
        return result

    for _ in range(9):
        run()
    snapshots = sorted(backups.glob("snapshot-*"))
    assert len(snapshots) == 7, len(snapshots)
    for snapshot in snapshots:
        assert (snapshot / ".complete").is_file()
        assert (snapshot.stat().st_mode & 0o077) == 0
        backup = sqlite3.connect(snapshot / "app.db")
        assert backup.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
        assert backup.execute("SELECT name FROM evidence").fetchone()[0] == "恢复样例"
        backup.close()
        assert (snapshot / "uploads" / evidence.relative_to(uploads)).read_bytes() == evidence.read_bytes()
    restored = root / "restored"
    shutil.copytree(snapshots[-1], restored)
    restored_db = sqlite3.connect(restored / "app.db")
    assert restored_db.execute("SELECT COUNT(*) FROM evidence").fetchone()[0] == 1
    restored_db.close()
    assert (restored / "uploads" / evidence.relative_to(uploads)).read_bytes() == evidence.read_bytes()
    # Failed attempts cannot rotate completed backups or leave published partials.
    before = set(backups.glob("snapshot-*"))
    run(False, UPLOAD_DIR=str(root / "missing"))
    corrupt = root / "corrupt"
    corrupt.mkdir()
    (corrupt / "app.db").write_text("not a database")
    run(False, DATA_DIR=str(corrupt))
    run(False, BACKUP_KEEP="6")
    assert set(backups.glob("snapshot-*")) == before
    assert not list(backups.glob(".partial.*"))
    assert not (backups / ".backup-lock").exists()
    assert "event=backup_failed" in (backups / "backup.log").read_text()
    assert str(root) not in (backups / "backup.log").read_text()
    db.close()
print("backup: WAL snapshot, attachment restore, seven-copy retention and failures passed")
