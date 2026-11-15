#!/usr/bin/env python3
"""Exercise the real headless executable and native adapters without an account or inference."""

import argparse
import json
from pathlib import Path
import unittest

import support
from support import Client, Fixture
from test_recovery import RecoveryTests
from test_failures import FailureTests
from test_source import SourceTests
from test_edits import EditTests
from test_goals import GoalTests
from test_queue import QueueTests
from test_tasks import TaskTests
from test_task_interests import TaskInterestTests
from test_task_stops import TaskStopTests
from test_task_windows import TaskWindowTests
from test_task_activity import TaskActivityTests
from test_task_questions import TaskQuestionTests
from test_settings_facts import SettingsFactTests
from test_settings_changes import SettingsChangeTests
from test_backend_settings import BackendSettingsTests
from test_tool_labels import ToolLabelTests
from test_startup_launch import StartupLaunchTests
from test_startup_open import StartupOpenTests
from test_startup_grok import StartupGrokTests
from test_startup_archive import StartupArchiveTests
from test_startup_reconnect import StartupReconnectTests
from test_claude_reopen import ClaudeReopenTests
from test_context_defaults import ContextDefaultTests
from test_ralph import RalphTests
from test_prompt_history import PromptHistoryTests


class DaemonTests(PromptHistoryTests, RalphTests, TaskQuestionTests, ContextDefaultTests, ToolLabelTests, BackendSettingsTests, TaskActivityTests, SettingsChangeTests, TaskWindowTests, SettingsFactTests, TaskInterestTests, FailureTests, RecoveryTests, SourceTests, EditTests, GoalTests, QueueTests, TaskTests, TaskStopTests, unittest.TestCase):
    def test_codex_reads_controls_fifo_and_independent_output_cursors_while_send_waits(self):
        fixture = Fixture("codex-held", codex=True)
        self.addCleanup(fixture.close)
        first = Client(fixture, "expanded")
        second = Client(fixture, "collapsed")
        self.addCleanup(first.close)
        self.addCleanup(second.close)
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="codex", cwd=str(fixture.root), name="Headless alias")["session"]
        self.assertEqual(first.outcome(create)["state"]["kind"], "accepted")
        # Native startup reads can publish settings between two independent snapshots.
        def common_snapshot():
            a = first.call("session/subscribe", session=session, since=None)["snapshot"]
            b = second.call("session/subscribe", session=session, since=None)["snapshot"]
            return (a, b) if a["seq"] == b["seq"] else None

        a, b = support.wait_for(common_snapshot, "both readers reach the same shared cursor")
        self.assertEqual(a, b)
        self.assertEqual(a["info"]["name"], "Headless alias")
        self.assertTrue(a["has_older"])
        history = second.call("session/history", session=session, before=a["entries"][0]["id"], limit=20)
        self.assertEqual(len(history["entries"]), 20)
        held = first.operation(2)
        first.call("session/send", session=session, operation=held, text="held")
        queued = second.operation(1)
        second.call("session/send", session=session, operation=queued, text="second")
        card = first.message(lambda m: m.get("method") == "session/event" and m["params"]["event"]["kind"] == "approval_requested")["params"]["event"]["approval"]
        turn = first.message(lambda m: m.get("method") == "session/event" and m["params"]["event"]["kind"] == "info_changed" and m["params"]["event"]["info"]["current_turn"])["params"]["event"]["info"]["current_turn"]
        tool = first.message(lambda m: m.get("method") == "session/event" and m["params"]["event"]["kind"] in ("entry_added", "entry_updated") and (m["params"]["event"]["entry"].get("tool") or {}).get("has_output"))["params"]["event"]["entry"]["id"]
        chunk = first.call("entry/output/subscribe", session=session, entry=tool, cursor=None)
        self.assertIn("chunk-a", "".join(part["text"] for part in chunk["segments"]))
        first.call("entry/output/unsubscribe", session=session, entry=tool)
        resumed = first.call("entry/output/subscribe", session=session, entry=tool, cursor=chunk["next"])
        self.assertEqual(resumed["segments"], [])
        self.assertEqual([r["method"] for r in fixture.native.requests if r.get("method") in ("turn/start", "turn/steer")], ["turn/start"])
        interrupt = first.operation(3)
        first.call("session/interrupt", session=session, operation=interrupt, turn=turn)
        answer = first.operation(4)
        race = second.operation(2)
        winner = first.send("session/approve", session=session, operation=answer, approval=card["id"], answer={"kind": "decision", "decision": "once"})
        loser = second.send("session/approve", session=session, operation=race, approval=card["id"], answer={"kind": "decision", "decision": "once"})
        first.response(winner)
        second.response(loser)
        states = sorted([first.outcome(answer)["state"]["kind"], second.outcome(race)["state"]["kind"]])
        self.assertEqual(states, ["accepted", "rejected"])
        self.assertEqual(first.outcome(held)["state"]["kind"], "accepted")
        self.assertEqual(second.outcome(queued)["state"]["kind"], "accepted")
        update = first.message(lambda m: m.get("method") == "entry/output" and any("chunk-b" in p["text"] for p in m["params"]["chunk"]["segments"]))
        self.assertEqual(update["params"]["chunk"]["from"], chunk["next"])
        second.call("sessions/watch")
        first.call("sessions/watch")
        first_events = [m["params"] for m in first.messages if m.get("method") == "session/event" and int(m["params"]["seq"]) > int(a["seq"])]
        second_events = [m["params"] for m in second.messages if m.get("method") == "session/event" and int(m["params"]["seq"]) > int(a["seq"])]
        self.assertEqual(first_events, second_events)
        self.assertFalse(any(m.get("method") == "entry/output" for m in second.messages))
        self.assertFalse(any(m.get("method") == "operation/updated" and m["params"]["operation"]["client"] != first.hello["client"] for m in first.messages))
        self.assertEqual(fixture.native.answers, 1)
        self.assertEqual(fixture.native.interrupts, 1)

    def test_grok_create_retry_survives_disconnect_and_restores_lazily(self):
        fixture = Fixture("grok-create")
        self.addCleanup(fixture.close)
        first = Client(fixture, "first")
        self.addCleanup(first.close)
        operation = first.operation(1)
        params = dict(operation=operation, backend="grok", cwd=str(fixture.root), name="Phone spike")
        created = first.call("session/create", **params)
        session = created["session"]
        self.assertEqual(first.outcome(operation)["state"]["kind"], "accepted")
        self.assertEqual(len(fixture.native_requests("session/new")), 1)
        second = Client(fixture, "second")
        self.addCleanup(second.close)
        snapshot = second.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertEqual(snapshot["info"]["availability"], "ready")
        self.assertEqual(snapshot["info"]["name"], "Phone spike")
        first.close()
        resumed = Client(fixture, "resumed", resume=first.hello)
        self.addCleanup(resumed.close)
        self.assertTrue(resumed.hello["resumed"])
        self.assertEqual(resumed.call("session/create", **params)["session"], session)
        self.assertEqual(len(fixture.native_requests("session/new")), 1)
        send = resumed.operation(2)
        resumed.call("session/send", session=session, operation=send, text="hello phone", policy="steer")
        self.assertEqual(resumed.outcome(send)["state"]["kind"], "accepted")
        self.assertEqual(len(fixture.native_requests("session/prompt")), 1)
        fixture.stop()
        fixture.start()
        restarted = Client(fixture, "restarted", resume=resumed.hello)
        self.addCleanup(restarted.close)
        self.assertFalse(restarted.hello["resumed"])
        self.assertNotEqual(restarted.hello["instance"], resumed.hello["instance"])
        saved = restarted.call("sessions/watch")["sessions"]
        self.assertEqual([(row["id"], row["availability"], row["ralph"]) for row in saved], [(session, "dormant", False)])
        self.assertEqual(fixture.native_requests("session/load"), [])
        other = Client(fixture, "second-restorer")
        self.addCleanup(other.close)
        restoring = restarted.send("session/subscribe", session=session, since=snapshot["seq"])
        also_restoring = other.send("session/subscribe", session=session, since=None)
        restored = restarted.response(restoring)["result"]
        other.response(also_restoring)
        self.assertEqual(restored["kind"], "snapshot")
        self.assertEqual(restored["snapshot"]["info"]["id"], session)
        # A subscribe may return explicit restoring facts; ordered shared resets adopt history.
        support.wait_for(lambda: fixture.native_requests("session/load"), "one lazy native restore")
        self.assertEqual(len(fixture.native_requests("session/load")), 1)
        (fixture.root / "release-history").write_text("go")
        support.wait_for(lambda: next((row for row in restarted.call("sessions/watch")["sessions"] if row["id"] == session and row["availability"] == "ready"), None), "native history adopted")
        ready = restarted.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertEqual(ready["info"]["availability"], "ready")
        history = next(entry for entry in ready["entries"] if entry.get("source"))
        source = restarted.call("entry/source", session=session, entry=history["id"], cursor=None, limit=65536)
        self.assertIn("History complete", source["text"])
        print("Headless example:", {"hello": first.hello["client"], "session/create": params, "session/subscribe": session})

    def test_claude_startup_approval_is_answerable_before_create_finishes(self):
        fixture = Fixture("claude-startup", mode="claude-startup-question")
        self.addCleanup(fixture.close)
        first = Client(fixture, "creator")
        second = Client(fixture, "startup-observer")
        self.addCleanup(first.close)
        self.addCleanup(second.close)
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="claude", cwd=str(fixture.root), name="Startup alias")["session"]
        snapshot = second.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertEqual(snapshot["info"]["availability"], "starting")
        if snapshot["approvals"]:
            card = snapshot["approvals"][0]
        else:
            card = second.message(lambda m: m.get("method") == "session/event" and m["params"]["event"]["kind"] == "approval_requested")["params"]["event"]["approval"]
        answer = second.operation(1)
        second.call("session/approve", session=session, approval=card["id"], operation=answer, answer={"kind": "decision", "decision": "once"})
        self.assertEqual(second.outcome(answer)["state"]["kind"], "accepted")
        self.assertEqual(first.outcome(create)["state"]["kind"], "accepted")
        ready = first.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertEqual(ready["info"]["availability"], "ready")
        self.assertEqual(ready["info"]["name"], "Startup alias")
        send = first.operation(2)
        first.call("session/send", session=session, text="hello Claude", operation=send)
        self.assertEqual(first.outcome(send)["state"]["kind"], "accepted")

    def test_claude_native_start_failure_finishes_create_without_claiming_ready(self):
        fixture = Fixture("claude-failed", mode="claude-startup-closed")
        self.addCleanup(fixture.close)
        first = Client(fixture, "creator")
        self.addCleanup(first.close)
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="claude", cwd=str(fixture.root), name=None)["session"]
        self.assertEqual(first.outcome(create)["state"]["kind"], "uncertain")
        state = next(row for row in first.call("sessions/watch")["sessions"] if row["id"] == session)
        self.assertIn(state["availability"], ("closed", "unavailable"))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=support.BINARY)
    parser.add_argument("--artifacts", type=Path, default=support.ARTIFACTS)
    arguments, remaining = parser.parse_known_args()
    support.BINARY = arguments.binary.resolve()
    support.ARTIFACTS = arguments.artifacts.resolve()
    unittest.main(argv=[__file__, *remaining])
