#!/usr/bin/env python3
"""Claude stream peer that exits on interrupt and needs no account or model."""

import json
import os
from pathlib import Path
import signal
import sys


ROOT = Path(os.environ["CARLOS_TEST_ROOT"])


def record(direction, message):
    with (ROOT / "rpc.jsonl").open("a") as log:
        log.write(json.dumps({"direction": direction, "message": message}) + "\n")


def send(message):
    record("server", message)
    print(json.dumps(message), flush=True)


def interrupt(_number, _frame):
    record("client", {"signal": "SIGINT", "pid": os.getpid()})
    raise SystemExit(0)


def event(kind, **fields):
    send({"type": "stream_event", "event": {"type": kind, **fields}})


def respond(text):
    event("message_start", message={"id": "message-test", "role": "assistant", "content": []})
    event("content_block_start", index=0, content_block={"type": "text", "text": ""})
    event("content_block_delta", index=0, delta={"type": "text_delta", "text": "Answer: " + text})
    if text == "hold this turn":
        return
    event("content_block_stop", index=0)
    event("message_stop")
    send({"type": "result", "subtype": "success", "session_id": "claude-test-session"})


if __name__ == "__main__":
    signal.signal(signal.SIGINT, interrupt)
    send({"type": "system", "subtype": "init", "session_id": "claude-test-session"})
    for line in sys.stdin:
        request = json.loads(line)
        record("client", request)
        respond(request["message"]["content"][0]["text"])
