#!/usr/bin/env python3
"""Test the helper through an OS pipe that this host genuinely stops reading."""
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import tempfile
import time
import uuid


class Host:
    def __init__(self, command, directory):
        self.log = Path(directory) / "cli.jsonl"
        env = {"PATH": "/usr/bin:/bin", "HOME": directory,
               "CLAUDE_CONFIG_DIR": directory + "/claude", "CARLOS_FAKE_CLI_LOG": str(self.log)}
        self.process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=subprocess.PIPE, cwd=directory, env=env)
        self.buffer = b""
        self.frames = []
        self.directory = directory

    def send(self, request_id, method, params=None):
        frame = {"v": 1, "id": request_id, "method": method, "params": params or {}}
        self.process.stdin.write((json.dumps(frame) + "\n").encode())
        self.process.stdin.flush()

    def prompt(self, request_id, text):
        self.send(request_id, "prompt", {"uuid": str(uuid.uuid4()), "text": text,
                  "origin": {"kind": "human"}, "priority": "later"})

    def read_until(self, predicate):
        deadline = time.monotonic() + 4
        while not predicate(self.frames):
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not select.select([self.process.stdout], [], [], remaining)[0]:
                raise RuntimeError("Timed out waiting for helper frames")
            chunk = os.read(self.process.stdout.fileno(), 65536)
            if not chunk:
                raise RuntimeError("Helper exited before expected frames arrived")
            self.buffer += chunk
            while b"\n" in self.buffer:
                line, self.buffer = self.buffer.split(b"\n", 1)
                self.frames.append(json.loads(line))

    def cli_until(self, predicate):
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            lines = self.log.read_text().splitlines() if self.log.exists() else []
            # The fake CLI may still be appending the final line.
            frames = []
            for line in lines:
                try:
                    frames.append(json.loads(line))
                except json.JSONDecodeError:
                    pass
            if any(predicate(frame) for frame in frames):
                return
            time.sleep(.01)
        raise RuntimeError("Timed out waiting for native CLI input")

    def start(self):
        self.send("start", "start", {"executable": str(Path(__file__).with_name("fake-cli.py")),
                  "cwd": self.directory, "settingSources": ["user", "project", "local"]})
        self.read_until(lambda frames: any(frame.get("id") == "start" for frame in frames))

    def close(self):
        if self.process.poll() is None:
            self.process.kill()
        self.process.wait(timeout=3)


def resume_after_controls(host):
    host.prompt("callbacks", "callbacks")
    host.read_until(lambda frames: len([f for f in frames if f.get("type") == "callback"]) == 3)
    host.prompt("burst", "output-burst-small")
    host.cli_until(lambda frame: frame.get("fake") == "burst_sent")
    # No read from helper stdout occurs between the burst and these native replies.
    host.send("answer", "callback.respond", {"callbackId": "permission-1",
              "answer": {"behavior": "deny", "message": "No"}})
    host.send("model", "model.set", {"model": "sonnet"})
    host.cli_until(lambda frame: frame.get("response", {}).get("request_id") == "permission-1")
    host.cli_until(lambda frame: frame.get("request", {}).get("subtype") == "set_model")
    expected = {"burst", "answer", "model"}
    for index in range(20):
        request_id = f"hello-{index}"
        expected.add(request_id)
        host.send(request_id, "hello")
    time.sleep(.05)
    host.read_until(lambda frames: has_all_replies(frames, expected) and len(output_events(frames)) == 16)
    host.send("shutdown", "shutdown")
    host.read_until(lambda frames: any(f.get("id") == "shutdown" for f in frames))
    result = {"code": host.process.wait(timeout=3), "sequences": output_events(host.frames),
              "replyCounts": {key: len([f for f in host.frames if f.get("id") == key]) for key in expected},
              "controlsBeforeDrain": True}
    return result


def has_all_replies(frames, expected):
    return expected.issubset({frame.get("id") for frame in frames if frame.get("ok") is True})


def output_events(frames):
    return [frame["message"]["sequence"] for frame in frames
            if frame.get("message", {}).get("subtype") == "test_output"]


def leave_output_blocked(host, mode):
    text = {"flood": "output-burst-large", "callback-flood": "callback-flood"}.get(mode, "output-burst-small")
    host.prompt("burst", text)
    if mode not in ("flood", "callback-flood"):
        host.cli_until(lambda frame: frame.get("fake") == "burst_sent")
        if mode == "eof":
            host.process.stdin.close()
        else:
            host.send("shutdown", "shutdown")
    started = time.monotonic()
    code = host.process.wait(timeout=4)
    return {"code": code, "stderr": host.process.stderr.read().decode(),
            "elapsed": time.monotonic() - started}


def main():
    mode = sys.argv[1]
    with tempfile.TemporaryDirectory(prefix="carlos-blocked-host-") as directory:
        host = Host(sys.argv[2:], directory)
        try:
            host.start()
            result = resume_after_controls(host) if mode == "resume" else leave_output_blocked(host, mode)
            print(json.dumps(result))
        finally:
            host.close()


if __name__ == "__main__":
    main()
