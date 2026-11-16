"""Ask for an actual ACP permission and record its exact answer without inference."""

import json
import os
from pathlib import Path
import sys

OPTIONS = [
    {"optionId":"native:once:α", "kind":"allow_once", "name":"Allow this command once"},
    {"optionId":"native:always:β", "kind":"allow_always", "name":"Allow this command in future sessions"},
    {"optionId":"native:no:γ", "kind":"reject_once", "name":"Reject this command once"},
    {"optionId":"native:never:δ", "kind":"reject_always", "name":"Reject this command in future sessions"},
]


def main():
    import select
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "ui"))
    import fake_grok as peer
    mode = os.environ["CARLOS_TEST_SERVER"]
    native_id = "91001" if mode.endswith("string") else 91001
    prompt, opening, completed = None, None, False
    control, buffer = None, b""

    def permission(title="Run the checked fixture command"):
        options = OPTIONS + OPTIONS[:1] if mode.endswith("malformed") else OPTIONS
        peer.send({"jsonrpc":"2.0", "id":native_id, "method":"session/request_permission", "params":{
            "sessionId":peer.SESSION,
            "toolCall":{"toolCallId":"permission-tool", "title":title,
                        "kind":"execute", "rawInput":{"command":"printf permission_fixture"}},
            "options":options,
        }})

    while True:
        step = peer.ROOT / "permission-step"
        action = step.read_text() if step.exists() else None
        if action and action != control:
            control = action
            if action == "replace":
                permission("A different command reused this native id")
            elif action == "close":
                return
            elif action == "late":
                permission("A late permission after interruption")
        if not select.select([sys.stdin], [], [], .01)[0]:
            continue
        chunk = os.read(sys.stdin.fileno(), 65536)
        if not chunk:
            return
        buffer += chunk
        while b"\n" in buffer:
            line, buffer = buffer.split(b"\n", 1)
            request = json.loads(line)
            peer.record("client", request)
            method = request.get("method")
            if method is None:
                with (peer.ROOT / "permission-answers.jsonl").open("a") as log:
                    log.write(json.dumps(request) + "\n")
                if opening is not None:
                    peer.send({"id":opening, "result":{"sessionId":peer.SESSION}})
                    opening = None
                elif prompt is not None and not completed:
                    peer.chunk("Permission fixture finished")
                    peer.send({"id":prompt, "result":{"stopReason":"end_turn"}})
                    completed = True
            elif method in ("session/new", "session/load") and mode.endswith("startup"):
                opening = request["id"]
                permission()
            elif method == "session/prompt":
                prompt, completed = request["id"], False
                # Give interrupt a concrete native turn even before a tool produces output.
                peer.chunk("Permission fixture waiting")
                permission()
                if mode.endswith("broken-input"):
                    import time
                    os.close(sys.stdin.fileno())
                    (peer.ROOT / "permission-input-closed").write_text("closed")
                    # Keep stdout and the card live until Carlos attempts the broken cancel.
                    while True:
                        time.sleep(.01)
            elif method == "session/cancel":
                if prompt is not None and not completed:
                    peer.send({"id":prompt, "result":{"stopReason":"cancelled"}})
                    completed = True
            else:
                peer.handle(request)


if __name__ == "__main__":
    main()
