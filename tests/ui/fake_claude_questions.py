"""Native SDK callback fixtures; every answer returns through its original request ID."""

import json
import os
from pathlib import Path
import threading
import time
import uuid

pending = set()
sender = None
finish_result = None
set_state = None
prompt_id = None
scenario = None
parent_finished = False
session_id = None


def permission(identifier, tool="Bash", title=None, **extra):
    pending.add(identifier)
    sender({"type": "control_request", "request_id": identifier, "request": {
        "subtype": "can_use_tool", "tool_name": tool, "tool_use_id": "tool-" + identifier,
        "input": {"command": "echo " + identifier}, "title": title or "Allow " + identifier + "?", **extra}})


def form(identifier, schema=None, **extra):
    pending.add(identifier)
    sender({"type": "control_request", "request_id": identifier, "request": {
        "subtype": "elicitation", "mcp_server_name": "counter-server", "title": "Native form",
        "message": "Choose values for this server", "description": "Only this request receives the answers",
        "requested_schema": schema or {"type": "object", "properties": {
            "age": {"type": "integer", "title": "Age", "minimum": 18, "maximum": 99}}, "required": ["age"]}, **extra}})


def question(identifier, single=False, default_to_no=False):
    questions = [{"question": "Which output format?", "header": "Format", "multiSelect": False,
                  "options": [{"label": "Summary", "description": "Brief result"},
                              {"label": "Detailed", "description": "Full explanation"}]}]
    if not single:
        questions += [{"question": "Which sections?", "header": "Sections", "multiSelect": True,
                       "options": [{"label": "Introduction", "description": "Opening context"},
                                   {"label": "Conclusion", "description": "Closing text"}]},
                      {"question": "Which custom greeting?", "header": "Greeting", "multiSelect": False,
                       "options": [{"label": "Hello", "description": "English"},
                                   {"label": "Bonjour", "description": "French"}]}]
    permission(identifier, "AskUserQuestion", "Answer native questions", agent_id="child-ask",
               input={"questions": questions, "metadata": {"source": "fixture"}}, decision_reason="Child needs your choice",
               default_to_no=default_to_no)


def start(text, prompt, send, result, state, native_session):
    global sender, finish_result, set_state, prompt_id, scenario, parent_finished, session_id
    if text not in {"ask native", "ask single", "mcp form", "typed form", "browser consent", "native plan",
                    "permission flags", "concurrent questions", "cancel questions", "unsupported form", "prepare questions", "undeclared dialog", "browser early", "precise integer", "ask default no"}:
        return False
    sender, finish_result, set_state, prompt_id, scenario = send, result, state, prompt, text
    session_id = native_session
    parent_finished = text == "prepare questions"
    if text == "prepare questions":
        def wait_for_host():
            gate = Path(os.environ["CARLOS_TEST_ROOT"]) / "show-questions"
            while not gate.exists():
                time.sleep(.01)
            question("late-question", single=True)
        threading.Thread(target=wait_for_host, daemon=True).start()
        return False
    if text == "undeclared dialog":
        sender({"type": "control_request", "request_id": "unknown-dialog", "request": {
            "subtype": "request_user_dialog", "dialog_kind": "unknown-kind", "payload": {"question": "Unknown"}}})
    elif text in {"ask native", "ask single", "ask default no"}:
        question("ask-question", single=text != "ask native", default_to_no=text == "ask default no")
    elif text in {"mcp form", "unsupported form"}:
        form("form-question", schema={"type": "object", "properties": {"nested": {"type": "object"}}} if text == "unsupported form" else None)
    elif text == "typed form":
        form("typed-question", schema={"type": "object", "properties": {
            "a_age": {"type": "integer", "title": "Age", "minimum": 18, "default": 30},
            "b_email": {"type": "string", "title": "Email", "format": "email"},
            "c_color": {"type": "string", "title": "Color", "oneOf": [{"const": "#f00", "title": "Red"}, {"const": "#0f0", "title": "Green"}]},
            "d_tags": {"type": "array", "title": "Tags", "minItems": 1, "maxItems": 2,
                       "items": {"anyOf": [{"const": "r", "title": "Read"}, {"const": "w", "title": "Write"}]}},
            "e_optin": {"type": "boolean", "title": "Opt in", "default": False},
            "f_note": {"type": "string", "title": "Note", "default": "Draft note"}},
            "required": ["a_age", "b_email", "c_color", "d_tags", "e_optin"]})
    elif text == "precise integer":
        form("integer-question", schema={"type": "object", "properties": {"count": {"type": "integer", "title": "Count"}}, "required": ["count"]})
    elif text in {"browser consent", "browser early"}:
        form("browser-question", mode="url", url="https://auth.example.com/confirm?request=fixture", elicitation_id="browser-1")
        if text == "browser early":
            sender({"type": "system", "subtype": "elicitation_complete", "session_id": session_id,
                    "uuid": str(uuid.uuid4()), "mcp_server_name": "counter-server", "elicitation_id": "browser-1"})
    elif text == "native plan":
        permission("plan-question", "ExitPlanMode", "Review native plan", input={
            "plan": "# Proposed native plan\n" + "\n".join(f"Step {i}: review source" for i in range(1, 25)) + "\nFinal plan line",
            "planFilePath": "/tmp/native-plan.md"}, description="Accept this plan once; no broader rule is saved")
    elif text == "permission flags":
        permission("flag-question", "mcp__fixture__run", "Untrusted\x1b[2J title", default_to_no=True,
                   suppress_always_allow_rule=True, agent_id="child-flags", decision_reason="Check\x1b[31m carefully",
                   mcp_server={"name": "fixture-server", "source": "plugin"}, permission_suggestions=[{
                       "type": "addRules", "rules": [{"toolName": "Bash"}], "behavior": "allow", "destination": "session"}])
    else:
        question("child-question", single=True)
        permission("child-permission", agent_id="child-two", description="Independent child permission")
        permission("child-third", agent_id="child-three")
        form("child-form")
        if text == "concurrent questions":
            finish_result(prompt)
            set_state("idle")
            parent_finished = True
    return True


def control(subtype):
    if subtype != "interrupt" or scenario != "cancel questions":
        return
    for identifier in ["child-permission", "child-question"]:
        pending.discard(identifier)
        sender({"type": "control_cancel_request", "request_id": identifier})


def response(message):
    identifier = message["response"]["request_id"]
    if identifier not in pending:
        return False
    pending.remove(identifier)
    if identifier == "browser-question" and message["response"].get("response", {}).get("action") == "accept":
        for elicitation in ["unknown-browser", "browser-1", "browser-1"]:
            sender({"type": "system", "subtype": "elicitation_complete", "session_id": session_id,
                    "uuid": str(uuid.uuid4()), "mcp_server_name": "counter-server", "elicitation_id": elicitation})
    if not pending:
        sender({"type": "assistant", "session_id": session_id,
                "message": {"id": "callback-finished", "model": "sdk-sonnet", "role": "assistant",
                            "content": [{"type": "text", "text": "Native callbacks settled"}]}})
        if not parent_finished:
            finish_result(prompt_id)
            set_state("idle")
        return True
    return False
