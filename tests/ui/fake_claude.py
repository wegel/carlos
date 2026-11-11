#!/usr/bin/env python3
"""Deterministic native CLI peer for the official SDK, without an account or model."""

import json
import os
from pathlib import Path
import sys
import time
import uuid

import fake_claude_questions as questions
from fake_claude_activity import Activity
from fake_claude_sessions import Sessions

ROOT = Path(os.environ["CARLOS_TEST_ROOT"])
MODE = os.environ.get("CARLOS_TEST_SERVER", "modern")
sessions = Sessions(sys.argv[1:], Path.cwd())
SESSION = sessions.session_id
active = None
initialize = None
prompts = 0
model = "sdk-sonnet"
batches = {}


def record(direction, message):
    with (ROOT / "rpc.jsonl").open("a") as log:
        log.write(json.dumps({"direction": direction, "message": message}) + "\n")


def send(message):
    print(json.dumps(message), flush=True)
    record("server", message)


def control_reply(request, result):
    send({"type": "control_response", "response": {"subtype": "success",
          "request_id": request["request_id"], "response": result}})


def state(value):
    send({"type": "system", "subtype": "session_state_changed", "state": value,
          "session_id": SESSION, "uuid": str(uuid.uuid4())})


def event(kind, prompt, **fields):
    send({"type": "stream_event", "session_id": SESSION, "parent_tool_use_id": None,
          **({"user_message_uuid": prompt, "user_message_uuids": batches.get(prompt, [prompt])}
             if kind == "message_start" else {}), "event": {"type": kind, **fields}})


def result(prompt, failed=False, interrupted=False, queued=0):
    sessions.result(prompt)
    send({"type": "result", "subtype": "error_during_execution" if failed else "success",
          "is_error": failed, "session_id": SESSION, "uuid": str(uuid.uuid4()),
          "user_message_uuid": prompt, "user_message_uuids": batches.get(prompt, [prompt]),
          "errors": ["Fixture model failure"] if failed else [], "result": "Fixture model failure" if failed else "Done",
          "duration_ms": 1, "duration_api_ms": 0, "num_turns": 1, "total_cost_usd": 0,
          "usage": {}, "modelUsage": {}, "permission_denials": [], "queued_turn_count": queued,
          **({"terminal_reason": "aborted_tools" if MODE == "claude-interrupt-before-ack" else "aborted_streaming"} if interrupted else {})})


def finish_initialize(request):
    control_reply(request, {"commands": [{"name": "context", "description": "Native context"}],
        "models": [{"value": "sdk-sonnet", "displayName": "SDK Sonnet", "description": "fixture",
                    "supportsEffort": True, "supportedEffortLevels": ["low", "medium", "high"]},
                   {"value": "sdk-opus", "displayName": "SDK Opus", "description": "fixture",
                    "supportsEffort": True, "supportedEffortLevels": ["low", "medium", "high", "max"]}],
        "agents": [], "account": {}, "output_style": "default"})
    send({"type": "system", "subtype": "init", "session_id": SESSION, "model": model})


def handle_control(message):
    global initialize, active, model
    request = message["request"]
    subtype = request["subtype"]
    if activity.control(message):
        return
    questions.control(subtype)
    if subtype == "initialize":
        if MODE == "claude-startup-closed":
            time.sleep(1)
            sys.exit(3)
        if MODE == "claude-fork-start-failure":
            marker = ROOT / "first-native-session"
            if marker.exists() and marker.read_text() != SESSION:
                sys.exit(3)
            marker.write_text(SESSION)
        if MODE in ("claude-startup-question", "claude-startup-confirm"):
            initialize = message
            send({"type": "control_request", "request_id": "startup-question", "request": {
                "subtype": "can_use_tool", "tool_name": "Bash", "input": {"command": "echo startup"},
                "tool_use_id": "startup-tool", "title": "Allow startup hook?",
                "default_to_no": MODE == "claude-startup-confirm"}})
        else:
            finish_initialize(message)
    elif subtype == "get_context_usage":
        control_reply(message, {"totalTokens": 32123, "maxTokens": 123456, "rawMaxTokens": 123456,
                      "percentage": 26, "categories": [], "gridRows": [], "source": "native-fixture"})
    else:
        if subtype == "set_model":
            model = request.get("model") or "sdk-sonnet"
        if subtype == "interrupt" and active and MODE == "claude-interrupt-before-ack":
            event("content_block_stop", active, index=0)
            result(active, interrupted=True)
            active = None
            state("idle")
            time.sleep(.05)
        if subtype == "interrupt" and MODE == "claude-interrupt-error":
            send({"type": "control_response", "response": {"subtype": "error", "request_id": message["request_id"], "error": "Fixture interrupt failure"}})
        else:
            control_reply(message, {"still_queued": []} if subtype == "interrupt" else {})
        if subtype == "interrupt" and MODE == "claude-interrupt-after-ack":
            time.sleep(.05)
        if subtype == "interrupt" and active:
            event("content_block_stop", active, index=0)
            result(active, interrupted=MODE != "claude-interrupt-error")
            active = None
            state("idle")


def read_tool(prompt):
    send({"type": "assistant", "session_id": SESSION, "user_message_uuid": prompt,
          "message": {"id": "claude-tool", "model": model, "role": "assistant", "content": [
              {"type": "tool_use", "id": "claude-read", "name": "Read", "input": {"file_path": "source.rs"}}]}})
    send({"type": "user", "session_id": SESSION, "message": {"role": "user", "content": [{
        "type": "tool_result", "tool_use_id": "claude-read", "is_error": False,
        "content": "\n".join(f"let claude_read_{i} = {i};" for i in range(8))}]}})


def handle_prompt(message):
    global active, prompts
    if activity.prompt(message):
        return
    text = message["message"]["content"]
    if isinstance(text, list):
        text = text[0]["text"]
    prompt = message["uuid"]
    prompts += 1
    sessions.prompt(message)
    if active:
        event("content_block_stop", active, index=0)
        if text == "merge this":
            batches[prompt] = [active, prompt]
        else:
            result(active, queued=1)
    active = prompt
    state("running")
    if questions.start(text, prompt, send, result, state, SESSION):
        return
    if text == "read source":
        read_tool(prompt)
    event("message_start", prompt, message={"id": "message-" + prompt, "role": "assistant", "model": model, "content": []})
    event("content_block_start", prompt, index=0, content_block={"type": "text", "text": ""})
    answer = "Answer: " + text
    if MODE == "claude-ralph-followup" and prompts > 1:
        answer = "@@AWAITING_HUMAN@@"
    event("content_block_delta", prompt, index=0, delta={"type": "text_delta", "text": answer})
    if text == "close helper":
        sys.exit(0)
    if text == "hold this turn":
        return
    event("content_block_stop", prompt, index=0)
    event("message_stop", prompt)
    if text == "steer this":
        state("idle")  # Check the inverse result/idle order as well.
    result(prompt, failed=text == "fail this turn" or MODE == "claude-ralph-failure")
    active = None
    if text != "steer this":
        state("idle")


activity = Activity(sys.modules[__name__])

if __name__ == "__main__":
    record("client", {"event": "launch", "pid": os.getpid(), "argv": sys.argv[1:]})
    for line in sys.stdin:
        request = json.loads(line)
        record("client", request)
        if request["type"] == "control_request":
            handle_control(request)
        elif request["type"] == "control_response" and initialize:
            finish_initialize(initialize)
            initialize = None
        elif request["type"] == "control_response":
            if activity.response(request):
                continue
            if questions.response(request):
                active = None
        elif request["type"] == "user":
            handle_prompt(request)
