"""Check a packaged Unix daemon without native sessions, network calls or user state."""

import json
import os
from pathlib import Path
import socket
import stat
import subprocess
import tempfile
import time


def greeting(path):
    with socket.socket(socket.AF_UNIX) as link:
        link.settimeout(2)
        link.connect(str(path))
        link.sendall(json.dumps({"jsonrpc":"2.0", "id":1, "method":"hello", "params":{
            "protocol":1, "client":"package-smoke", "token":""}}).encode() + b"\n")
        reply = json.loads(link.makefile("rb").readline(65537))
        if "error" in reply:
            raise AssertionError(reply)
        return reply["result"]


def check(binary, daemon, artifacts):
    expected = json.loads(subprocess.check_output([str(binary), "--build-info"], timeout=5))
    actual = json.loads(subprocess.check_output([str(daemon), "--build-info"], timeout=5))
    assert expected == actual, "packaged terminal and daemon differ"
    artifacts.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="carlos-package-daemon-") as directory:
        root = Path(directory)
        endpoint, state = root / "daemon.sock", root / "state"
        env = {**os.environ, "HOME":str(root), "XDG_CONFIG_HOME":str(root / "config"),
               "XDG_STATE_HOME":str(root / "host-state"), "PATH":""}
        with (artifacts / "daemon.log").open("wb") as log:
            process = subprocess.Popen([str(daemon), "--socket", str(endpoint), "--state-dir", str(state)],
                cwd=root, env=env, stdin=subprocess.DEVNULL, stdout=log, stderr=log)
            try:
                deadline = time.monotonic() + 5
                while not endpoint.exists() and process.poll() is None and time.monotonic() < deadline:
                    time.sleep(.005)
                assert process.poll() is None and endpoint.exists(), "packaged daemon did not listen"
                first, second = greeting(endpoint), greeting(endpoint)
                assert first["identity"]["build"] == expected
                assert first["instance"] == second["instance"], "device disconnect stopped daemon"
                assert stat.S_IMODE(endpoint.stat().st_mode) == 0o600
                assert stat.S_IMODE(state.stat().st_mode) == 0o700
                (artifacts / "identity.json").write_text(json.dumps(first, indent=2) + "\n")
            finally:
                if process.poll() is None:
                    process.terminate()
                process.wait(timeout=5)
            assert process.returncode == 0, "packaged daemon failed during shutdown"
        assert not endpoint.exists(), "owned socket survived normal daemon shutdown"
