"""Run the actual daemon with private homes, deterministic peers and captured wire traffic."""

import json
import os
from pathlib import Path
import select
import socket
import sqlite3
import subprocess
import tempfile
import time

REPO = Path(__file__).resolve().parents[2]
BINARY = REPO / "target/debug/carlosd"
ARTIFACTS = REPO / "target/daemon-artifacts"


def wait_for(predicate, description, timeout=8):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(.005)
    raise AssertionError(description)


class Client:
    def __init__(self, fixture, name, resume=None):
        self.path = fixture.root / (name + ".jsonl")
        self.socket = socket.socket(socket.AF_UNIX)
        self.socket.connect(str(fixture.socket))
        self.buffer = b""
        self.messages = []
        self.serial = 0
        params = {"protocol": 1, "client": name}
        if resume:
            params.update(resume=resume["client"], resume_token=resume["resume_token"], instance=resume["instance"])
        self.hello = self.call("hello", **params)

    def record(self, direction, message):
        with self.path.open("a") as log:
            log.write(json.dumps({"direction": direction, "message": message}) + "\n")

    def send(self, method, **params):
        self.serial += 1
        message = {"jsonrpc": "2.0", "id": self.serial, "method": method, "params": params}
        self.record("client", message)
        self.socket.sendall((json.dumps(message) + "\n").encode())
        return self.serial

    def receive(self, timeout=8):
        deadline = time.monotonic() + timeout
        while b"\n" not in self.buffer:
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not select.select([self.socket], [], [], remaining)[0]:
                raise AssertionError("daemon reply timed out; see " + str(self.path))
            chunk = self.socket.recv(65536)
            assert chunk, "daemon connection closed; see " + str(self.path)
            self.buffer += chunk
        line, self.buffer = self.buffer.split(b"\n", 1)
        message = json.loads(line)
        self.record("server", message)
        self.messages.append(message)
        return message

    def response(self, identity):
        while True:
            for message in self.messages:
                if message.get("id") == identity:
                    return message
            self.receive()

    def call(self, method, **params):
        reply = self.response(self.send(method, **params))
        assert "result" in reply, reply
        return reply["result"]

    def operation(self, counter):
        return {"client": self.hello["client"], "id": str(counter)}

    def outcome(self, operation):
        return wait_for(lambda: self._terminal(operation), "private operation finished")

    def _terminal(self, operation):
        result = self.call("operation/read", operation=operation)
        return result if result["state"]["kind"] not in ("pending", "queued") else None

    def close(self):
        self.socket.close()

    def message(self, predicate):
        while True:
            found = next((message for message in self.messages if predicate(message)), None)
            if found:
                return found
            self.receive()


class Fixture:
    def __init__(self, name, codex=None, mode="modern"):
        ARTIFACTS.mkdir(parents=True, exist_ok=True)
        self.root = Path(tempfile.mkdtemp(prefix=name + "-", dir=ARTIFACTS))
        self.socket = self.root / "daemon.sock"
        self.env = dict(os.environ)
        private = {key: self.root / key.lower() for key in (
            "HOME", "CODEX_HOME", "CLAUDE_CONFIG_DIR", "GROK_HOME", "XDG_CONFIG_HOME", "XDG_STATE_HOME")}
        for key, directory in private.items():
            directory.mkdir(mode=0o700)
            self.env[key] = str(directory)
        shim = self.root / "bin"
        shim.mkdir()
        (shim / "grok").symlink_to(REPO / "tests/ui/fake_grok.py")
        for command in ("codex", "claude"):
            path = shim / command
            path.write_text("#!/bin/sh\nexit 79\n")
            path.chmod(0o755)
        self.env.update(PATH=str(shim) + os.pathsep + os.environ["PATH"], CARLOS_TEST_ROOT=str(self.root),
                        CARLOS_TEST_SERVER=mode,
                        CARLOS_CLAUDE_CONTROLLER=str(REPO / "target/claude-controller/carlos-claude-controller"),
                        CARLOS_CLAUDE_EXECUTABLE=str(REPO / "tests/ui/fake_claude.py"))
        self.native = None
        if codex is True:
            from native_codex import NativeCodex
            self.native = NativeCodex(self.root)
            codex = self.native.path
        self.codex = codex
        self.process = None
        self.start()

    def start(self):
        self.log = (self.root / "daemon.log").open("a")
        arguments = [str(BINARY), "--socket", str(self.socket), "--state-dir", str(self.root / "state")]
        if self.codex:
            arguments += ["--codex-socket", str(self.codex)]
        self.process = subprocess.Popen(arguments, env=self.env, stdout=self.log, stderr=self.log, cwd=self.root)
        wait_for(lambda: self.socket.exists() or self.process.poll() is not None, "daemon socket appeared")
        assert self.process.poll() is None, (self.root / "daemon.log").read_text()

    def stop(self):
        if self.process and self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
                raise AssertionError("explicit daemon shutdown did not finish: " + str(self.root))
        if self.process:
            assert self.process.returncode == 0, (self.root / "daemon.log").read_text()
        self.log.close()

    def native_requests(self, method):
        path = self.root / "rpc.jsonl"
        if not path.exists():
            return []
        return [record["message"] for line in path.read_text().splitlines() if
                (record := json.loads(line))["direction"] == "client" and record["message"].get("method") == method]

    def close(self):
        try:
            self.stop()
        finally:
            if self.native:
                self.native.close()

    def saved(self):
        with sqlite3.connect(self.root / "state/sessions.sqlite3") as database:
            return database.execute("SELECT id,name,native_thread,ralph FROM sessions ORDER BY id").fetchall()
