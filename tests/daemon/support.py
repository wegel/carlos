"""Run the actual daemon with private homes, deterministic peers and captured wire traffic."""

import json
from contextlib import closing
import os
from pathlib import Path
import select
import shutil
import socket
import sqlite3
import subprocess
import tempfile
import time

REPO = Path(__file__).resolve().parents[2]
BINARY = REPO / "target/debug/carlosd"
ARTIFACTS = REPO / "target/daemon-artifacts"


def write_json(path, value):
    """Publish fixture controls as a complete file, including while native reads overlap."""
    with tempfile.NamedTemporaryFile(mode="w", dir=path.parent, delete=False) as pending:
        temporary = Path(pending.name)
        try:
            json.dump(value, pending)
            pending.close()
            temporary.replace(path)
        finally:
            temporary.unlink(missing_ok=True)


def wait_for(predicate, description, timeout=8):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(.005)
    raise AssertionError(description)


class Client:
    def __init__(self, fixture, name, resume=None, greet=True):
        self.path = fixture.root / (name + ".jsonl")
        self.socket = socket.socket(socket.AF_UNIX)
        self.socket.connect(str(fixture.socket))
        self.buffer = b""
        self.messages = []
        self.serial = 0
        params = {"protocol": 1, "client": name}
        if resume:
            params.update(resume=resume["client"], resume_token=resume["resume_token"], instance=resume["instance"])
        self.hello = self.call("hello", **params) if greet else None
        if greet:
            fixture.memory("hello:" + name)

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
    def __init__(self, name, codex=None, mode="modern", tool_summaries="off", listen=None, origin=None):
        ARTIFACTS.mkdir(parents=True, exist_ok=True)
        self.root = Path(tempfile.mkdtemp(prefix="carlos-daemon-" + name + "-", dir="/tmp"))
        self.artifacts = ARTIFACTS / self.root.name
        self.socket = self.root / "daemon.sock"
        self.env = dict(os.environ)
        private = {key: self.root / key.lower() for key in (
            "HOME", "CODEX_HOME", "CLAUDE_CONFIG_DIR", "GROK_HOME", "XDG_CONFIG_HOME", "XDG_STATE_HOME")}
        for key, directory in private.items():
            directory.mkdir(mode=0o700)
            self.env[key] = str(directory)
        shim = self.root / "bin"
        shim.mkdir()
        for command in ("codex", "grok", "claude"):
            path = shim / command
            path.write_text(native_wrapper(command))
            path.chmod(0o755)
        helper = BINARY.with_name("carlos-claude-controller")
        if not helper.is_file():
            helper = REPO / "target/claude-controller/carlos-claude-controller"
        self.env.update(PATH=str(shim) + os.pathsep + os.environ["PATH"], CARLOS_TEST_ROOT=str(self.root),
                        CARLOS_TEST_SERVER=mode,
                        CARLOS_CLAUDE_CONTROLLER=str(helper),
                        CARLOS_CLAUDE_WIRE_TRACE=str(self.root / "claude-controller.jsonl"),
                        CARLOS_CLAUDE_EXECUTABLE=str(shim / "claude"))
        if tool_summaries is None:
            self.env.pop("CARLOS_TOOL_SUMMARIES", None)
        else:
            self.env["CARLOS_TOOL_SUMMARIES"] = tool_summaries
        self.native = None
        if codex is True:
            from native_codex import NativeCodex
            self.native = NativeCodex(self.root)
            codex = self.native.path
        self.codex = codex
        self.listen = listen
        self.origin = origin
        self.ws_uri = None
        self.process = None
        self.start()

    def start(self):
        self.log = (self.root / "daemon.log").open("a")
        log_start = self.log.tell()
        arguments = [str(BINARY), "--socket", str(self.socket), "--state-dir", str(self.root / "state")]
        if self.codex:
            arguments += ["--codex-socket", str(self.codex)]
        if self.listen:
            arguments += ["--listen", self.listen]
        if self.origin:
            arguments += ["--allow-origin", self.origin]
        self.process = subprocess.Popen(arguments, env=self.env, stdout=self.log, stderr=self.log, cwd=self.root)
        wait_for(lambda: self.socket.exists() or self.process.poll() is not None, "daemon socket appeared")
        assert self.process.poll() is None, (self.root / "daemon.log").read_text()
        if self.listen:
            def address():
                text = (self.root / "daemon.log").read_bytes()[log_start:].decode()
                prefix = "carlosd: network listening on "
                return next((line[len(prefix):] for line in text.splitlines() if line.startswith(prefix)), None)
            self.ws_uri = wait_for(address, "network listener reports its actual bound address")

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
            shutil.copytree(self.root, self.artifacts, dirs_exist_ok=True,
                            ignore=lambda directory, names: [name for name in names if (Path(directory)/name).is_socket()])
            (self.artifacts / "source-root.txt").write_text(str(self.root))
            shutil.rmtree(self.root)

    def saved(self):
        with closing(sqlite3.connect(self.root / "state/sessions.sqlite3")) as database:
            return database.execute("SELECT id,name,native_thread,ralph FROM sessions ORDER BY id").fetchall()

    def memory(self, label):
        status = Path(f"/proc/{self.process.pid}/status")
        if not status.exists():
            return
        metrics = {}
        for line in status.read_text().splitlines():
            key, _, value = line.partition(":")
            if key in ("VmRSS", "VmHWM", "RssAnon", "RssFile", "Threads"):
                metrics[key] = int(value.split()[0])
        with (self.root / "daemon-memory.jsonl").open("a") as log:
            log.write(json.dumps({"label": label, "daemon_pid": self.process.pid, "metrics": metrics}) + "\n")

    def pids(self):
        path = self.root / "native-pids.jsonl"
        return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []


