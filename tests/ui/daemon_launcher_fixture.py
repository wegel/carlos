"""Own private managed-daemon fixtures and reap only descendants captured from this test."""

import ctypes
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import threading
import time
from types import SimpleNamespace

from terminal import Terminal
from terminal_daemon_processes import capture_children, processes, remaining
from test_daemon_client import daemon_support
from native_codex import NativeCodex


def subreaper(enabled):
    if sys.platform == "linux":
        library = ctypes.CDLL(None, use_errno=True)
        previous = ctypes.c_int()
        assert library.prctl(37, ctypes.byref(previous), 0, 0, 0) == 0
        assert library.prctl(36, int(enabled), 0, 0, 0) == 0
        return bool(previous.value)
    return False


class Host:
    def __init__(self, binary, daemon, artifacts, name, companion=True):
        self.root = artifacts / name
        self.root.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix="carlos-launch-")
        self.host = Path(self.temporary.name)
        self.bundle = self.root / "bundle"
        self.bundle.mkdir()
        self.binary = self.bundle / "carlos"
        shutil.copy2(binary, self.binary)
        if companion:
            shutil.copy2(daemon, self.bundle / "carlosd")
        self.terminals, self.owned = [], {}
        self.env = {"HOME":str(self.host), "XDG_RUNTIME_DIR":str(self.host / "runtime"),
                    "XDG_STATE_HOME":str(self.host / "state"), "XDG_CONFIG_HOME":str(self.host / "config")}
        self.socket = self.host / "runtime/carlos/daemon.sock"
        self.native = NativeCodex(self.host)
        self.native.history = lambda:{"id":"native-root", "status":{"type":"idle"}, "turns":[{
            "id":"history", "status":"completed", "items":[{"id":"answer", "type":"agentMessage",
            "phase":"final_answer", "text":"Launcher fixture ready"}]}]}

    def terminal(self, name, resume="native-root", arguments=None, expected=0):
        args = arguments if arguments is not None else ["--codex-socket", str(self.native.path)]
        terminal = Terminal(self.binary, self.root / name, resume=resume, extra_args=args, extra_env=self.env)
        self.terminals.append((terminal, expected))
        return terminal

    def client(self, path=None):
        return daemon_support.Client(SimpleNamespace(root=self.root, socket=path or self.socket,
                                                    memory=lambda _label:None), "launcher-proof")

    def capture(self):
        self.owned.update(capture_children(os.getpid()))

    def daemon_ids(self):
        self.capture()
        terminals = {terminal.pid for terminal, _ in self.terminals}
        return set(remaining(self.owned)) - terminals

    def failed(self, terminal, text):
        terminal.contains(text)
        terminal.wait(lambda:terminal.process.poll() is not None, "launcher error returns before terminal setup")
        assert terminal.process.returncode == 1
        self.capture()

    def fake_companion(self, source):
        path = self.bundle / "carlosd"
        path.write_text(f"#!{sys.executable}\n" + source)
        path.chmod(0o700)

    def identity(self):
        return json.loads(subprocess.check_output([str(self.binary), "--build-info"], timeout=5))

    def close(self):
        self.capture()
        errors = []
        for terminal, expected in reversed(self.terminals):
            try:
                if expected == 0:
                    terminal.close()
                elif not terminal.output.closed:
                    terminal.snapshot("expected-error")
                    terminal.process.wait(timeout=2)
                    os.close(terminal.fd)
                    terminal.output.close()
                    assert terminal.process.returncode == expected
            except BaseException as error:
                errors.append(str(error))
        self.stop_descendants(errors)
        try:
            self.native.close()
        finally:
            trace = self.host / "codex.jsonl"
            if trace.exists():
                shutil.copy2(trace, self.root / "native.jsonl")
            (self.root / "native-state.json").write_text(json.dumps({
                "started":self.native.started, "resumed":self.native.resumed,
                "errors":self.native.errors}, indent=2) + "\n")
            self.temporary.cleanup()
        assert not errors, "\n".join(errors)

    def stop_descendants(self, errors):
        for pid in remaining(self.owned):
            try:
                os.kill(pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
        deadline = time.monotonic() + 5
        while live := remaining(self.owned):
            for pid in live:
                try:
                    os.waitpid(pid, os.WNOHANG)
                except ChildProcessError:
                    pass
            if time.monotonic() >= deadline:
                errors.append(f"test-owned processes ignored shutdown: {sorted(live)}")
                for pid in remaining(live):
                    os.kill(pid, signal.SIGKILL)
                break
            time.sleep(.005)


class GreetingProxy:
    """Forward a real daemon greeting with one incompatible source marker."""

    def __init__(self, path, upstream, record):
        path.parent.mkdir(parents=True, mode=0o700, exist_ok=True)
        self.upstream, self.record, self.errors = upstream, record, []
        self.listener = socket.socket(socket.AF_UNIX)
        self.listener.bind(str(path))
        path.chmod(0o600)
        self.listener.listen(1)
        self.listener.settimeout(3)
        self.thread = threading.Thread(target=self.run, daemon=True)
        self.thread.start()

    def run(self):
        try:
            with self.listener.accept()[0] as device, socket.socket(socket.AF_UNIX) as daemon:
                device.settimeout(3)
                daemon.settimeout(3)
                daemon.connect(str(self.upstream))
                daemon.sendall(device.makefile("rb").readline(65537))
                reply = json.loads(daemon.makefile("rb").readline(65537))
                reply["result"]["identity"]["build"]["source"] = "0" * 32
                self.record.write_text(json.dumps(reply, indent=2) + "\n")
                device.sendall(json.dumps(reply).encode() + b"\n")
        except BaseException as error:
            self.errors.append(repr(error))

    def close(self):
        self.listener.close()
        self.thread.join(timeout=4)
        assert not self.thread.is_alive() and not self.errors, self.errors
