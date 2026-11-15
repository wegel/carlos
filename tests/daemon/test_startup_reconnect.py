#!/usr/bin/env python3
"""Reconnect the current closed Codex owner explicitly without replaying prompts or native opens."""

import argparse
from pathlib import Path
import unittest

import support
from support import Client, Fixture, wait_for


class StartupReconnectTests(unittest.TestCase):
    def fixture(self, name):
        fixture = Fixture("startup-reconnect-" + name, codex=True)
        self.addCleanup(fixture.close)
        fixture.native.saved_rows = []
        first, other = Client(fixture, "first"), Client(fixture, "other")
        self.addCleanup(first.close)
        self.addCleanup(other.close)
        operation = first.operation(1)
        session = first.call("session/create", operation=operation, backend="codex", cwd=str(fixture.root), name=None)["session"]
        self.assertEqual(first.outcome(operation)["state"]["kind"], "accepted")
        self.availability(first, session, "ready")
        other.call("session/subscribe", session=session, since=None)
        return fixture, first, other, session

    def availability(self, client, session, expected):
        return wait_for(lambda: (page if (page := client.call("session/subscribe", session=session, since=None)["snapshot"])["info"]["availability"] == expected else None), expected + " native availability")

    def test_two_clients_share_one_explicit_native_reconnect_and_never_resend_unconfirmed_input(self):
        fixture, first, other, session = self.fixture("shared")
        prompt = first.operation(2)
        first.call("session/send", session=session, operation=prompt, text="held")
        wait_for(lambda: any(request.get("method") == "turn/start" for request in fixture.native.requests), "original prompt write")
        fixture.native.disconnect()
        self.availability(first, session, "closed")
        self.assertEqual(first.outcome(prompt)["state"]["kind"], "uncertain")
        fixture.native.saved_hold_resume = True
        reconnect = first.operation(3)
        first.call("session/reconnect", session=session, operation=reconnect)
        self.assertEqual(first.outcome(reconnect)["state"]["kind"], "accepted")
        wait_for(lambda: (fixture.root / "saved-resume-held").exists(), "rebuild holds exact native resume")
        self.availability(first, session, "restoring")
        self.availability(other, session, "restoring")
        duplicate = other.operation(1)
        other.call("session/reconnect", session=session, operation=duplicate)
        self.assertEqual(other.outcome(duplicate)["state"]["kind"], "rejected")
        self.assertEqual(fixture.native.resumed, 1)
        hello = first.hello
        first.close()
        resumed = Client(fixture, "resumed", resume=hello)
        self.addCleanup(resumed.close)
        self.assertEqual(resumed.call("session/reconnect", session=session, operation=reconnect)["state"]["kind"], "accepted")
        self.assertEqual(fixture.native.resumed, 1)
        fixture.native.release_saved("resume")
        self.availability(resumed, session, "ready")
        self.availability(other, session, "ready")
        self.assertEqual(len([request for request in fixture.native.requests if request.get("method") == "turn/start"]), 1)
        following = resumed.operation(4)
        resumed.call("session/send", session=session, operation=following, text="after native recovery")
        self.assertEqual(resumed.outcome(following)["state"]["kind"], "accepted")
        turns = [request for request in fixture.native.requests if request.get("method") == "turn/start"]
        self.assertEqual([request["params"]["input"][0]["text"] for request in turns], ["held", "after native recovery"])
        self.assertEqual(len(fixture.saved()), 1)
        self.assertEqual(fixture.native.started, 1)

    def test_failed_rebuild_stays_closed_and_only_a_new_explicit_operation_can_retry(self):
        fixture, first, other, session = self.fixture("failure")
        fixture.native.disconnect()
        self.availability(first, session, "closed")
        fixture.native.saved_fail_resume = True
        operation = first.operation(2)
        first.call("session/reconnect", session=session, operation=operation)
        self.assertEqual(first.outcome(operation)["state"]["kind"], "accepted", "the rebuild started, not completed")
        self.availability(first, session, "closed")
        self.assertEqual(fixture.native.resumed, 1)
        first.call("session/reconnect", session=session, operation=operation)
        other.call("session/subscribe", session=session, since=None)
        self.assertEqual(fixture.native.resumed, 1)
        fixture.native.saved_fail_resume = False
        retry = first.operation(3)
        first.call("session/reconnect", session=session, operation=retry)
        self.assertEqual(first.outcome(retry)["state"]["kind"], "accepted")
        self.availability(first, session, "ready")
        self.assertEqual(fixture.native.resumed, 2)

    def test_ready_owner_rejects_reconnect_without_any_native_resume(self):
        fixture, first, _, session = self.fixture("ready")
        operation = first.operation(2)
        first.call("session/reconnect", session=session, operation=operation)
        self.assertEqual(first.outcome(operation)["state"]["kind"], "rejected")
        self.assertEqual(fixture.native.resumed, 0)

    def test_claude_owner_rejects_codex_reconnect_without_replacing_its_helper(self):
        fixture = Fixture("startup-reconnect-unsupported", mode="modern")
        self.addCleanup(fixture.close)
        client = Client(fixture, "caller")
        self.addCleanup(client.close)
        create = client.operation(1)
        session = client.call("session/create", operation=create, backend="claude", cwd=str(fixture.root), name=None)["session"]
        self.assertEqual(client.outcome(create)["state"]["kind"], "accepted")
        self.availability(client, session, "ready")
        launched = fixture.pids()
        self.assertEqual(len([child for child in launched if child["backend"] == "claude"]), 1)
        operation = client.operation(2)
        client.call("session/reconnect", session=session, operation=operation)
        self.assertEqual(client.outcome(operation)["state"]["kind"], "rejected")
        self.availability(client, session, "ready")
        self.assertEqual(fixture.pids(), launched)



if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path, required=True)
    args, tests = parser.parse_known_args()
    support.BINARY = args.binary.resolve()
    support.ARTIFACTS = args.artifacts.resolve()
    unittest.main(argv=[__file__, *tests])
