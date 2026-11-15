#!/usr/bin/env python3
"""A deterministic ACP peer that gates a large restore before accepting human text."""

import json
import os
from pathlib import Path
import sys
import time

ROOT = Path(os.environ["CARLOS_TEST_ROOT"])
SESSION = "grok-saved"


def record(direction, message):
    with (ROOT / "rpc.jsonl").open("a") as log:
        log.write(json.dumps({"direction": direction, "message": message}) + "\n")


def send(message):
    record("server", message)
    print(json.dumps(message), flush=True)


def chunk(text):
    send({"method": "session/update", "params": {"sessionId": SESSION,
          "update": {"sessionUpdate": "agent_message_chunk",
                     "content": {"type": "text", "text": text}}}})


def handle(request):
    method = request["method"]
    result = {}
    if method == "session/load":
        while not (ROOT / "release-history").exists():
            time.sleep(0.01)
        for _ in range(6000):
            chunk("x")
        chunk("\nHistory complete\n")
        result = {"sessionId": SESSION}
    elif method == "session/new":
        result = {"sessionId": SESSION}
    elif method == "session/prompt":
        chunk("Answer: " + request["params"]["prompt"][0]["text"])
        result = {"stopReason": "end_turn"}
    elif method == "session/list":
        result = {"sessions": []}
    if "id" in request:
        send({"jsonrpc": "2.0", "id": request["id"], "result": result})


def main():
    for line in sys.stdin:
        request = json.loads(line)
        record("client", request)
        handle(request)


if __name__ == "__main__":
    main()
