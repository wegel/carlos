#!/usr/bin/env python3
"""Archive exact opaque choices without importing conversations or replaying native writes."""

import argparse
from pathlib import Path
import unittest

import support
from support import Client, Fixture, wait_for


class StartupArchiveTests(unittest.TestCase):
    def fixture(self, name, count=3):
        fixture = Fixture("startup-archive-" + name, codex=True)
        self.addCleanup(fixture.close)
        fixture.native.saved_rows = [{"id":f"PRIVATE_ARCHIVE_{index}", "name":f"Saved {index}",
            "preview":"Saved text", "cwd":str(fixture.root), "createdAt":index,
            "updatedAt":index} for index in range(count)]
        client = Client(fixture, "picker")
        self.addCleanup(client.close)
        page = client.call("saved/open", backend="codex", cwd=str(fixture.root), purpose="picker")
        return fixture, client, page

    def params(self, client, page, counter=1):
        return dict(operation=client.operation(counter), scope=page["scope"], choice=page["rows"][0]["choice"])

    def test_disconnected_archive_retains_private_unscoped_outcome_without_import_or_resend(self):
        fixture, first, page = self.fixture("disconnect")
        fixture.native.saved_hold_archive = True
        params = self.params(first, page)
        pending = first.call("saved/archive", **params)
        self.assertNotIn("session", pending)
        wait_for(lambda: len(fixture.native.saved_archives) == 1, "captured archive write")
        native = fixture.native.saved_archives[0]
        self.assertEqual(native, "PRIVATE_ARCHIVE_2")
        hello = first.hello
        first.close()
        other = Client(fixture, "other")
        self.addCleanup(other.close)
        self.assertEqual(other.call("sessions/watch")["sessions"], [])
        self.assertIn("another client", other.response(other.send("operation/read", operation=params["operation"]))["error"]["message"])
        resumed = Client(fixture, "resumed", resume=hello)
        self.addCleanup(resumed.close)
        self.assertEqual(resumed.call("saved/archive", **params)["state"]["kind"], "pending")
        fixture.native.release_saved("archive")
        result = resumed.outcome(params["operation"])
        self.assertEqual(result["state"]["kind"], "accepted")
        self.assertNotIn("session", result)
        self.assertEqual(resumed.call("saved/archive", **params), result)
        self.assertEqual(fixture.native.saved_archives, [native])
        self.assertEqual(fixture.saved(), [])
        self.assertEqual(fixture.native.resumed, 0)
        self.assertEqual(fixture.native.started, 0)

    def test_stale_and_foreign_choices_never_reach_native_archive(self):
        fixture, first, page = self.fixture("stale")
        other = Client(fixture, "other")
        self.addCleanup(other.close)
        params = self.params(other, page)
        self.assertIn("expired", other.response(other.send("saved/archive", **params))["error"]["message"])
        first.call("saved/page", scope=page["scope"], direction="newest")
        self.assertIn("expired", first.response(first.send("saved/archive", **self.params(first, page)))["error"]["message"])
        self.assertEqual(fixture.native.saved_archives, [])
        self.assertEqual(fixture.saved(), [])

    def test_rejection_and_lost_reply_do_not_authorize_a_repeat_archive(self):
        for uncertain in (False, True):
            with self.subTest(uncertain=uncertain):
                fixture, client, page = self.fixture("failure-" + str(uncertain))
                fixture.native.saved_reject_archive = not uncertain
                fixture.native.saved_disconnect_archive = uncertain
                params = self.params(client, page)
                client.call("saved/archive", **params)
                outcome = client.outcome(params["operation"])
                self.assertEqual(outcome["state"]["kind"], "uncertain" if uncertain else "rejected")
                self.assertNotIn("session", outcome)
                self.assertEqual(client.call("saved/archive", **params), outcome)
                self.assertEqual(len(fixture.native.saved_archives), 1)
                self.assertEqual(fixture.saved(), [])

    def test_archiving_a_whole_page_keeps_its_original_neighbor_boundary(self):
        fixture, client, page = self.fixture("boundary", count=33)
        for counter, row in enumerate(page["rows"], 1):
            operation = client.operation(counter)
            client.call("saved/archive", operation=operation, scope=page["scope"], choice=row["choice"])
            self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
        following = client.call("saved/page", scope=page["scope"], direction="older")
        self.assertEqual([row["name"] for row in following["rows"]], ["Saved 0"])
        self.assertFalse(following["has_older"])
        self.assertEqual(len(fixture.native.saved_archives), 32)
        self.assertEqual(fixture.saved(), [])


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path, required=True)
    args, tests = parser.parse_known_args()
    support.BINARY = args.binary.resolve()
    support.ARTIFACTS = args.artifacts.resolve()
    unittest.main(argv=[__file__, *tests])
