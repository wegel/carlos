"""Recover the shared Claude helper explicitly while each terminal keeps its own draft."""

import json
from pathlib import Path
import sys

from test_daemon_client import daemon_support


class DaemonClaudeReopen:
    def test_daemon_explicit_resume_reopens_failed_fresh_claude_branch_and_preserves_closed_draft(self):
        runner = sys.modules[type(self).__module__]
        daemon_support.BINARY = runner.BINARY.parent / "carlosd"
        daemon_support.ARTIFACTS = runner.ARTIFACTS / "daemon"
        fixture = daemon_support.Fixture("terminal-claude-reopen", mode="claude-fork-start-failure")
        self.addCleanup(fixture.close)
        client = daemon_support.Client(fixture, "observer")
        self.addCleanup(client.close)
        create = client.operation(1)
        session = client.call("session/create", operation=create, backend="claude", cwd=str(fixture.root), name="Claude recovery")["session"]
        self.assertEqual(client.outcome(create)["state"]["kind"], "accepted")
        first = self.attach_daemon(fixture, session, "claude-first")
        first.contains("Idle")
        first.paste("first original Claude prompt")
        first.send(b"\r")
        first.contains("Answer: first original Claude prompt")
        first.rewind("first original Claude prompt")
        first.replace_input("edited text stays private 🦊")
        first.send(b"\r")
        first.wait(lambda: first.input_is("edited text stays private 🦊"), "failed replacement returns the edited text")
        first.contains("Claude is closed")
        self.assertEqual(len(fixture.pids()), 2)
        subscriptions = len(first.requests("session/subscribe"))
        first.send(b"\x1b[15~")
        first.contains("reopen this saved session")
        self.assertNotIn("Waiting for the session to become ready", first.text)
        self.assertTrue(first.input_is("edited text stays private 🦊"))
        self.assertEqual(len(first.requests("session/subscribe")), subscriptions,
                         "F5 cannot make a closed Claude helper ready by subscribing")
        self.assertEqual(len(fixture.pids()), 2, "no implicit native restart or resend")
        (fixture.root / "first-native-session").unlink()
        second = self.attach_daemon(fixture, session, "claude-reopened")
        second.contains("Idle")
        first.wait(lambda: first.input_is("edited text stays private 🦊") and "Claude is closed" not in first.text,
                   "same owner becomes ready without consuming the first terminal's draft")
        self.assertTrue(second.input_is(""))
        self.assertEqual(len(fixture.pids()), 3)
        self.assertEqual(len(second.requests("session/open")), 1)
        self.assertEqual(second.requests("session/send"), [])
        self.assertEqual(len(first.requests("session/rewind")), 1)
        trace = Path(fixture.root / "claude-controller.jsonl")
        requests = [row["message"] for line in trace.read_text().splitlines()
                    if (row := json.loads(line))["direction"] == "client"]
        starts = [request["params"] for request in requests if request["method"] == "start"]
        self.assertEqual(starts[1]["sessionId"], starts[2]["sessionId"])
        self.assertNotIn("resume", starts[2])
        self.assertEqual(len([request for request in requests if request["method"] == "prompt"]), 1)
        first.send(b"\r")
        first.contains("Answer: edited text stays private 🦊")
        second.contains("Answer: edited text stays private 🦊")
        self.assertEqual(len(first.requests("session/subscribe")), subscriptions,
                         "F5 on the closed session did not add a subscription")
        self.assertEqual(len(first.requests("session/send")), 2, "only a new Enter sends recovered text")
        self.assertEqual(second.requests("session/send"), [])
        first.snapshot("failed-edit-draft-explicit-claude-open")
        second.snapshot("same-logical-owner-new-helper")
