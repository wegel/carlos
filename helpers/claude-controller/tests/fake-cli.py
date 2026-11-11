#!/usr/bin/env python3
"""Deterministic CLI peer for the real Agent SDK; never calls a model or reads credentials."""
import json
import os
import sys

SESSION = "11111111-1111-4111-8111-111111111111"
SUPPORTED_DIALOG_KINDS = []
HANG_USAGE = False


def emit(message):
    print(json.dumps(message), flush=True)


def control_response(request, response):
    emit({"type": "control_response", "response": {
        "subtype": "success", "request_id": request["request_id"], "response": response,
    }})


def callback(request_id, request):
    emit({"type": "control_request", "request_id": request_id, "request": request})


def handle_control(message):
    global SUPPORTED_DIALOG_KINDS
    request = message["request"]
    subtype = request["subtype"]
    response = {}
    if subtype == "initialize":
        SUPPORTED_DIALOG_KINDS = request.get("supportedDialogKinds", [])
        if "--resume=00000000-0000-4000-8000-000000000000" in sys.argv:
            print("initialization parked", file=sys.stderr, flush=True)
            return
        response = {"commands": [{"name": "help", "description": "Help"}],
                    "models": [{"value": "fake-model", "displayName": "Fake model",
                                "description": "Test only", "supportsEffort": True}],
                    "agents": [], "account": {}, "output_style": "default"}
    elif subtype == "get_usage":
        if HANG_USAGE:
            print("optional usage parked", file=sys.stderr, flush=True)
            return
        response = {"session": {"total_cost_usd": 9, "total_api_duration_ms": 1, "total_duration_ms": 1,
                    "total_lines_added": 0, "total_lines_removed": 0, "model_usage": {}},
                    "subscription_type": "pro", "rate_limits_available": True,
                    "rate_limits": {"five_hour": {"utilization": 25, "resets_at": "2099-01-01T00:00:00Z"},
                    "seven_day": {"utilization": 40, "resets_at": "2099-01-02T00:00:00Z"},
                    "model_scoped": [{"display_name": "Fable", "utilization": 7, "resets_at": "2099-01-02T00:00:00Z"}]}, "behaviors": None}
    elif subtype == "stop_task" and request.get("task_id") == "fail-task":
        emit({"type": "control_response", "response": {"subtype": "error", "request_id": message["request_id"], "error": "Stop fixture failure"}})
        return
    elif subtype == "get_context_usage":
        response = {"usedTokens": 123, "maxTokens": 200000, "source": "fake-cli"}
    elif subtype == "set_model" and request.get("model") == "no-response":
        print("model change parked", file=sys.stderr, flush=True)
        return
    elif subtype == "set_model" and request.get("model") == "cancel-form":
        emit({"type": "control_cancel_request", "request_id": "form"})
    control_response(message, response)


def handle_callback_prompt(text):
    if text == "callbacks":
        for index in (1, 2):
            callback(f"permission-{index}", {"subtype": "can_use_tool", "tool_name": "Bash",
                     "input": {"command": f"echo {index}"}, "tool_use_id": f"tool-{index}",
                     "agent_id": f"agent-{index}", "title": f"Allow command {index}?",
                     "permission_suggestions": [], "decision_reason": "test prompt"})
        callback("question", {"subtype": "elicitation", "mcp_server_name": "test-server",
                 "message": "Choose a value", "mode": "form", "requested_schema": {
                     "type": "object", "properties": {"value": {"type": "integer"}}, "required": ["value"]}})
        if "test-dialog" in SUPPORTED_DIALOG_KINDS:
            callback("dialog", {"subtype": "request_user_dialog", "dialog_kind": "test-dialog",
                     "payload": {"question": "Continue?"}})
        return True
    if text == "undeclared-dialog":
        # Deliberately violate the CLI contract to check the host's defensive path.
        callback("undeclared-dialog", {"subtype": "request_user_dialog", "dialog_kind": "undeclared",
                 "payload": {"question": "Unsupported"}})
        return True
    return False


def handle_prompt(message):
    global HANG_USAGE
    text = message["message"]["content"]
    if text == "park-usage":
        HANG_USAGE = True
        return
    if text.startswith("form:"):
        callback("form", {"subtype": "elicitation", "mcp_server_name": "test-server", "message": "Answer the form",
            "mode": "form", "requested_schema": json.loads(text[5:])})
        return
    if text.startswith("ask:"):
        callback("ask", {"subtype": "can_use_tool", "tool_name": "AskUserQuestion", "tool_use_id": "ask-tool",
            "input": json.loads(text[4:])})
        return
    if text == "eof":
        sys.exit(0)
    if text == "crash":
        print("fake CLI crashed", file=sys.stderr, flush=True)
        sys.exit(7)
    if handle_callback_prompt(text):
        return
    if text == "callback-flood":
        for index in range(8192):
            callback(f"flood-{index}", {"subtype": "can_use_tool", "tool_name": "Bash",
                     "input": {"command": "x" * 2048}, "tool_use_id": f"tool-{index}"})
        return
    if text in ("output-burst-small", "output-burst-large"):
        count = 16 if text.endswith("small") else 256
        for index in range(count):
            emit({"type": "system", "subtype": "test_output", "session_id": SESSION,
                  "sequence": index, "text": "x" * 65536})
        with open(os.environ["CARLOS_FAKE_CLI_LOG"], "a") as log:
            log.write(json.dumps({"fake": "burst_sent"}) + "\n")
        return
    if text == "cancel-callback":
        callback("cancel-me", {"subtype": "can_use_tool", "tool_name": "Read", "input": {}, "tool_use_id": "tool-cancel"})
        emit({"type": "control_cancel_request", "request_id": "cancel-me"})
        return
    emit({**message, "session_id": SESSION, "isReplay": True})
    emit({"type": "result", "subtype": "success", "session_id": SESSION, "uuid": "result-id",
          "is_error": False, "result": "Done", "duration_ms": 1, "duration_api_ms": 0,
          "num_turns": 1, "total_cost_usd": 0, "usage": {}, "modelUsage": {}, "permission_denials": []})
    emit({"type": "system", "subtype": "task_notification", "session_id": SESSION,
          "uuid": "task-event", "task_id": "child-1", "tool_use_id": "parent-tool",
          "status": "completed", "summary": "Child completed after result", "output_file": "/tmp/fake",
          "future_field": {"retained": True}})


if os.environ.get("CARLOS_FAKE_CLI_LOG"):
    with open(os.environ["CARLOS_FAKE_CLI_LOG"], "a") as log:
        log.write(json.dumps({"argv": sys.argv[1:], "cwd": os.getcwd(),
                              "emit_session_state": os.environ.get("CLAUDE_CODE_EMIT_SESSION_STATE_EVENTS")}) + "\n")
for line in sys.stdin:
    message = json.loads(line)
    if os.environ.get("CARLOS_FAKE_CLI_LOG"):
        with open(os.environ["CARLOS_FAKE_CLI_LOG"], "a") as log:
            log.write(json.dumps(message) + "\n")
    if message["type"] == "control_request":
        handle_control(message)
    elif message["type"] == "user":
        handle_prompt(message)
