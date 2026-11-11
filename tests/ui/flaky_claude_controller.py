#!/usr/bin/env python3
"""Reject one callback reply without settling the real SDK callback."""

import json
import os
import signal
import subprocess
import sys
import threading

child = subprocess.Popen([os.environ["CARLOS_REAL_CLAUDE_CONTROLLER"]], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
lock = threading.Lock()
failed = False


def output(frame):
    with lock:
        sys.stdout.write(frame)
        sys.stdout.flush()


def forward():
    for line in child.stdout:
        output(line.decode())
    os._exit(child.wait())


signal.signal(signal.SIGTERM, lambda *_: child.terminate())
threading.Thread(target=forward, daemon=True).start()
try:
    for line in sys.stdin:
        frame = json.loads(line)
        if frame.get("method") == "callback.respond" and not failed:
            failed = True
            output(json.dumps({"v": 1, "type": "reply", "id": frame["id"], "ok": False,
                               "error": {"code": "fixture_reply_failure", "message": "Try this answer again"}}) + "\n")
        else:
            child.stdin.write(line.encode())
            child.stdin.flush()
finally:
    child.stdin.close()
    try:
        child.wait(timeout=3)
    except subprocess.TimeoutExpired:
        child.kill()
        child.wait()
