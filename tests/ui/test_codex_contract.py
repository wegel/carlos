"""Check installed Codex history/fork methods without inference or user sessions."""

import argparse
import json
import os
from pathlib import Path
import selectors
import subprocess
import tempfile
import time
import uuid


class Codex:
    def __init__(self, binary, root):
        self.root = root
        self.root.mkdir(parents=True, exist_ok=True)
        self.log = (root / "rpc.jsonl").open("w")
        self.stderr = (root / "stderr.log").open("w")
        self.process = subprocess.Popen(
            [binary, "app-server"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
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
                                                "beforeTurnId": before})
            fork_id = result["thread"]["id"]
            assert fork_id != session_id
            history = server.call("thread/turns/list", {"threadId": fork_id,
                "sortDirection": "asc", "itemsView": "full"})
            assert [turn["id"] for turn in history["data"]] == expected, history
        assert path.read_bytes() == original, "source fixture changed"
        print("PASS: installed Codex forks before first and later turns; source is intact")
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
