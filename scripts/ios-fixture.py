#!/usr/bin/env python3
"""Run the actual iOS UI against a private daemon and native Codex peer, without inference."""

import argparse
from contextlib import contextmanager
import json
import os
from pathlib import Path
import re
import selectors
import subprocess
import sys
import time
import uuid


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests/daemon"))
from network_support import grant
from support import Fixture


def mac_settings():
    """Read only the SSH fields needed by this owned, temporary port forward."""
    path = os.environ.get("MAC_BUILD_ENV", str(ROOT / "etc/mac-build.env"))
    values = subprocess.check_output([
        "bash", "-c", 'source "$1"; printf "%s\\0%s\\0" "$MAC_BUILD_HOST" "${MAC_SSH_KEY:-}"',
        "ios-settings", path,
    ], cwd=ROOT).decode().split("\0")
    host, key = values[:2]
    arguments = ["ssh", "-o", "ControlMaster=no", "-o", "StrictHostKeyChecking=no",
                 "-o", "UserKnownHostsFile=/dev/null", "-o", "LogLevel=INFO",
                 "-o", "ConnectTimeout=10", "-o", "ExitOnForwardFailure=yes"]
    if key:
        arguments += ["-i", key]
    return arguments, host


def allocated_port(process, log):
    """Read OpenSSH's allocated remote port without exposing private host details."""
    with selectors.DefaultSelector() as selector:
        selector.register(process.stderr, selectors.EVENT_READ)
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            if process.poll() is not None:
                break
            for key, _ in selector.select(timeout=.2):
                line = key.fileobj.readline()
                log.write(line)
                log.flush()
                match = re.search(r"Allocated port (\d+) for remote forward", line)
                if match:
                    return int(match[1])
    raise RuntimeError("Cannot open the owned Mac port forward; see its private SSH log")


@contextmanager
def forward(local_port, artifacts):
    """Bind only Mac loopback and remove only this process's forward on exit."""
    arguments, host = mac_settings()
    process = subprocess.Popen(
        arguments + ["-N", "-T", "-R", f"127.0.0.1:0:127.0.0.1:{local_port}", host],
        stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE,
        text=True, bufsize=1,
    )
    try:
        with (artifacts / "ssh-forward.log").open("w") as log:
            yield allocated_port(process, log)
    finally:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
        process.stderr.close()


def configure_controls(native):
    """Complete only after the native peer receives the captured original Stop turn."""
    original = native.handle

    async def handle(link, request):
        await original(link, request)
        if request["method"] == "turn/interrupt":
            assert request["params"]["turnId"] == "live", request
            await native.notify(link, "turn/completed", turn={"id": "live", "status": "interrupted"})

    native.handle = handle


def run(workflow):
    """Keep the fixture alive through XCTest, then assert actual native traffic."""
    identity = "phone-" + uuid.uuid4().hex[:12]
    artifacts = ROOT / "target/ios-artifacts" / identity
    artifacts.mkdir(parents=True)
    fixture = Fixture(identity, codex=True, listen="127.0.0.1:0")
    try:
        if workflow == "controls":
            configure_controls(fixture.native)
        token = grant(fixture, "native-phone")
        local_port = int(fixture.ws_uri.rsplit(":", 1)[1])
        with forward(local_port, artifacts) as remote_port:
            config = {"url": f"ws://127.0.0.1:{remote_port}", "token": token,
                      "cwd": str(fixture.root), "workflow": workflow}
            inputs = artifacts / "inputs"
            inputs.mkdir(mode=0o700)
            (inputs / "fixture.json").write_text(json.dumps(config))
            (inputs / "fixture.json").chmod(0o600)
            subprocess.run(["bash", "scripts/ios-mac.sh", "sync", str(inputs), f"fixture/{identity}"],
                           cwd=ROOT, check=True)
            result = subprocess.run(["bash", "scripts/ios-build.sh", "ui-test", f"fixture/{identity}"], cwd=ROOT)
            requests = [request for request in fixture.native.requests if request.get("method") == "turn/start"]
            observed = [request["params"]["input"][0]["text"] for request in requests]
            (artifacts / "native-prompts.json").write_text(json.dumps(observed, ensure_ascii=False) + "\n")
            if result.returncode:
                raise RuntimeError(f"Actual phone UI test failed; native evidence: {artifacts}")
            expected = "held" if workflow == "controls" else "phone exact prompt 🦀"
            assert observed == [expected], observed
            if workflow == "controls":
                assert fixture.native.answers == 1 and fixture.native.interrupts == 1
                answers = [request for request in fixture.native.requests if request.get("id") == "card"]
                assert len(answers) == 1 and answers[0]["result"]["decision"] == "accept", answers
            assert not fixture.native.errors, fixture.native.errors
            print(f"Actual phone {workflow} screen and native traffic passed: {artifacts}")
    finally:
        fixture.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workflow", choices=("prompt", "controls"), default="prompt")
    run(parser.parse_args().workflow)
