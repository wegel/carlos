#!/usr/bin/env python3
"""Override saved catalogue replies while reusing the ordinary deterministic ACP peer."""

import json
from pathlib import Path
import sys

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
        peer.chunk("History complete\n")
        peer.send({"id":request["id"], "result":{"sessionId":peer.SESSION}})
    else:
        peer.handle(request)


for line in sys.stdin:
    request = json.loads(line)
    peer.record("client", request)
    handle(request)
