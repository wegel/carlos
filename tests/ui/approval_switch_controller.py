#!/usr/bin/env python3
"""Gate a real SDK fork while its old helper has an unanswered default-to-no callback."""

import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import threading
import time

child = subprocess.Popen([os.environ["CARLOS_REAL_CLAUDE_CONTROLLER"]], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
lock = threading.Lock()
gate = Path(os.environ["CARLOS_TEST_ROOT"]) / "allow-helper-switch"
replacement = gate.exists()


def output(frame):
    with lock:
        sys.stdout.write(frame)
        sys.stdout.flush()


def callback(command, title):
    output(json.dumps({"v": 1, "type": "callback", "kind": "permission", "callbackId": "reused-id",
                       "payload": {"toolName": "Bash", "title": title, "defaultToNo": True,
                                   "input": {"command": command}}}) + "\n")


def forward():
    for line in child.stdout:
        output(line.decode())
    os._exit(child.wait())


signal.signal(signal.SIGTERM, lambda *_: child.terminate())
threading.Thread(target=forward, daemon=True).start()
try:
    for line in sys.stdin:
        frame = json.loads(line)
        if frame.get("method") == "sessions.forkBefore":
            callback("echo original request", "Allow original request?")
            while not gate.exists():
                time.sleep(.01)
        if replacement and frame.get("method") == "start":
            callback("echo different request", "Allow different request?")
        if frame.get("method") == "callback.respond" and frame["params"]["callbackId"] == "reused-id":
            output(json.dumps({"v": 1, "type": "reply", "id": frame["id"], "ok": True, "result": {}}) + "\n")
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
