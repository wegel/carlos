#!/usr/bin/env python3
"""Hold only an older SDK read reply while real helper events and controls keep flowing."""

import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import threading
import time

root = Path(os.environ["CARLOS_TEST_ROOT"])
child = subprocess.Popen([os.environ["CARLOS_REAL_CLAUDE_CONTROLLER"]], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
lock = threading.Lock()
targets = set()
held = []
started = False


def output(line):
    with lock:
        sys.stdout.write(line)
        sys.stdout.flush()


def forward():
    for raw in child.stdout:
        line = raw.decode()
        frame = json.loads(line)
        with lock:
            pause = frame.get("id") in targets and not (root / "release-history").exists()
            if pause:
                held.append(line)
                (root / "history-held").write_text(json.dumps(frame))
        if not pause:
            output(line)
    os._exit(child.wait())


def release():
    while child.poll() is None:
        if (root / "release-history").exists():
            with lock:
                ready, held[:] = list(held), []
            for line in ready:
                if (root / "fail-history").exists():
                    frame = json.loads(line)
                    line = json.dumps({"v":1, "type":"reply", "id":frame["id"], "ok":False,
                        "error":{"code":"stale_snapshot", "message":"fixture SDK snapshot expired"}}) + "\n"
                output(line)
        time.sleep(.01)


signal.signal(signal.SIGTERM, lambda *_: child.terminate())
threading.Thread(target=forward, daemon=True).start()
threading.Thread(target=release, daemon=True).start()
try:
    for line in sys.stdin:
        frame = json.loads(line)
        started |= frame.get("method") == "start"
        if started and frame.get("method") == "sessions.read" and "endOffset" in frame.get("params", {}):
            with lock:
                targets.add(frame["id"])
        child.stdin.write(line.encode())
        child.stdin.flush()
finally:
    child.stdin.close()
    try:
        child.wait(timeout=3)
    except subprocess.TimeoutExpired:
        child.kill()
        child.wait()
