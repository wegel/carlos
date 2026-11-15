"""Check only descendants captured from a test-owned daemon, including PID reuse guards."""

import os
from pathlib import Path
import signal
import subprocess
import time


def processes():
    if Path("/proc/self/stat").exists():
        records = {}
        for item in Path("/proc").iterdir():
            if not item.name.isdigit():
                continue
            try:
                fields = (item / "stat").read_text().rsplit(") ", 1)[1].split()
                records[int(item.name)] = (int(fields[1]), fields[19], fields[0])
            except (OSError, IndexError, ValueError):
                pass
        return records
    rows = subprocess.check_output(["ps", "-axo", "pid=,ppid=,stat=,lstart="], text=True)
    return {int(fields[0]): (int(fields[1]), " ".join(fields[3:]), fields[2])
            for row in rows.splitlines() if len(fields := row.split()) >= 8}


def capture_children(owner):
    records = processes()
    owned = {}
    parents = {owner}
    while parents:
        children = {pid for pid, (parent, _, _) in records.items() if parent in parents}
        for pid in children:
            owned[pid] = records[pid][1]
        parents = children
    return owned


def remaining(owned):
    current = processes()
    return {pid: started for pid, started in owned.items()
            if pid in current and current[pid][1] == started}


def wait_children(owned, seconds):
    deadline = time.monotonic() + seconds
    while live := remaining(owned):
        if time.monotonic() >= deadline:
            return live
        time.sleep(.01)
    return {}


def finish_children(owned, errors):
    live = wait_children(owned, 2)
    if not live:
        return
    errors.append(f"test-owned daemon left descendants: {sorted(live)}")
    for action, seconds in ((signal.SIGTERM, .5), (signal.SIGKILL, 1)):
        for pid in remaining(live):
            try:
                os.kill(pid, action)
            except ProcessLookupError:
                pass
        live = wait_children(live, seconds)
        if not live:
            return
    errors.append(f"test-owned descendants were not reaped: {sorted(live)}")
