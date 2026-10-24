#!/usr/bin/env python3
"""Deterministic app-server peer for tests of the real Carlos executable."""

import copy
import json
import os
from pathlib import Path
import sys


def user_item(key, text):
    return {"type": "userMessage", "id": key,
            "content": [{"type": "text", "text": text}]}


def completed_turn(key, text):
    return {"id": key, "status": "completed", "items": [
        user_item(key + "-user", text),
        {"type": "agentMessage", "id": key + "-answer",
         "phase": "final_answer", "text": "Answer: " + text},
    ]}


def source_items(command_read=False):
    read = {"type": "toolOutput", "id": "source-read", "name": "Read",
            "input": {"file_path": "main.rs"}, 'output': '1:let greeting = "hello";'}
    if command_read:
        read = {"type": "commandExecution", "id": "source-read", "command": "cat main.rs",
                "commandActions": [{"type": "read", "command": "cat main.rs", "path": "main.rs"}],
                "aggregatedOutput": '1:let greeting = "hello";', "exitCode": 0}
    return [read, {"type": "toolOutput", "id": "source-diff", "files": [{
        "filePath": "main.rs", "diff": '@@ -1 +1 @@\n-let value = "before";\n+let value = "after";\n'
    }]}]


def long_tool_items(failure=False):
    if failure:
        return [{"type": "commandExecution", "id": "failed-command", "command": "run-check",
                 "aggregatedOutput": "\n".join(f"failure detail {i}" for i in range(8)), "exitCode": 2}]
    return [{"type": "commandExecution", "id": "long-read", "command": "cat source.rs",
             "commandActions": [{"type": "read", "command": "cat source.rs", "path": "source.rs"}],
             "aggregatedOutput": "\n".join(f"let read_{i} = {i};" for i in range(8)), "exitCode": 0},
            {"type": "toolOutput", "id": "long-diff", "files": [{"filePath": "source.rs",
             "diff": "@@ -1,4 +1,4 @@\n" + "\n".join(
                 [f"-let before_{i} = {i};" for i in range(4)] +
                 [f"+let after_{i} = {i};" for i in range(4)])}]}]


def short_tool_items():
    script = "python3 - <<'PY'\nfrom pathlib import Path\nprint('short script marker')\nPY"
    return [{"type": "commandExecution", "id": "short-script", "command": script,
             "aggregatedOutput": "short script marker", "exitCode": 0},
            {"type": "commandExecution", "id": "short-git", "command": "git add source.rs",
             "aggregatedOutput": "", "exitCode": 0}]


