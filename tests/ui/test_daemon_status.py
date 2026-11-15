"""Check status labels through the actual socket terminal and deterministic native peers."""

import argparse
import asyncio
import hashlib
import json
from pathlib import Path
import tempfile
import time
import unittest

from terminal import Terminal
from test_daemon_client import DaemonClient, daemon_support


class DaemonStatus(unittest.TestCase):
    daemon_session = DaemonClient.daemon_session
    attach_daemon = DaemonClient.attach_daemon

    def summary_root(self):
        root = ARTIFACTS / self._testMethodName
        root.mkdir(parents=True, exist_ok=True)
        return root

    def launch(self, directory, width, resume, extra_args, extra_env=None):
        terminal = Terminal(BINARY, directory, resume=resume, width=width,
                            extra_args=extra_args, extra_env=extra_env)
        self.addCleanup(terminal.close)
        return terminal

    @staticmethod
    def status_history():
        return {"id":"native-root", "status":{"type":"idle"},
                "model":"fixture-model", "reasoningEffort":"low", "turns":[{
                    "id":"history", "status":"completed", "items":[{
                        "id":"status-ready", "type":"agentMessage", "phase":"final_answer",
                        "text":"Status fixture ready"}]}]}

    def quota_session(self, name):
        def configure(fixture):
            original = fixture.native.handle
            fixture.quota_reads = []
            async def handle(link, request):
                if request.get("method") == "account/rateLimits/read":
                    fixture.quota_reads.append((link, request["id"]))
                    return
                await original(link, request)
            fixture.native.handle = handle
        fixture, client, session = self.daemon_session(name, self.status_history, configure)
        daemon_support.wait_for(lambda:fixture.quota_reads, "daemon starts one optional quota read")
        return fixture, client, session

    def native_frames(self, fixture, method, **params):
        async def send():
            for link in list(fixture.native.connections):
                await fixture.native.notify(link, method, **params)
        asyncio.run_coroutine_threadsafe(send(), fixture.native.loop).result(timeout=3)

    def quota_reply(self, fixture, used=60, days=2.1):
        reset = int(time.time()) + round(days * 86400)
        async def send():
            link, identity = fixture.quota_reads[0]
            await fixture.native.send(link, {"id":identity, "result":{"rateLimits":{
                "limitId":"codex", "secondary":{"usedPercent":used,
                "windowDurationMins":10080, "resetsAt":reset}}}})
        asyncio.run_coroutine_threadsafe(send(), fixture.native.loop).result(timeout=3)
        return reset

    def assert_weekly(self, terminal, label, color="89b4fa"):
        terminal.wait(lambda:terminal.screen.display[-2].rstrip().endswith(label),
                      "weekly allowance stays at the right edge")
        row = terminal.screen.lines - 2
        start = terminal.screen.columns - 1 - len(label)
        self.assertEqual(terminal.screen.buffer[row][start].fg, "6c7086")
        for column in range(start + 1, start + label.index("/")):
            self.assertEqual(terminal.screen.buffer[row][column].fg, color)

    def test_codex_withheld_quota_keeps_two_drafts_private_then_resizes_the_shared_footer(self):
        fixture, client, session = self.quota_session("status-codex-held")
        first = self.attach_daemon(fixture, session, "status-first", width=100)
        second = self.attach_daemon(fixture, session, "status-second", width=100)
        for terminal, draft in [(first, "first 🌍"), (second, "second 日本語")]:
            terminal.contains("Status fixture ready")
            terminal.paste(draft)
            terminal.wait(lambda:terminal.input_is(draft), "private draft during held native quota read")
            self.assertNotIn("W100%", terminal.screen.display[-2])
        self.quota_reply(fixture)
        self.native_frames(fixture, "thread/tokenUsage/updated", tokenUsage={
            "modelContextWindow":100000, "total":{"totalTokens":91000}})
        for terminal in (first, second):
            self.assert_weekly(terminal, "W40%/2.1d")
            terminal.contains("91k/100k 91%")
            terminal.contains("fixture-model/low")
            column = terminal.screen.display[-2].index("91%")
            self.assertEqual(terminal.screen.buffer[terminal.screen.lines - 2][column].fg, "f38ba8")
        for width in (54, 40, 32, 18, 12, 8, 100):
            first.resize(width, 32)
            if width >= 12:
                self.assert_weekly(first, "W40%/2.1d")
            if width == 40:
                first.contains("91k/100k 91%")
            first.snapshot(f"weekly-{width}")
        self.assertTrue(first.input_is("first 🌍"))
        self.assertTrue(second.input_is("second 日本語"))
        self.assertEqual(len(fixture.quota_reads), 1, "two devices share the daemon's native timer")
        for terminal in (first, second):
            self.assertEqual(terminal.requests("session/send"), [])
            self.assertEqual(terminal.requests("task/subscribe"), [])
            self.assertEqual(terminal.requests("account/rateLimits/read"), [])

    def test_codex_sparse_quota_updates_keep_the_bucket_and_expired_values_disappear(self):
        fixture, client, session = self.quota_session("status-codex-updates")
        terminal = self.attach_daemon(fixture, session, "status-updates", width=100)
        terminal.contains("Status fixture ready")
        reset = self.quota_reply(fixture)
        self.assert_weekly(terminal, "W40%/2.1d")
        self.native_frames(fixture, "account/rateLimits/updated", rateLimits={
            "limitId":"other-model", "secondary":{"usedPercent":99,
            "windowDurationMins":10080, "resetsAt":reset}})
        self.native_frames(fixture, "account/rateLimits/updated", rateLimits={
            "limitId":"codex", "secondary":{"usedPercent":90}})
        self.assert_weekly(terminal, "W10%/2.1d", "f38ba8")
        self.native_frames(fixture, "account/rateLimits/updated", rateLimits={
            "limitId":"codex", "secondary":{"usedPercent":90,
            "windowDurationMins":10080, "resetsAt":int(time.time()) - 1}})
        terminal.wait(lambda:"W10%" not in terminal.screen.display[-2], "expired quota hides rather than invents allowance")
        terminal.paste("still private")
        terminal.wait(lambda:terminal.input_is("still private"), "quota changes keep input editable")
        self.assertEqual(len(fixture.quota_reads), 1)
        self.assertEqual(terminal.requests("session/send"), [])

    def claude_session(self, name, mode="modern"):
        daemon_support.BINARY = BINARY.parent / "carlosd"
        daemon_support.ARTIFACTS = ARTIFACTS / "daemon"
        fixture = daemon_support.Fixture(name, mode=mode)
        self.addCleanup(fixture.close)
        client = daemon_support.Client(fixture, "status-owner")
        self.addCleanup(client.close)
        operation = client.operation(1)
        session = client.call("session/create", operation=operation, backend="claude",
                              cwd=str(fixture.root), name="Claude status")["session"]
        client.outcome(operation)
        daemon_support.wait_for(lambda:client.call("session/subscribe", session=session, since=None)
                                ["snapshot"]["info"]["context"] is not None, "native initial context count")
        terminal = self.attach_daemon(fixture, session, name, width=140)
        terminal.contains("32k/123k")
        return fixture, client, session, terminal

    @staticmethod
    def helper_requests(fixture, method):
        path = fixture.root / "claude-controller.jsonl"
        return [row["message"] for line in path.read_text().splitlines()
                if (row := json.loads(line))["direction"] == "client"
                and row["message"].get("method") == method]

    def test_claude_native_quota_and_activity_use_the_main_footer_without_task_interest(self):
        fixture, client, session, terminal = self.claude_session("status-claude-quota")
        terminal.paste("native quotas")
        terminal.send(b"\r")
        terminal.contains("Quota received")
        self.assert_weekly(terminal, "W60%/1.0d")
        self.assertEqual(terminal.requests("task/subscribe"), [])
        terminal.paste("private quota draft 🌍")
        terminal.send(b"\x1bOQ")
        terminal.contains("five hour: 25.0% used")
        terminal.contains("seven day: 40.0% used")
        terminal.send(b"\x1bOQ")
        terminal.wait(lambda:terminal.input_is("private quota draft 🌍"), "task panel keeps quota draft")
        self.assertEqual([request["params"]["text"] for request in self.helper_requests(fixture, "prompt")], ["native quotas"])

    def test_claude_stale_context_thinking_and_deadline_refresh_keep_private_input(self):
        fixture, client, session, terminal = self.claude_session("status-claude-stale", "claude-context-retry")
        terminal.paste("native thinking")
        terminal.send(b"\r")
        terminal.contains("Thinking ~43 tokens")
        terminal.contains("~32k/123k")
        terminal.paste("private stale draft 🌍")
        terminal.wait(lambda:terminal.input_is("private stale draft 🌍"), "input stays live after an optional context failure")
        terminal.wait(lambda:"42k/123k" in terminal.text, "native context retry finishes", timeout=12)
        terminal.wait(lambda:"~42k/123k" not in terminal.screen.display[-2], "matching native reply clears stale marker")
        self.assertTrue(terminal.input_is("private stale draft 🌍"))
        self.assertEqual([request["params"]["text"] for request in self.helper_requests(fixture, "prompt")], ["native thinking"])
        self.assertEqual(terminal.requests("task/subscribe"), [])
        terminal.snapshot("fresh-count-after-native-retry")

    def test_claude_explicit_text_keeps_compaction_failures_success_and_context_notices_once(self):
        fixture, client, session, first = self.claude_session("status-claude-compact")
        second = self.attach_daemon(fixture, session, "status-compact-second", width=140)
        second.paste("private compaction draft 日本語")
        first.paste("native compact")
        first.send(b"\r")
        for terminal in (first, second):
            terminal.contains("Fixture compaction refused")
            terminal.contains("Claude compacted context")
            terminal.contains("900/123k")
            self.assertEqual(terminal.text.count("Claude compacted context"), 1)
        self.assertTrue(second.input_is("private compaction draft 日本語"))
        self.assertEqual([request["params"]["text"] for request in self.helper_requests(fixture, "prompt")], ["native compact"])
        self.assertEqual(second.requests("session/send"), [])
        first.snapshot("native-compaction-once")


def identity(path):
    return {"path":str(path.resolve()), "sha256":hashlib.sha256(path.read_bytes()).hexdigest()}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--daemon", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path)
    args, tests = parser.parse_known_args()
    ARTIFACTS = (args.artifacts or Path(tempfile.mkdtemp(prefix="carlos-status-ui-"))).resolve()
    ARTIFACTS.mkdir(parents=True, exist_ok=True)
    pair = ARTIFACTS / "executables"
    pair.mkdir()
    BINARY = pair / "carlos"
    BINARY.symlink_to(args.binary.resolve())
    (pair / "carlosd").symlink_to(args.daemon.resolve())
    (ARTIFACTS / "executables.json").write_text(json.dumps({
        "terminal":identity(args.binary), "daemon":identity(args.daemon),
        "test":identity(Path(__file__))}, indent=2) + "\n")
    unittest.main(argv=[__file__, *tests])
