#!/usr/bin/env python3
"""Recover a closed Claude helper only through explicit open, without replacing its owner."""

import argparse
import json
import os
from pathlib import Path
import signal
import unittest

import support
from support import Client, Fixture, wait_for


class ClaudeReopenTests(unittest.TestCase):
    def fixture(self, name, mode="modern", seed=True):
        fixture = Fixture("claude-reopen-" + name, mode=mode)
        self.addCleanup(fixture.close)
        first = self.client(fixture, "first")
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="claude", cwd=str(fixture.root),
                             name="Same logical owner", launch={"claude_context_window":" 47000 "})["session"]
        self.assertEqual(first.outcome(create)["state"]["kind"], "accepted")
        self.availability(first, session, "ready")
        if seed:
            prompt = first.operation(2)
            first.call("session/send", session=session, operation=prompt, text="seed before reopen")
            self.assertEqual(first.outcome(prompt)["state"]["kind"], "accepted")
            wait_for(lambda: any(row["text"] == "Answer: seed before reopen" for row in self.snapshot(first, session)["entries"]), "native seed answer")
            wait_for(lambda: self.snapshot(first, session)["info"]["phase"] == "idle", "native seed turn idle")
        return fixture, first, session

    def client(self, fixture, name, resume=None):
        client = Client(fixture, name, resume=resume)
        self.addCleanup(client.close)
        return client

    def snapshot(self, client, session):
        return client.call("session/subscribe", session=session, since=None)["snapshot"]

    def availability(self, client, session, expected):
        return wait_for(lambda: page if (page := self.snapshot(client, session))["info"]["availability"] == expected else None,
                        "Claude " + expected)

    def helpers(self, fixture):
        path = fixture.root / "claude-controller.jsonl"
        return [record["message"] for line in path.read_text().splitlines()
                if (record := json.loads(line))["direction"] == "client"] if path.exists() else []

    def native(self, fixture):
        path = fixture.root / "rpc.jsonl"
        return [record["message"] for line in path.read_text().splitlines()
                if (record := json.loads(line))["direction"] == "client"] if path.exists() else []

    def kill(self, fixture, client, session):
        child = [row for row in fixture.pids() if row["backend"] == "claude"][-1]
        os.kill(child["pid"], signal.SIGTERM)
        return self.availability(client, session, "closed")

    def open(self, fixture, client, address, counter):
        return dict(operation=client.operation(counter), backend="claude", cwd=str(fixture.root),
                    target={"kind":"resume", "address":address})

    def change_next_mode(self, fixture, mode):
        shim = fixture.root / "bin/claude"
        text = shim.read_text().replace("root = Path(os.environ", f"os.environ['CARLOS_TEST_SERVER'] = {mode!r}\nroot = Path(os.environ", 1)
        shim.write_text(text)

    def test_two_explicit_opens_share_replacement_and_startup_controls_without_resetting_sequence(self):
        fixture, first, session = self.fixture("shared")
        other = self.client(fixture, "other")
        native = fixture.saved()[0][2]
        settings = other.call("settings/read", session=session)["settings"]
        change = other.operation(1)
        other.call("settings/set", session=session, operation=change, expected=settings["revision"],
                   change={"model":{"kind":"value", "value":"sdk-opus"}, "effort":{"kind":"value", "value":"high"}}, save_default=False)
        self.assertEqual(other.outcome(change)["state"]["kind"], "accepted")
        before = self.kill(fixture, first, session)
        defaults = Path(fixture.env["XDG_CONFIG_HOME"]) / "carlos/runtime-defaults.json"
        defaults.parent.mkdir(parents=True, exist_ok=True)
        defaults.write_text(json.dumps({"model":"sdk-sonnet", "effort":"low", "summary":None}))
        original_defaults = defaults.read_bytes()
        self.change_next_mode(fixture, "claude-startup-question")
        original = self.open(fixture, first, session, 3)
        concurrent = self.open(fixture, other, native, 2)
        first_request = first.send("session/open", **original)
        other_request = other.send("session/open", **concurrent)
        self.assertEqual(first.response(first_request)["result"]["session"], session)
        self.assertEqual(other.response(other_request)["result"]["session"], session)
        self.assertEqual(first.outcome(original["operation"])["state"]["kind"], "accepted")
        starting = self.availability(first, session, "starting")
        card = wait_for(lambda: (page["approvals"][0] if (page := self.snapshot(other, session))["approvals"] else None), "replacement startup callback")
        self.assertGreater(int(starting["seq"]), int(before["seq"]))
        interrupt = other.operation(3)
        other.call("session/approve", session=session, operation=interrupt, approval=card["id"], answer={"kind":"interrupt_query"})
        self.assertEqual(other.outcome(interrupt)["state"]["kind"], "accepted")
        answer = other.operation(4)
        other.call("session/approve", session=session, operation=answer, approval=card["id"], answer={"kind":"decision", "decision":"once"})
        self.assertEqual(other.outcome(answer)["state"]["kind"], "accepted")
        ready = self.availability(first, session, "ready")
        self.assertGreater(int(ready["seq"]), int(starting["seq"]))
        self.assertEqual(other.outcome(concurrent["operation"])["state"]["kind"], "accepted")
        self.assertEqual(len(fixture.pids()), 2)
        self.assertEqual([row["context_window"] for row in fixture.pids()], [" 47000 "] * 2)
        self.assertEqual([request["params"].get("resume") for request in self.helpers(fixture) if request["method"] == "start"], [None, native])
        last_start = [request["params"] for request in self.helpers(fixture) if request["method"] == "start"][-1]
        self.assertEqual(last_start["model"], "sdk-opus")
        self.assertEqual(last_start["effort"], "high")
        self.assertEqual(defaults.read_bytes(), original_defaults, "reopen keeps owner choices without rereading or saving host defaults")
        events = [int(message["params"]["seq"]) for message in first.messages if message.get("method") == "session/event"]
        self.assertTrue(events)
        self.assertEqual(events, sorted(set(events)), "numbered facts never reset or repeat during same-owner reopen")
        hello = first.hello
        first.close()
        resumed = self.client(fixture, "resumed", hello)
        self.assertEqual(resumed.call("session/open", **original)["state"]["kind"], "accepted")
        self.availability(resumed, session, "ready")
        self.assertEqual(len(fixture.pids()), 2)
        self.assertEqual(len([request for request in self.native(fixture) if request.get("type") == "user"]), 1)
        self.assertFalse(any(message.get("method") == "operation/updated" and message["params"]["operation"]["client"] != other.hello["client"] for message in other.messages))
        self.assertEqual(len(fixture.saved()), 1)

    def test_failed_fresh_edit_reopens_exact_new_branch_without_resending_recovered_text(self):
        fixture, first, session = self.fixture("fresh-edit", "claude-fork-start-failure")
        original_native = fixture.saved()[0][2]
        snapshot = self.snapshot(first, session)
        entry = next(row["id"] for row in snapshot["entries"] if row.get("kind") == "user")
        edit = first.operation(3)
        first.call("session/rewind", session=session, operation=edit, entry=entry, text="edited draft kept 🦊")
        failed = first.outcome(edit)
        self.assertEqual(failed["state"]["kind"], "uncertain")
        self.assertEqual(failed["state"]["recovery"]["text"], "edited draft kept 🦊")
        self.availability(first, session, "closed")
        failed_start = [request for request in self.helpers(fixture) if request["method"] == "start"][-1]["params"]
        planned = failed_start["sessionId"]
        self.assertNotEqual(planned, original_native)
        self.assertNotIn("resume", failed_start)
        (fixture.root / "first-native-session").unlink()
        reopen = self.open(fixture, first, session, 4)
        first.call("session/open", **reopen)
        self.assertEqual(first.outcome(reopen["operation"])["state"]["kind"], "accepted")
        self.availability(first, session, "ready")
        starts = [request["params"] for request in self.helpers(fixture) if request["method"] == "start"]
        self.assertEqual(starts[-1]["sessionId"], planned)
        self.assertNotIn("resume", starts[-1])
        self.assertEqual(len([request for request in self.helpers(fixture) if request["method"] == "sessions.forkBefore"]), 1)
        self.assertEqual(len([request for request in self.native(fixture) if request.get("type") == "user"]), 1)
        self.assertEqual(first.call("operation/read", operation=edit), failed)
        send = first.operation(5)
        first.call("session/send", session=session, operation=send, text="edited draft kept 🦊")
        self.assertEqual(first.outcome(send)["state"]["kind"], "accepted")
        self.assertEqual(len([request for request in self.native(fixture) if request.get("type") == "user"]), 2)

    def test_repeated_close_before_first_prompt_stays_new_then_saved_history_resumes(self):
        fixture, first, session = self.fixture("blank", seed=False)
        native = fixture.saved()[0][2]
        for counter in (2, 3):
            self.kill(fixture, first, session)
            params = self.open(fixture, first, session, counter)
            first.call("session/open", **params)
            self.assertEqual(first.outcome(params["operation"])["state"]["kind"], "accepted")
            self.availability(first, session, "ready")
            start = [request["params"] for request in self.helpers(fixture) if request["method"] == "start"][-1]
            self.assertEqual(start["sessionId"], native)
            self.assertNotIn("resume", start)
            self.assertNotIn("model", start, "observed inherited model is never pinned")
        send = first.operation(4)
        first.call("session/send", session=session, operation=send, text="first durable prompt")
        self.assertEqual(first.outcome(send)["state"]["kind"], "accepted")
        wait_for(lambda: any(row["text"] == "Answer: first durable prompt" for row in self.snapshot(first, session)["entries"]), "native persisted first answer")
        self.kill(fixture, first, session)
        reopen = self.open(fixture, first, session, 5)
        first.call("session/open", **reopen)
        self.assertEqual(first.outcome(reopen["operation"])["state"]["kind"], "accepted")
        page = self.availability(first, session, "ready")
        self.assertTrue(any(row["text"] == "first durable prompt" for row in page["entries"]))
        start = [request["params"] for request in self.helpers(fixture) if request["method"] == "start"][-1]
        self.assertEqual(start["resume"], native)
        self.assertNotIn("sessionId", start)
        self.assertEqual(len([request for request in self.native(fixture) if request.get("type") == "user"]), 1)
        self.assertEqual(len(fixture.pids()), 4)

    def test_failed_reopen_needs_new_explicit_operation_and_never_retries_on_subscription(self):
        fixture, first, session = self.fixture("failure", "claude-fork-start-failure")
        self.kill(fixture, first, session)
        (fixture.root / "first-native-session").write_text("another-native-root")
        reopen = self.open(fixture, first, session, 3)
        first.call("session/open", **reopen)
        self.assertEqual(first.outcome(reopen["operation"])["state"]["kind"], "accepted")
        wait_for(lambda: len(fixture.pids()) == 2, "attempted replacement")
        self.availability(first, session, "closed")
        hello = first.hello
        first.close()
        resumed = self.client(fixture, "resumed", hello)
        resumed.call("session/open", **reopen)
        self.availability(resumed, session, "closed")
        self.assertEqual(len(fixture.pids()), 2)
        (fixture.root / "first-native-session").unlink()
        retry = self.open(fixture, resumed, session, 4)
        resumed.call("session/open", **retry)
        self.assertEqual(resumed.outcome(retry["operation"])["state"]["kind"], "accepted")
        self.availability(resumed, session, "ready")
        self.assertEqual(len(fixture.pids()), 3)
        self.assertEqual(len([request for request in self.native(fixture) if request.get("type") == "user"]), 1)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path, required=True)
    args, tests = parser.parse_known_args()
    support.BINARY = args.binary.resolve()
    support.ARTIFACTS = args.artifacts.resolve()
    unittest.main(argv=[__file__, *tests])
