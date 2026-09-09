#!/usr/bin/env python3
"""Fault-injection boundary for backup tests; ordinary file/SQLite tools remain real."""
import os
from pathlib import Path
import shutil
import sys

command = Path(sys.argv[0]).name
args = sys.argv[1:]
state_path = Path(os.environ['TEST_WEB_STATE'])
state = state_path.read_text().strip()
if command == 'docker':
    if args[0] == 'inspect':
        print(state)
    elif 'ps' in args:
        if '--status' in args:  # Existing implementation only sees running.
            if state == 'running':
                print('web')
        else:
            print('a' * 64)
    elif 'stop' in args:
        state_path.write_text('exited')
    elif 'start' in args:
        state_path.write_text('running')
    else:
        sys.exit(2)
    sys.exit(0)

if command == 'sqlite3' and state in ('running', 'restarting'):
    print('writer still active during snapshot', file=sys.stderr)
    sys.exit(3)
if command in os.environ.get('TEST_FAIL_COMMAND', '').split(','):
    print('injected tool failure: ' + os.environ['BACKUP_DIR'], file=sys.stderr)
    sys.exit(7)
original = shutil.which(command, path=os.environ['TEST_ORIGINAL_PATH'])
os.execv(original, [original] + args)
