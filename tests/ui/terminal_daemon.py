"""Own one private daemon for opt-in terminal workflows while keeping native RPC logs unchanged."""

import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time

from daemon_recorder import DaemonRecorder
from terminal_daemon_processes import capture_children, finish_children


class OwnedDaemon:
    def __init__(self, binary, root, env, codex_socket=None):
        self.root = root
        self.process = None
        self.recorder = None
        self.closed = False
        self.sockets = tempfile.TemporaryDirectory(prefix="carlos-ui-daemon-")
        directory = Path(self.sockets.name)
        self.socket = directory / "daemon.sock"
        self.endpoint = directory / "terminal.sock"
        self.wire = root / "daemon-wire"
        self.wire.mkdir(exist_ok=True)
        self.log = (root / "daemon.log").open("ab")
        try:
            arguments = [str(binary), "--socket", str(self.socket), "--state-dir", str(root / "daemon-state")]
            if codex_socket is not None:
                arguments += ["--codex-socket", codex_socket]
            self.process = subprocess.Popen(arguments,
                stdin=subprocess.DEVNULL, stdout=self.log, stderr=self.log, env=env,
                cwd=root, start_new_session=True)
            deadline = time.monotonic() + 8
            while not self.socket.exists() and self.process.poll() is None and time.monotonic() < deadline:
                time.sleep(.005)
            assert self.socket.exists() and self.process.poll() is None, self.diagnostic("daemon did not listen")
            self.recorder = DaemonRecorder(self.endpoint, self.socket, self.wire)
        except BaseException:
            self.close()
            raise

    def diagnostic(self, message):
        return message + ": " + str(self.root / "daemon.log")

    def messages(self, direction="client"):
        path = self.wire / "rpc.jsonl"
        if not path.exists():
            return []
        return [record["message"] for line in path.read_bytes().splitlines(keepends=True)
                if line.endswith(b"\n") and (record := json.loads(line))["direction"] == direction]

    def close(self):
        if self.closed:
            return
        self.closed = True
        errors = []
        children = capture_children(self.process.pid) if self.process and self.process.poll() is None else {}
        try:
            if self.recorder:
                self.recorder.close()
        except BaseException as error:
            errors.append(str(error))
        try:
            self.stop_process(errors)
            finish_children(children, errors)
        finally:
            self.log.close()
            self.sockets.cleanup()
        assert not errors, "\n".join(errors)

    def stop_process(self, errors):
        if self.process is None:
            return
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(self.process.pid, signal.SIGKILL)
                self.process.wait(timeout=3)
                errors.append(self.diagnostic("owned daemon ignored SIGTERM"))
        if self.process.returncode != 0:
            errors.append(self.diagnostic(f"owned daemon exited {self.process.returncode}"))
