#!/usr/bin/env python3
"""Run the actual iOS UI against a private daemon and native Codex peer, without inference."""

import argparse
from contextlib import contextmanager, ExitStack
import hashlib
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
from native_task_questions import callback_key, form as native_form, question
from ios_browser_fixture import browser_page
from ios_history_fixture import configure_history, history_control
from ios_ralph_fixture import configure_ralph
from ios_syntax_fixture import configure_syntax
from ios_reconnect_fixture import reconnect_relay, check_reconnect


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


def configure_controls(native, *, stream_output=False):
    """Complete only after the native peer receives the captured original Stop turn."""
    original = native.handle

    async def handle(link, request):
        await original(link, request)
        if request["method"] == "turn/interrupt":
            assert request["params"]["turnId"] == "live", request
            if stream_output:
                await native.notify(link, "item/commandExecution/outputDelta", turnId="live",
                                    itemId="live-tool", delta="chunk-b after Stop 🌍\n")
            await native.notify(link, "turn/completed", turn={"id": "live", "status": "interrupted"})

    native.handle = handle


def configure_source(native, *, code=False):
    """Publish a source larger than a client frame; the phone must fetch every original byte."""
    source = "# Native source\n\n" + ("native paragraph " + "x" * 65500 + " 🌍\n\n") * 160
    if code:
        source = "# Native code\n\n```rust\n" + "".join(f"let value_{line} = {line}; // 🦀\n" for line in range(10000)) + "```\n\n"
    source += "phone source complete 🦀\n"
    submitted = "phone large code" if code else "phone long source"
    original = native.prompt

    async def prompt(link, request):
        if request["params"]["input"][0]["text"] != submitted:
            await original(link, request)
            return
        native.prompts += 1
        await native.notify(link, "turn/started", turn={"id": "live"})
        await native.notify(link, "item/started", turnId="live", item={
            "type": "agentMessage", "id": "phone-large-source", "phase": "final_answer", "text": ""})
        # Real native streams keep each frame bounded too; do not bypass its 8-MiB guard.
        for offset in range(0, len(source), 16384):
            await native.notify(link, "item/agentMessage/delta", turnId="live",
                                itemId="phone-large-source", delta=source[offset:offset + 16384])
        await native.notify(link, "turn/completed", turn={"id": "live", "status": "completed"})

    native.prompt = prompt
    raw = source.encode()
    return {"sourceSha256": hashlib.sha256(raw).hexdigest(), "sourceBytes": len(raw)}


def configure_stream(native, *, code=False):
    """Hold one native answer until the original Stop, without approvals or fake UI state."""
    configure_controls(native)
    original = native.prompt
    answer = "Streaming paragraph 🌍" + ("\n\n```rust\nlet count = 42;\n" if code else "")

    async def prompt(link, request):
        if request["params"]["input"][0]["text"] != "phone stream":
            await original(link, request)
            return
        native.prompts += 1
        await native.notify(link, "turn/started", turn={"id": "live"})
        await native.notify(link, "item/started", turnId="live", item={
            "type": "agentMessage", "id": "phone-stream", "phase": "final_answer", "text": ""})
        await native.notify(link, "item/agentMessage/delta", turnId="live", itemId="phone-stream",
                            delta=answer)

    native.prompt = prompt
    return {"streamText": answer, "streamLabel": "let count = 42;\n" if code else answer}


def configure_questions(native, *, form_request=False, browser_url=None):
    """Reply only to the exact original native question, then publish its actual completion."""
    original_prompt = native.prompt
    original_response = native.task_question_response
    kind = "browser" if browser_url else "form" if form_request else "questions"
    identity = f"phone:{kind}:α"
    prompt_text = f"phone {kind}"
    result_text = {"browser": "Browser consent accepted 🦀", "form": "Phone form accepted 🦀",
                   "questions": "Phone answers accepted 🦀"}[kind]

    async def prompt(link, request):
        if request["params"]["input"][0]["text"] != prompt_text:
            await original_prompt(link, request)
            return
        native.prompts += 1
        await native.notify(link, "turn/started", turn={"id": "live"})
        frame = native_form(identity, turn="live") if form_request else question(identity, thread=native.thread_id, turn="live", blocking=True)
        if browser_url:
            frame = native_form(identity, mode="url")
            frame["params"]["url"] = browser_url
            await native.notify(link, "turn/completed", turn={"id": "live", "status": "completed"})
        frame["params"]["threadId"] = native.thread_id
        native.task_callbacks[callback_key(identity)] = frame
        native.task_callback_ids.add(callback_key(identity))
        await native.send(link, frame)

    async def response(link, answer):
        handled = await original_response(link, answer)
        if handled and answer.get("id") == identity:
            await native.notify(link, "serverRequest/resolved", requestId=identity)
            await native.notify(link, "item/agentMessage/delta", turnId="live", itemId="phone-question-result",
                                delta=result_text)
            await native.notify(link, "turn/completed", turn={"id": "live", "status": "completed"})
        return handled

    native.prompt = prompt
    native.task_question_response = response


