#!/usr/bin/env python3
"""Add held and refused settings replies to the existing native Claude fixture."""

import json
from pathlib import Path
import select
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "ui"))
import fake_claude as peer

held = []


def settings_config():
    path = peer.ROOT / "settings-config.json"
    return json.loads(path.read_text()) if path.exists() else {}


def control(request):
    subtype = request["request"]["subtype"]
    config = settings_config()
    if subtype in config.get("close", []):
        sys.exit(0)
    if subtype in config.get("hold", []):
        if config.get("approval_on_hold"):
            peer.send({"type":"control_request", "request_id":"settings-approval", "request":{
                "subtype":"can_use_tool", "tool_name":"Bash", "tool_use_id":"settings-tool",
                "input":{"command":"echo settings"}, "title":"Allow settings fixture?"}})
        held.append(request)
        (peer.ROOT / "settings-held").write_text(subtype)
        return
    if subtype in config.get("reject", []):
        peer.send({"type":"control_response", "response":{
            "subtype":"error", "request_id":request["request_id"], "error":"PRIVATE_SETTINGS_REJECTION"}})
        return
    if subtype == "set_model" and config.get("reset_on_model"):
        peer.send({"type":"conversation_reset", "session_id":peer.SESSION,
                   "uuid":str(peer.uuid.uuid4()), "new_conversation_id":str(peer.uuid.uuid4())})
        peer.send({"type":"system", "subtype":"init", "session_id":peer.SESSION, "model":"sdk-sonnet"})
    peer.handle_control(request)


peer.record("client", {"event":"launch", "pid":peer.os.getpid(), "argv":sys.argv[1:]})
while True:
    if held and (peer.ROOT / "release-settings").exists():
        config = settings_config()
        config["hold"] = []
        (peer.ROOT / "settings-config.json").write_text(json.dumps(config))
        for request in held:
            control(request)
        held.clear()
    if not select.select([sys.stdin], [], [], .01)[0]:
        continue
    line = sys.stdin.readline()
    if not line:
        break
    request = json.loads(line)
    peer.record("client", request)
    if request["type"] == "control_request":
        control(request)
    elif request["type"] == "control_response":
        if peer.activity.response(request):
            continue
        if peer.questions.response(request):
            peer.active = None
    elif request["type"] == "user":
        peer.handle_prompt(request)
