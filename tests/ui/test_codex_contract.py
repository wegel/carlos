"""Check installed Codex history, forks, and saved names without inference or user sessions."""

import argparse
import json
import os
from pathlib import Path
import selectors
import subprocess
import tempfile
import time
import uuid

from codex_history_fixture import write_history


class Codex:
    def __init__(self, binary, root, extra_args=()):
        self.root = root
        self.root.mkdir(parents=True, exist_ok=True)
        (root / "codex-home").mkdir(exist_ok=True)
        self.log = (root / "rpc.jsonl").open("a")
        self.stderr = (root / "stderr.log").open("w")
        self.process = subprocess.Popen(
            [binary, *extra_args, "app-server"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=self.stderr,
            env={**os.environ, "CODEX_HOME": str(root / "codex-home")})
        self.selector = selectors.DefaultSelector()
        self.selector.register(self.process.stdout, selectors.EVENT_READ)
        self.next_id = 0
        self.buffer = b""

    def send(self, message):
        self.log.write(json.dumps({"direction": "client", "message": message}) + "\n")
        self.log.flush()
        self.process.stdin.write((json.dumps(message) + "\n").encode())
        self.process.stdin.flush()

    def call(self, method, params):
        self.next_id += 1
        request_id = self.next_id
        self.send({"id": request_id, "method": method, "params": params})
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            if b"\n" not in self.buffer:
                if not self.selector.select(0.1):
                    continue
                data = os.read(self.process.stdout.fileno(), 65536)
                if not data:
                    raise AssertionError("app-server exited; see " + str(self.root))
                self.buffer += data
                continue
            line, self.buffer = self.buffer.split(b"\n", 1)
            response = json.loads(line)
            self.log.write(json.dumps({"direction": "server", "message": response}) + "\n")
            self.log.flush()
            if response.get("id") == request_id:
                if "error" in response:
                    raise AssertionError(f"{method}: {response['error']}; see {self.root}")
                return response["result"]
        raise AssertionError("Timed out waiting for " + method)

    def close(self):
        self.process.terminate()
        try:
            self.process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()
        self.selector.close()
        self.process.stdin.close()
        self.process.stdout.close()
        self.log.close()
        self.stderr.close()


def fixture(root):
    session_id = str(uuid.uuid4())
    turn_ids = [str(uuid.uuid4()), str(uuid.uuid4())]
    timestamp = "2026-10-20T00:00:00Z"
    records = [{"type": "session_meta", "payload": {
        "id": session_id, "timestamp": timestamp, "cwd": str(root),
        "originator": "carlos-contract-test", "cli_version": "0.163.0",
        "source": "cli", "model_provider": "openai"}}]
    for key, text in zip(turn_ids, ["first", "second"]):
        records.extend([
            {"type": "event_msg", "payload": {"type": "task_started", "turn_id": key}},
            {"type": "event_msg", "payload": {"type": "user_message", "message": text,
                                                "images": [], "local_images": []}},
            {"type": "response_item", "payload": {"type": "message", "role": "user",
                "content": [{"type": "input_text", "text": text}]}},
            {"type": "response_item", "payload": {"type": "message", "role": "assistant",
                "phase": "final_answer", "content": [{"type": "output_text", "text": "Answer: " + text}]}},
            {"type": "event_msg", "payload": {"type": "task_complete", "turn_id": key,
                                                "last_agent_message": "Answer: " + text}},
        ])
    sessions = root / "codex-home" / "sessions"
    sessions.mkdir(parents=True)
    path = sessions / ("rollout-2026-10-20T00-00-00-" + session_id + ".jsonl")
    path.write_text("".join(json.dumps({"timestamp": timestamp, **record}) + "\n" for record in records))
    return session_id, turn_ids, path


def check(binary, root):
    root.mkdir(parents=True, exist_ok=True)
    version = subprocess.run([binary, "--version"], check=True, capture_output=True, text=True).stdout
    (root / "codex-version.txt").write_text(version)
    print(version.strip(), flush=True)
    schema = root / "schema"
    subprocess.run([binary, "app-server", "generate-json-schema", "--experimental", "--out", str(schema)],
                   check=True, capture_output=True)
    fork_schema = json.loads((schema / "v2" / "ThreadForkParams.json").read_text())
    assert "beforeTurnId" in fork_schema["properties"], "Codex lacks beforeTurnId"
    quota_schema = json.loads((schema / "v2" / "GetAccountRateLimitsResponse.json").read_text())
    assert "rateLimits" in quota_schema["properties"], "Codex lacks quota snapshots"
    window = quota_schema["definitions"]["RateLimitWindow"]["properties"]
    assert {"usedPercent", "windowDurationMins", "resetsAt"} <= window.keys()
    notifications = (schema / "ServerNotification.json").read_text()
    assert "account/rateLimits/updated" in notifications, "Codex lacks quota updates"
    session_id, turn_ids, path = fixture(root)
    original = path.read_bytes()
    server = Codex(binary, root)
    try:
        server.call("initialize", {"clientInfo": {"name": "carlos-contract-test", "version": "1"},
                                   "capabilities": {"experimentalApi": True}})
        server.send({"method": "initialized"})
        response = server.call("thread/read", {"threadId": session_id, "includeTurns": True})
        assert [turn["id"] for turn in response["thread"]["turns"]] == turn_ids
        for before, expected in zip(turn_ids, [[], turn_ids[:1]]):
            result = server.call("thread/fork", {"threadId": session_id,
                                                "beforeTurnId": before,"excludeTurns":True})
            fork_id = result["thread"]["id"]
            assert fork_id != session_id
            history = server.call("thread/turns/list", {"threadId": fork_id,
                "sortDirection": "asc", "itemsView": "full"})
            assert [turn["id"] for turn in history["data"]] == expected, history
        assert path.read_bytes() == original, "source fixture changed"
        check_rename(server, fork_id)
        print("PASS: installed Codex forks before first and later turns; source is intact")
    finally:
        server.close()
    verify_saved_name(binary, root, fork_id)
    check_queue_persistence(binary, root / "queue")
    check_paged_history(binary, root / "paged")
    check_context_size(binary, root / "context-size")


def check_context_size(binary, root):
    server = Codex(binary, root, ["-c", "model_context_window=500000",
                                  "-c", "model_auto_compact_token_limit=450000"])
    try:
        server.call("initialize", {"clientInfo": {"name": "carlos-context-test", "version": "1"},
                                   "capabilities": {"experimentalApi": True}})
        server.send({"method": "initialized"})
        config = server.call("config/read", {})["config"]
        assert config["model_context_window"] == 500000
        assert config["model_auto_compact_token_limit"] == 450000
        assert not (root / "codex-home/config.toml").exists(), "process flags wrote native defaults"
        print("PASS: context sizes reach installed Codex through process flags without writing native defaults")
    finally:
        server.close()


def check_queue_persistence(binary, root):
    root.mkdir(parents=True)
    thread,_,path=fixture(root)
    original=path.read_bytes()
    expected=[]
    for generation in range(2):
        server=Codex(binary,root)
        try:
            server.call("initialize",{"clientInfo":{"name":"carlos-queue-test","version":"1"},"capabilities":{"experimentalApi":True}})
            server.send({"method":"initialized"})
            server.call("thread/read",{"threadId":thread,"includeTurns":False})
            if generation==0:
                for index in range(3):
                    client=str(uuid.uuid4())
                    payload=[{"type":"text","text":"queue fixture "+str(index),"text_elements":[]}]
                    if index==0:
                        payload[0]["text"]+=" long native input"*6000
                    if index==1:
                        payload.append({"type":"image","url":"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a82sAAAAASUVORK5CYII=","detail":"low"})
                    added=server.call("thread/queue/add",{"threadId":thread,"clientUserMessageId":client,"input":payload})["queuedSubmission"]
                    assert added["clientUserMessageId"]==client
                    assert added["input"][0]["text"]==payload[0]["text"]
                    if index==1:
                        assert added["input"][1]["url"]==payload[1]["url"]
                    expected.append(added)
                expected.reverse()
                server.call("thread/queue/reorder",{"threadId":thread,"queuedSubmissionIds":[entry["id"] for entry in expected]})
                edited=server.call("thread/queue/update",{"threadId":thread,"queuedSubmissionId":expected[0]["id"],
                    "input":[{"type":"text","text":"saved Unicode 日本語","text_elements":[]}]})["queuedSubmission"]
                assert edited["clientUserMessageId"]==expected[0]["clientUserMessageId"]
                expected[0]=edited
            pages=[]
            cursor=None
            while True:
                page=server.call("thread/queue/list",{"threadId":thread,"cursor":cursor,"limit":1})
                assert len(page["data"])<=1
                pages.extend(page["data"])
                cursor=page.get("nextCursor")
                if cursor is None:
                    break
            assert pages==expected,"native queue payload/order changed across server restart"
            if generation==1:
                deleted=server.call("thread/queue/delete",{"threadId":thread,"queuedSubmissionId":expected[1]["id"]})
                assert deleted["deleted"] is True
                assert server.call("thread/queue/delete",{"threadId":thread,"queuedSubmissionId":expected[1]["id"]})["deleted"] is False
            assert path.read_bytes()==original,"an unconsumed queue entry changed transcript history"
        finally:
            server.close()
    events=[json.loads(line)["message"] for line in (root/"rpc.jsonl").read_text().splitlines()]
    assert not any(message.get("method")=="turn/started" for message in events),"an unloaded queue must not start inference"
    print("PASS: native queue pages preserve IDs, attachments, order and edited text across restart without inference")


def check_paged_history(binary, root):
    root.mkdir(parents=True)
    thread, turns, path = write_history(root,count=105,compact=True)
    server = Codex(binary,root)
    try:
        server.call("initialize",{"clientInfo":{"name":"carlos-history-test","version":"1"},"capabilities":{"experimentalApi":True}})
        server.send({"method":"initialized"})
        resumed = server.call("thread/resume",{"threadId":thread,"excludeTurns":True})
        assert resumed["thread"]["turns"] == []
        original = path.read_bytes()
        history = paged_history(server,thread)
        assert [turn["id"] for turn in history] == turns
        assert sum(item["type"] == "contextCompaction" for turn in history for item in turn["items"]) == 1
        assert history[-1]["items"][-1]["text"] == "HISTORY_REPLY:104"
        fork = server.call("thread/fork",{"threadId":thread,"beforeTurnId":turns[-1],"excludeTurns":True})
        assert fork["thread"]["turns"] == []
        fork_history = paged_history(server,fork["thread"]["id"])
        assert [turn["id"] for turn in fork_history] == turns[:-1]
        assert path.read_bytes() == original, "fork changed source history"
        print("PASS: metadata-only resume/fork,106 compacted turns and item pages preserve exact ancestry")
    finally:
        server.close()


def paged_history(server, thread):
    turns, cursor = [], None
    while True:
        page = server.call("thread/turns/list",{"threadId":thread,"sortDirection":"asc","itemsView":"notLoaded","limit":100,"cursor":cursor})
        turns.extend(page["data"])
        cursor = page.get("nextCursor")
        if cursor is None:
            break
    for turn in turns:
        assert turn["items"] == []
        cursor, items = None, []
        while True:
            page = server.call("thread/items/list",{"threadId":thread,"turnId":turn["id"],"sortDirection":"asc","limit":1,"cursor":cursor})
            assert all(entry["turnId"] == turn["id"] for entry in page["data"])
            items.extend(entry["item"] for entry in page["data"])
            cursor = page.get("nextCursor")
            if cursor is None:
                break
        turn["items"] = items
    return turns


def check_rename(server, thread_id):
    before = server.call("thread/read", {"threadId": thread_id, "includeTurns": True})
    server.call("thread/name/set", {"threadId": thread_id, "name": "Renamed test session"})
    after = server.call("thread/read", {"threadId": thread_id, "includeTurns": True})
    assert after["thread"]["name"] == "Renamed test session"
    assert before["thread"]["turns"] == after["thread"]["turns"], "rename changed transcript"
    events = [json.loads(line)["message"] for line in (server.root / "rpc.jsonl").read_text().splitlines()]
    assert any(event.get("method") == "thread/name/updated"
               and event["params"].get("threadId") == thread_id
               and event["params"].get("threadName") == "Renamed test session" for event in events)


def verify_saved_name(binary, root, thread_id):
    server = Codex(binary, root)
    try:
        server.call("initialize", {"clientInfo": {"name": "carlos-contract-test", "version": "1"}})
        server.send({"method": "initialized"})
        thread = server.call("thread/read", {"threadId": thread_id})["thread"]
        assert thread["name"] == "Renamed test session", "name lost after server restart"
        print("PASS: installed Codex saves names across server restarts and leaves transcript unchanged")
    finally:
        server.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--codex", default="codex")
    parser.add_argument("--artifacts", type=Path)
    args = parser.parse_args()
    root = (args.artifacts or Path(tempfile.mkdtemp(prefix="carlos-codex-contract-"))).resolve()
    print("Codex contract artifacts:", root, flush=True)
    check(args.codex, root)
