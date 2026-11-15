"""Reap owned native descendants under a separate supervisor while another process stays alive."""

import argparse
import ctypes
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time
import unittest

import support
from support import Client, Fixture


@unittest.skipUnless(sys.platform.startswith("linux"), "requires the isolated Linux subreaper")
class OwnedProcessTests(unittest.TestCase):
    def test_session_stop_reaps_owned_grok_descendants(self):
        self.check_backend("grok")

    def test_session_stop_reaps_compiled_claude_helper_native_descendants(self):
        self.check_backend("claude")

    def test_session_stop_reaps_owned_codex_descendants(self):
        self.check_backend("codex")

    def check_backend(self, backend):
        directory = support.ARTIFACTS / ("owned-process-" + backend)
        directory.mkdir(parents=True, exist_ok=True)
        completed = subprocess.run([sys.executable, __file__, "--backend", backend,
                                    "--binary", str(support.BINARY), "--artifacts", str(directory)],
                                   capture_output=True, text=True, timeout=15)
        (directory / "supervisor.log").write_text(completed.stdout + completed.stderr)
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)


def wait_descendant(pid, timeout=.5):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            reaped, _ = os.waitpid(pid, os.WNOHANG)
            if reaped == pid:
                return True
        except ChildProcessError:
            pass
        time.sleep(.005)
    return False


def close_captured_child(pid):
    if pid is None:
        return
    try:
        reaped, _ = os.waitpid(pid, os.WNOHANG)
        if reaped == 0:
            # waitpid proves this still belongs to the isolated supervisor before signalling.
            os.kill(pid, signal.SIGKILL)
            os.waitpid(pid, 0)
    except ChildProcessError:
        pass


def supervisor(backend):
    assert ctypes.CDLL(None).prctl(36, 1, 0, 0, 0) == 0, "separate Linux subreaper"
    fixture = Fixture("owned-descendant-" + backend)
    bystander = subprocess.Popen(["sleep", "30"])
    client = None
    descendant = None
    marker = fixture.root / "owned-descendant-pid"
    try:
        shim = fixture.root / "bin" / backend
        text = shim.read_text()
        prefix = "import subprocess\ndescendant=subprocess.Popen(['sleep','30'])\n(root/'owned-descendant-pid').write_text(str(descendant.pid))\n"
        shim.write_text(text.replace("os.execv(sys.executable", prefix + "os.execv(sys.executable"))
        client = Client(fixture, "operator")
        create = client.operation(1)
        session = client.call("session/create", operation=create, backend=backend,
                              cwd=str(fixture.root), name=None)["session"]
        assert client.outcome(create)["state"]["kind"] == "accepted"
        descendant = int(marker.read_text())
        stop = client.operation(2)
        client.call("session/stop", operation=stop, session=session)
        result = client.outcome(stop)
        assert result["state"]["kind"] == "accepted", result
        reaped = wait_descendant(descendant)
        assert bystander.poll() is None, "another process must remain alive"
        report = {"backend": backend, "result": result, "descendant": descendant,
                  "descendant_reaped": reaped, "bystander_alive": True}
        (fixture.root / "owned-process-result.json").write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report), flush=True)
        if reaped:
            descendant = None
        assert reaped, "accepted stop left an owned descendant alive"
    finally:
        if descendant is None and marker.exists():
            descendant = int(marker.read_text())
        if client:
            client.close()
        fixture.close()
        close_captured_child(descendant)
        bystander.terminate()
        bystander.wait(timeout=2)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--backend", choices=("grok", "claude", "codex"), required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path, required=True)
    args = parser.parse_args()
    support.BINARY = args.binary.resolve()
    support.ARTIFACTS = args.artifacts.resolve()
    supervisor(args.backend)
