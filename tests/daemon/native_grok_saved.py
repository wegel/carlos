#!/usr/bin/env python3
"""Override saved catalogue replies while reusing the ordinary deterministic ACP peer."""

import json
from pathlib import Path
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "ui"))
import fake_grok as peer


def handle(request):
    config = json.loads((peer.ROOT / "grok-saved.json").read_text())
    method = request["method"]
    if method == "session/list":
        if config.get("error"):
            peer.send({"id":request["id"], "error":{"code":-32601, "message":"fixture saved catalogue unavailable"}})
        else:
            peer.send({"id":request["id"], "result":{"sessions":config.get("rows", [])}})
    elif method == "session/load":
        peer.SESSION = request["params"]["sessionId"]
        count = config.get("replay_turns", 0)
        for index in range(count):
            if index == config.get("hold_after"):
                (peer.ROOT / "replay-held").touch()
                deadline = time.monotonic() + 15
                while not (peer.ROOT / "release-replay").exists() and time.monotonic() < deadline:
                    time.sleep(.01)
                assert (peer.ROOT / "release-replay").exists(), "test did not release native replay"
            peer.send({"method":"session/update", "params":{"sessionId":peer.SESSION, "update":{
                "sessionUpdate":"user_message_chunk", "content":{"type":"text", "text":f"Grok user {index} 🌍"}}}})
            peer.chunk(f"Grok answer {index} 🌍")
        if not count:
            peer.chunk("History complete\n")
        peer.send({"id":request["id"], "result":{"sessionId":peer.SESSION}})
    else:
        peer.handle(request)


for line in sys.stdin:
    request = json.loads(line)
    peer.record("client", request)
    handle(request)
