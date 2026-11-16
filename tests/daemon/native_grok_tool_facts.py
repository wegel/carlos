"""Report real ACP tool kinds and outcomes while keeping hidden read bodies hidden."""

import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "ui"))
import fake_grok as peer

OUTPUT = "done 🌍\n"


def update(**fields):
    peer.send({"method":"session/update", "params":{"sessionId":peer.SESSION, "update":fields}})


for line in sys.stdin:
    request = json.loads(line)
    peer.record("client", request)
    if request["method"] != "session/prompt":
        peer.handle(request)
        continue
    for identity, kind, status in (("read", "read", "completed"), ("command", "execute", "completed"), ("search", "search", "failed")):
        update(sessionUpdate="tool_call", toolCallId=identity, title=f"Native {identity} 🌍", kind=kind, status="pending", rawInput={"path":"é.rs"})
        update(sessionUpdate="tool_call_update", toolCallId=identity, status=status, rawOutput={"output":OUTPUT})
    peer.send({"jsonrpc":"2.0", "id":request["id"], "result":{"stopReason":"end_turn"}})
