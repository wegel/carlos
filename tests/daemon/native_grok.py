#!/usr/bin/env python3
"""Hold one real ACP prompt and reject a queued successor that arrives before cancellation."""

import json
import os
from pathlib import Path
import sys

ROOT = Path(os.environ["CARLOS_TEST_ROOT"])
SESSION = "grok-held"
held = None
cancelled = False


def send(message):
    with (ROOT / "rpc.jsonl").open("a") as log:
        log.write(json.dumps({"direction": "server", "message": message}) + "\n")
    print(json.dumps(message), flush=True)


def text(value):
    send({"method": "session/update", "params": {"sessionId": SESSION, "update": {
        "sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": value}}}})


for line in sys.stdin:
    request = json.loads(line)
    with (ROOT / "rpc.jsonl").open("a") as log:
        log.write(json.dumps({"direction": "client", "message": request}) + "\n")
    method = request["method"]
    result = {}
    if method == "session/new":
        result = {"sessionId": SESSION}
    elif method == "session/prompt":
        prompt = request["params"]["prompt"][0]["text"]
        if prompt == "first":
            held = request["id"]
            text("running first")
            continue
        assert cancelled, "queued prompt arrived before native cancel"
        text("second answer")
        result = {"stopReason": "end_turn"}
    elif method == "session/cancel":
        cancelled = True
        if held is not None:
            send({"id": held, "result": {"stopReason": "cancelled"}})
            held = None
    if "id" in request:
        send({"id": request["id"], "result": result})
