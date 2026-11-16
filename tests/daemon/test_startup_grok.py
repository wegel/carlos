#!/usr/bin/env python3
"""Preserve ACP continue ordering, explicit-ID bypass and the old local picker fallback."""

import argparse
import json
from pathlib import Path
import unittest

import support
from support import Client, Fixture, wait_for


class StartupGrokTests(unittest.TestCase):
    def test_grok_preserves_full_native_replay_before_ready_then_pages_every_stored_row(self):
        fixture, client = self._grok_saved_fixture("complete-replay")
        (fixture.root / "grok-saved.json").write_text(json.dumps({"replay_turns":180, "hold_after":100}))
        operation = client.operation(1)
        session = client.call("session/open", operation=operation, backend="grok", cwd=str(fixture.root),
            target={"kind":"resume", "address":"complete-native-replay"})["session"]
        wait_for(lambda: (fixture.root / "replay-held").exists(), "native replay paused before completion")
        partial = client.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertIn(partial["info"]["availability"], ("starting", "restoring"))
        self.assertEqual(fixture.saved(), [])
        (fixture.root / "release-replay").touch()
        self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
        page = client.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertEqual(page["info"]["availability"], "ready")
        latest = {row["text"]:row["id"] for row in page["entries"]}
        rows = page["entries"]
        while page["has_older"]:
            page = client.call("session/history", session=session, before=rows[0]["id"], limit=100)
            self.assertTrue(page["entries"])
            rows = page["entries"] + rows
        self.assertEqual([row["text"] for row in rows], [f"Grok {kind} {index} 🌍" for index in range(180) for kind in ("user","answer")])
        self.assertEqual(len({row["id"] for row in rows}),360)
        self.assertTrue(all(latest.get(row["text"],row["id"]) == row["id"] for row in rows))
        self.assertEqual(len(fixture.native_requests("session/load")),1)
        self.assertEqual(fixture.native_requests("session/prompt"), [])
        self.assertFalse(any("cursor" in request.get("params",{}) for request in fixture.native_requests("session/load")))

    def _grok_saved_fixture(self, name, rows=None, error=False):
        fixture = Fixture("startup-grok-" + name, mode="grok-saved-normal")
        self.addCleanup(fixture.close)
        (fixture.root / "grok-saved.json").write_text(json.dumps({"rows":rows or [], "error":error}))
        client = Client(fixture, "launcher")
        self.addCleanup(client.close)
        return fixture, client

    def _grok_saved_local(self, fixture):
        path = Path(fixture.env["GROK_HOME"]) / "sessions/group/local-native/summary.json"
        path.parent.mkdir(parents=True)
        path.write_text(json.dumps({"info":{"id":"local-native", "cwd":str(fixture.root)},
            "session_summary":"Local saved title", "updated_at":"2026-11-10T00:00:00.000Z"}))

    def test_continue_preserves_native_last_row_on_updated_time_ties(self):
        rows = [{"sessionId":name, "title":name, "updatedAt":"2026-11-10T00:00:00Z"}
                for name in ("a-native-first", "z-native-last")]
        fixture, client = self._grok_saved_fixture("tie", rows)
        page = client.call("saved/open", backend="grok", cwd=str(fixture.root), purpose="continue")
        self.assertEqual(page["rows"][0]["name"], "z-native-last")
        operation = client.operation(1)
        client.call("session/open", operation=operation, backend="grok", cwd=str(fixture.root),
                    target={"kind":"choice", "scope":page["scope"], "choice":page["rows"][0]["choice"]})
        self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
        self.assertEqual([request["params"]["sessionId"] for request in fixture.native_requests("session/load")], ["z-native-last"])
        self.assertFalse(fixture.native_requests("session/new"))

    def test_empty_continue_does_not_use_local_history_and_can_explicitly_create_new(self):
        fixture, client = self._grok_saved_fixture("empty")
        self._grok_saved_local(fixture)
        page = client.call("saved/open", backend="grok", cwd=str(fixture.root), purpose="continue")
        self.assertEqual(page["rows"], [])
        self.assertFalse(fixture.native_requests("session/new"))
        operation = client.operation(1)
        client.call("session/create", operation=operation, backend="grok", cwd=str(fixture.root), name=None)
        self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
        self.assertEqual(len(fixture.native_requests("session/new")), 1)
        self.assertFalse(fixture.native_requests("session/load"))

    def test_continue_reports_native_listing_failure_without_fallback_or_new_session(self):
        fixture, client = self._grok_saved_fixture("error", error=True)
        self._grok_saved_local(fixture)
        response = client.response(client.send("saved/open", backend="grok", cwd=str(fixture.root), purpose="continue"))
        self.assertIn("catalogue unavailable", response["error"]["message"])
        self.assertEqual(client.call("sessions/watch")["sessions"], [])
        self.assertFalse(fixture.native_requests("session/new"))
        self.assertFalse(fixture.native_requests("session/load"))

    def test_picker_uses_local_fallback_after_native_empty_or_error(self):
        for error in (False, True):
            with self.subTest(native_error=error):
                fixture, client = self._grok_saved_fixture("fallback-" + str(error), error=error)
                self._grok_saved_local(fixture)
                page = client.call("saved/open", backend="grok", cwd=str(fixture.root), purpose="picker")
                self.assertEqual([row["name"] for row in page["rows"]], ["Local saved title"])
                self.assertNotIn("local-native", json.dumps(page))
                operation = client.operation(1)
                client.call("session/open", operation=operation, backend="grok", cwd=str(fixture.root),
                    target={"kind":"choice", "scope":page["scope"], "choice":page["rows"][0]["choice"]})
                self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
                self.assertEqual([request["params"]["sessionId"] for request in fixture.native_requests("session/load")], ["local-native"])

    def test_explicit_native_resume_bypasses_even_a_broken_catalogue(self):
        fixture, client = self._grok_saved_fixture("explicit", error=True)
        operation = client.operation(1)
        client.call("session/open", operation=operation, backend="grok", cwd=str(fixture.root),
                    target={"kind":"resume", "address":"explicit-native"})
        self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
        self.assertFalse(fixture.native_requests("session/list"))
        self.assertEqual([request["params"]["sessionId"] for request in fixture.native_requests("session/load")], ["explicit-native"])


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path, required=True)
    args, tests = parser.parse_known_args()
    support.BINARY = args.binary.resolve()
    support.ARTIFACTS = args.artifacts.resolve()
    unittest.main(argv=[__file__, *tests])