def configure_workflow(fixture, workflow, artifacts, resources):
    """Keep each deterministic native scenario and its owned resources together."""
    source, visits = {}, None
    if workflow == "controls":
        configure_controls(fixture.native)
    elif workflow == "history":
        visits = configure_history(fixture)
        port = resources.enter_context(history_control(visits))
        directory = artifacts / "history"
        directory.mkdir(mode=0o700)
        remote = resources.enter_context(forward(port, directory))
        source["historyControlURL"] = f"http://127.0.0.1:{remote}/release"
    elif workflow == "ralph":
        configure_controls(fixture.native)
        configure_ralph(fixture)
    elif workflow == "syntax":
        source = configure_syntax(fixture.native)
    elif workflow == "reconnect":
        visits = resources.enter_context(reconnect_relay(fixture))
        source = {"relayPort": visits.port, "relayControlPath": visits.control}
    elif workflow == "tools":
        configure_controls(fixture.native, stream_output=True)
    elif workflow in ("source", "code-source"):
        source = configure_source(fixture.native, code=workflow == "code-source")
    elif workflow in ("stream", "scroll", "code-stream"):
        source = configure_stream(fixture.native, code=workflow == "code-stream")
        if workflow == "scroll":
            original_history = fixture.native.history

            def history():
                thread = original_history()
                thread["turns"][0]["items"][-1]["aggregatedOutput"] = "".join(f"anchor-output {line}\n" for line in range(40))
                return thread

            fixture.native.history = history
    elif workflow == "questions":
        configure_questions(fixture.native)
    elif workflow == "form":
        configure_questions(fixture.native, form_request=True)
    elif workflow == "browser":
        port, visits = resources.enter_context(browser_page(fixture.native))
        directory = artifacts / "browser"
        directory.mkdir(mode=0o700)
        remote = resources.enter_context(forward(port, directory))
        configure_questions(fixture.native, browser_url=f"http://127.0.0.1:{remote}/complete")
    return source, visits


def observed_prompts(fixture, backend):
    """Read the original native peer's traffic for each actual backend protocol."""
    if backend == "codex":
        return [request["params"]["input"][0]["text"] for request in fixture.native.requests
                if request.get("method") == "turn/start"]
    if backend == "grok":
        return [request["params"]["prompt"][0]["text"] for request in fixture.native_requests("session/prompt")]
    records = [json.loads(line) for line in (fixture.root / "rpc.jsonl").read_text().splitlines()]
    contents = [record["message"]["message"]["content"] for record in records
                if record["direction"] == "client" and record["message"].get("type") == "user"]
    return [content if isinstance(content, str) else "".join(block["text"] for block in content)
            for content in contents]


