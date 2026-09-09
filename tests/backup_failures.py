#!/usr/bin/env python3
"""Exercise backup recovery with real files/SQLite and controlled external failures."""
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile

REPO = Path(__file__).resolve().parents[1]
ADAPTER = REPO / 'tests' / 'support' / 'backup_command.py'


def scenario(label, initial='running', fault=None):
    with tempfile.TemporaryDirectory(prefix='zongce-backup-fault-') as temporary:
        root = Path(temporary)
        data, uploads, backups, commands = (root / name for name in ('data', 'uploads', 'backups', 'commands'))
        for directory in (data, uploads, backups, commands):
            directory.mkdir()
        database = sqlite3.connect(data / 'app.db')
        database.execute('CREATE TABLE sample (value TEXT)')
        database.execute("INSERT INTO sample VALUES ('paired snapshot')")
        database.commit()
        database.close()
        (uploads / 'proof.pdf').write_bytes(b'%PDF-test')
        state = root / 'web-state'
        state.write_text(initial)
        # Only the Docker boundary and explicitly failing commands are replaced.
        for name in ('docker', 'sqlite3', 'touch', 'chmod', 'mktemp', 'mv', 'rm', 'rmdir'):
            (commands / name).symlink_to(ADAPTER)
        env = dict(os.environ, DATA_DIR=str(data), UPLOAD_DIR=str(uploads), BACKUP_DIR=str(backups),
                   TEST_WEB_STATE=str(state), TEST_ORIGINAL_PATH=os.environ['PATH'],
                   PATH=str(commands) + os.pathsep + os.environ['PATH'])
        if fault:
            env['TEST_FAIL_COMMAND'] = fault
        result = subprocess.run(['bash', str(REPO / 'scripts' / 'backup.sh')], env=env, capture_output=True, text=True)
        output = result.stdout + result.stderr
        assert str(root) not in output, f'{label}: raw path leaked in output'
        if (backups / 'backup.log').is_file():
            assert str(root) not in (backups / 'backup.log').read_text()
        if fault:
            assert result.returncode != 0, f'{label}: failed command reported success'
            assert 'event=backup_failed' in output, f'{label}: missing safe failure event'
            assert state.read_text() == initial, f'{label}: original service not restored'
            if fault == 'rmdir':
                assert 'stage=lock_cleanup' in output, f'{label}: cleanup stage missing'
                assert 'event=backup_complete' not in output, f'{label}: false success before cleanup'
            else:
                assert not (backups / '.backup-lock').exists(), f'{label}: lock leaked'
                assert not list(backups.glob('snapshot-*')), f'{label}: failed snapshot published'
                if fault == 'mv,rm':
                    assert 'stage=partial_cleanup' in output, f'{label}: partial cleanup stage missing'
        else:
            assert result.returncode == 0, f'{label}: backup did not stop an intended-running container'
            expected = 'running' if initial in ('running', 'restarting') else initial
            assert state.read_text() == expected, f'{label}: original service state not preserved'
            snapshots = list(backups.glob('snapshot-*'))
            assert len(snapshots) == 1
            restored = sqlite3.connect(snapshots[0] / 'app.db')
            assert restored.execute('SELECT value FROM sample').fetchone()[0] == 'paired snapshot'
            restored.close()
            assert (snapshots[0] / 'uploads' / 'proof.pdf').read_bytes() == b'%PDF-test'


cases = [('restarting service', 'restarting', None), ('stopped service', 'exited', None)] + [(f'{tool} failure', 'running', tool) for tool in ('touch', 'chmod', 'mktemp', 'mv', 'rmdir', 'mv,rm')]
failures = []
for label, initial, fault in cases:
    try:
        scenario(label, initial, fault)
        print(label + ': passed')
    except AssertionError as error:
        failures.append(str(error))
for failure in failures:
    print(failure)
assert not failures, f'{len(failures)} backup regressions failed'