def native_wrapper(command):
    script = str(REPO / ("tests/ui/fake_" + command + ".py"))
    return f'''#!/usr/bin/env python3
import json, os, sys, time
from pathlib import Path
root = Path(os.environ["CARLOS_TEST_ROOT"])
with (root / "native-pids.jsonl").open("a") as log:
    log.write(json.dumps({{"backend": {command!r}, "pid": os.getpid(), "argv": sys.argv[1:],
        "context_window": os.environ.get("CLAUDE_CODE_AUTO_COMPACT_WINDOW")}}) + "\\n")
if {command!r} == "grok" and os.environ.get("CARLOS_TEST_SERVER") == "grok-held-initialize":
    sys.stdin.readline()
    (root / "initialize-held").write_text("waiting")
    while True:
        time.sleep(1)
script = {script!r}
if {command!r} == "grok" and os.environ.get("CARLOS_TEST_SERVER") == "grok-held-prompt":
    script = {str(REPO / "tests/daemon/native_grok.py")!r}
if {command!r} == "grok" and os.environ.get("CARLOS_TEST_SERVER") == "grok-held-open":
    script = {str(REPO / "tests/daemon/native_grok_start.py")!r}
if {command!r} == "grok" and os.environ.get("CARLOS_TEST_SERVER") == "grok-tool-facts":
    script = {str(REPO / "tests/daemon/native_grok_tool_facts.py")!r}
if {command!r} == "grok" and os.environ.get("CARLOS_TEST_SERVER", "").startswith("grok-permission-"):
    script = {str(REPO / "tests/daemon/native_grok_permissions.py")!r}
if {command!r} == "grok" and os.environ.get("CARLOS_TEST_SERVER", "").startswith("grok-saved-"):
    script = {str(REPO / "tests/daemon/native_grok_saved.py")!r}
if {command!r} == "claude" and os.environ.get("CARLOS_TEST_SERVER", "").startswith("claude-settings-"):
    script = {str(REPO / "tests/daemon/native_claude_settings.py")!r}
os.execv(sys.executable, [sys.executable, script, *sys.argv[1:]])
'''


def alive(pid):
    try:
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False