def check_native(fixture, backend, workflow, observed, visits):
    """Prove exact human text and original native callbacks, not only a plausible screen."""
    expected = {"controls": ["held"], "tools": ["held"], "source": ["phone long source"], "code-source": ["phone large code"], "stream": ["phone stream"], "code-stream": ["phone stream"], "scroll": ["phone stream"], "ralph": ["phone ralph working"], "syntax": ["phone syntax"],
                "voice": [], "history": [], "reconnect": ["phone reconnect\nexact 🦀"], "keyboard": ["phone\nkeyboard\nexact 🦀"], "prompt": ["phone exact prompt 🦀"], "relaunch": [], "credentials": [], "token-update": ["phone exact prompt 🦀", "phone token restart"],
                "questions": ["phone questions"], "form": ["phone form"], "browser": ["phone browser"]}
    assert observed == expected[workflow], observed
    if backend != "codex":
        assert [process["backend"] for process in fixture.pids()] == [backend], fixture.pids()
        return
    native = fixture.native
    if workflow == "reconnect":
        check_reconnect(fixture, visits)
    if workflow == "history":
        assert native.started == 0 and native.resumed == 1
        assert len(visits.pages) == 3, visits.pages
        assert [page.get("cursor") for page in visits.pages] == [None, "PRIVATE_HISTORY:desc:100", "PRIVATE_HISTORY:desc:100"]
    if workflow in ("relaunch", "token-update"):
        assert native.started == 1, "Relaunch must not create another native session"
    elif workflow == "credentials":
        assert native.started == 0, "Connecting must not create a native session"
    if workflow in ("controls", "tools"):
        assert native.answers == 1 and native.interrupts == 1
        answers = [request for request in native.requests if request.get("id") == "card"]
        assert len(answers) == 1 and answers[0]["result"]["decision"] == "accept", answers
    elif workflow in ("stream", "scroll", "ralph", "code-stream"):
        assert native.interrupts == 1 and native.answers == 0
    elif workflow == "questions":
        assert native.task_answers == [{"id": "phone:questions:α", "result": {"answers": {
            "field-str": {"answers": ["Listed value", "user_note: phone note 🦀"]}}}}], native.task_answers
    elif workflow == "form":
        assert native.task_answers == [{"id": "phone:form:α", "result": {
            "action": "accept", "content": {"count": 5, "enabled": False, "mode": "fr"}}}], native.task_answers
    elif workflow == "browser":
        assert visits and all(visit == {"answers_before_open": 0} for visit in visits), visits
        assert native.task_answers == [{"id": "phone:browser:α", "result": {"action": "accept"}}], native.task_answers
        assert native.interrupts == 0, "A browser request without a turn cannot offer an enabled Stop"
    assert not native.errors, native.errors


def run(workflow, backend="codex"):
    """Keep the real daemon and native peer alive through XCTest, then verify its traffic."""
    identity = "phone-" + backend + "-" + uuid.uuid4().hex[:12]
    artifacts = ROOT / "target/ios-artifacts" / identity
    artifacts.mkdir(parents=True)
    fixture = Fixture(identity, codex=backend == "codex", mode="claude-normal" if backend == "claude" else "modern",
                      listen="127.0.0.1:0")
    resources = ExitStack()
    try:
        source, visits = configure_workflow(fixture, workflow, artifacts, resources)
        token = grant(fixture, "native-phone")
        port = source.pop("relayPort", int(fixture.ws_uri.rsplit(":", 1)[1]))
        with forward(port, artifacts) as remote:
            if workflow == "reconnect":
                source["reconnectControlURL"] = f"http://127.0.0.1:{remote}{source.pop('relayControlPath')}"
            config = {"url": f"ws://127.0.0.1:{remote}", "token": token, "backend": backend,
                      "cwd": str(fixture.root), "workflow": workflow, **source}
            inputs = artifacts / "inputs"
            inputs.mkdir(mode=0o700)
            (inputs / "fixture.json").write_text(json.dumps(config))
            (inputs / "fixture.json").chmod(0o600)
            subprocess.run(["bash", "scripts/ios-mac.sh", "sync", str(inputs), f"fixture/{identity}"], cwd=ROOT, check=True)
            result = subprocess.run(["bash", "scripts/ios-build.sh", "ui-test", f"fixture/{identity}"], cwd=ROOT)
            observed = observed_prompts(fixture, backend)
            (artifacts / "native-prompts.json").write_text(json.dumps(observed, ensure_ascii=False) + "\n")
            if result.returncode:
                raise RuntimeError(f"Actual phone UI test failed; native evidence: {artifacts}")
            check_native(fixture, backend, workflow, observed, visits)
            print(f"Actual phone {backend} {workflow} screen and native traffic passed: {artifacts}")
    finally:
        resources.close()
        fixture.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workflow", choices=("prompt", "controls", "source", "relaunch", "credentials", "tools", "questions", "form", "browser", "stream", "token-update", "keyboard", "voice", "scroll", "history", "ralph", "syntax", "reconnect", "code-source", "code-stream"), default="prompt")
    parser.add_argument("--backend", choices=("codex", "claude", "grok"), default="codex")
    arguments = parser.parse_args()
    if arguments.backend != "codex" and arguments.workflow != "prompt":
        parser.error("Non-Codex peers currently support the exact prompt workflow")
    run(arguments.workflow, arguments.backend)