class Server:
    def __init__(self):
        self.root = Path(os.environ["CARLOS_TEST_ROOT"])
        self.mode = os.environ.get("CARLOS_TEST_SERVER", "modern")
        self.state_path = self.root / "server-state.json"
        self.threads = json.loads(self.state_path.read_text()) if self.state_path.exists() else {
            "thread-original": {"id": "thread-original", "turns": [
                completed_turn("turn-first", "first"),
                completed_turn("turn-second", "second"),
            ]}
        }
        self.counter = 0
        self.failed_fork = False
        self.failed_start = False
        self.failed_rename = False
        if self.mode == "source-history":
            self.threads["thread-original"]["turns"] = [{
                "id": "source-turn", "status": "completed", "items": source_items(command_read=True)}]
        if self.mode == "tool-history":
            self.threads["thread-original"]["turns"] = [{
                "id": "tools-turn", "status": "completed", "items": long_tool_items()}]
        if self.mode.startswith("named"):
            self.threads["thread-original"].setdefault("name", "Cleanup monitor")
        if self.mode == "automatic":
            self.threads["thread-original"]["turns"].insert(
                1, {"id": "turn-automatic", "status": "completed", "items": []})

    def record(self, direction, value):
        with (self.root / "rpc.jsonl").open("a") as log:
            log.write(json.dumps({"direction": direction, "message": value}) + "\n")

    def send(self, value):
        self.record("server", value)
        print(json.dumps(value), flush=True)

    def notify(self, method, params):
        self.send({"method": method, "params": params})

    def reply(self, request, result=None, error=None):
        self.send({"id": request["id"], **(
            {"error": {"code": -32600, "message": error}}
            if error else {"result": result})})

    def persist(self):
        self.state_path.write_text(json.dumps(self.threads))

    def run(self):
        for line in sys.stdin:
            request = json.loads(line)
            self.record("client", request)
            if "id" not in request:
                continue
            self.handle(request)
            self.persist()

    def handle(self, request):
        method = request["method"]
        params = request.get("params", {})
        if method == "initialize":
            self.reply(request, {"userAgent": "carlos-test-server"})
        elif method == "model/list":
            self.reply(request, {"data": [], "nextCursor": None})
        elif method in ("thread/read", "thread/resume"):
            thread = copy.deepcopy(self.threads[params["threadId"]])
            if self.mode == "paginated":
                thread["turns"] = []
            self.reply(request, {"thread": thread})
        elif method == "thread/start":
            self.threads["thread-new"] = {"id": "thread-new", "turns": []}
            self.reply(request, {"thread": self.threads["thread-new"]})
        elif method == "thread/name/set":
            self.rename(request, params)
        elif method == "thread/fork":
            self.fork(request, params)
        elif method == "thread/rollback":
            self.rollback(request, params)
        elif method == "turn/start":
            self.start_turn(request, params)
        elif method == "turn/interrupt":
            self.reply(request, {})
            self.notify("turn/completed", {"threadId": params["threadId"],
                        "turn": {"id": params["turnId"], "status": "interrupted"}})
        elif method == "thread/turns/list":
            self.list_turns(request, params)
        else:
            self.reply(request, error="Unexpected method: " + method)

    def rename(self, request, params):
        if self.mode == "named-error" and not self.failed_rename:
            self.failed_rename = True
            self.reply(request, error="injected rename failure")
            return
        self.threads[params["threadId"]]["name"] = params["name"]
        self.reply(request, {})
        self.notify("thread/name/updated", {"threadId": params["threadId"], "threadName": params["name"]})

    def list_turns(self, request, params):
        if self.mode == "legacy":
            self.send({"id": request["id"], "error": {
                "code": -32601, "message": "Unknown method: thread/turns/list"}})
            return
        turns = self.threads[params["threadId"]]["turns"]
        if self.mode == "paginated" and len(turns) > 1 and not params.get("cursor"):
            self.reply(request, {"data": turns[:1], "nextCursor": "page-2"})
        else:
            # The next page deliberately overlaps the anchor to test ID merging.
            self.reply(request, {"data": turns, "nextCursor": None})

    def fork(self, request, params):
        if self.mode == "fork-error" and not self.failed_fork:
            self.failed_fork = True
            self.reply(request, error="injected fork failure")
            return
        self.counter += 1
        key = "thread-fork-" + str(self.counter)
        while key in self.threads:
            self.counter += 1
            key = "thread-fork-" + str(self.counter)
        thread = copy.deepcopy(self.threads[params["threadId"]])
        thread["id"] = key
        if self.mode == "named-fork":
            thread["name"] = None
        before = params.get("beforeTurnId")
        if self.mode != "legacy" and before:
            index = next(i for i, turn in enumerate(thread["turns"]) if turn["id"] == before)
            thread["turns"] = thread["turns"][:index]
        self.threads[key] = thread
        snapshot = copy.deepcopy(thread)
        if self.mode == "paginated":
            snapshot["turns"] = []
        self.reply(request, {"thread": snapshot})
        self.notify("thread/started", {"thread": thread})

    def rollback(self, request, params):
        if self.mode != "legacy":
            self.send({"id": request["id"], "error": {
                "code": -32601, "message": "Unknown method: thread/rollback"}})
            return
        thread = self.threads[params["threadId"]]
        thread["turns"] = thread["turns"][:-params["numTurns"]]
        self.reply(request, {"thread": thread})

    def start_turn(self, request, params):
        if self.mode == "start-error" and not self.failed_start:
            self.failed_start = True
            self.reply(request, error="injected turn failure")
            return
        thread_id = params["threadId"]
        text = params["input"][0]["text"]
        key = "turn-" + str(len(self.threads[thread_id]["turns"]) + 1)
        turn = completed_turn(key, text)
        if self.mode == "source-colors":
            turn["items"][1:1] = source_items()
        if self.mode in ("tool-output", "tool-live", "tool-failure"):
            turn["items"][1:1] = long_tool_items(failure=self.mode == "tool-failure")
        if self.mode == "tool-short":
            turn["items"][1:1] = short_tool_items()
        if self.mode == "tool-live":
            turn["status"] = "inProgress"
        if self.mode == "named-live":
            turn["status"] = "inProgress"
            turn["items"] = turn["items"][:1]
        if self.mode == "native-activity":
            turn["items"][1:1] = [
                {"type": "sleep", "id": "monitor-sleep", "durationMs": 30000},
                {"type": "subAgentActivity", "id": "monitor-start", "kind": "started",
                 "agentPath": "/root/monitor", "agentThreadId": "child"},
            ]
        self.reply(request, {"turn": {"id": key, "status": "inProgress"}})
        self.notify("turn/started", {"threadId": thread_id,
                    "turn": {"id": key, "status": "inProgress"}})
        for item in turn["items"]:
            self.notify("item/started", {"threadId": thread_id, "turnId": key,
                        "item": {**item, "text": ""} if item["type"] == "agentMessage" else item})
            if item["type"] == "agentMessage":
                self.notify("item/agentMessage/delta", {"threadId": thread_id,
                            "turnId": key, "itemId": item["id"], "delta": item["text"]})
            self.notify("item/completed", {"threadId": thread_id, "turnId": key, "item": item})
        self.threads[thread_id]["turns"].append(turn)
        if self.mode in ("named-live", "tool-live"):
            return
        self.notify("turn/completed", {"threadId": thread_id, "turn": turn})
        if self.mode == "named-external":
            self.threads[thread_id]["name"] = "Changed in native Codex"
            self.notify("thread/name/updated", {"threadId": thread_id, "threadName": "Changed in native Codex"})
            self.notify("thread/name/updated", {"threadId": "another-thread", "threadName": "Wrong name"})
        if self.mode == "native-activity":
            done = {"type": "subAgentActivity", "id": "monitor-done", "kind": "completed",
                    "agentPath": "/root/monitor", "agentThreadId": "child"}
            turn["items"].append(done)
            for _ in range(2):
                self.notify("item/completed", {"threadId": thread_id, "turnId": key, "item": done})
        if self.mode == "replayed-start":
            self.notify("turn/started", {"threadId": thread_id,
                        "turn": {"id": key, "status": "inProgress"}})


if __name__ == "__main__":
    Server().run()
