#!/usr/bin/env python3
"""Prove private saved choices, exact import dedupe and no resend against the actual daemon."""

import argparse
import json
from pathlib import Path
import unittest

import support
from support import Client, Fixture, wait_for


class StartupOpenTests(unittest.TestCase):
    def _open_fixture(self, name, count=80):
        fixture = Fixture("startup-open-" + name, codex=True)
        self.addCleanup(fixture.close)
        fixture.native.saved_rows = [{"id":f"PRIVATE_NATIVE_ID_{index:04d}", "name":f"Saved {index}",
            "preview":f"Preview {index} é🦊", "cwd":str(fixture.root), "createdAt":index % 7,
            "updatedAt":index // 2, "futureNativeField":"PRIVATE_NATIVE_METADATA"} for index in range(count)]
        return fixture

    def _open_client(self, fixture, name, resume=None):
        client = Client(fixture, name, resume=resume)
        self.addCleanup(client.close)
        return client

    def _open_page(self, fixture, client):
        return client.call("saved/open", backend="codex", cwd=str(fixture.root), purpose="picker")

    def _open_params(self, fixture, client, target, counter=1, cwd=None):
        return dict(operation=client.operation(counter), backend="codex", cwd=cwd or str(fixture.root), target=target)

    def test_private_pages_bound_metadata_and_preserve_global_order_and_jump_keys(self):
        fixture = self._open_fixture("pages")
        first, second = self._open_client(fixture, "picker-a"), self._open_client(fixture, "picker-b")
        ordered = sorted(fixture.native.saved_rows, key=lambda row: (-row["updatedAt"], -row["createdAt"], row["id"]))
        newest = self._open_page(fixture, first)
        self.assertEqual([row["name"] for row in newest["rows"]], [row["name"] for row in ordered[:32]])
        self.assertEqual(len(newest["rows"]), 32)
        self.assertFalse(newest["has_newer"])
        self.assertTrue(newest["has_older"])
        self.assertNotIn("PRIVATE_NATIVE", json.dumps(newest))
        other = self._open_page(fixture, second)
        self.assertNotEqual(newest["scope"], other["scope"])
        foreign = self._open_params(fixture, second, {"kind":"choice", "scope":newest["scope"], "choice":newest["rows"][0]["choice"]})
        self.assertIn("expired", second.response(second.send("session/open", **foreign))["error"]["message"])
        older = first.call("saved/page", scope=newest["scope"], direction="older")
        self.assertEqual([row["name"] for row in older["rows"]], [row["name"] for row in ordered[32:64]])
        back = first.call("saved/page", scope=newest["scope"], direction="newer")
        self.assertEqual([row["name"] for row in back["rows"]], [row["name"] for row in ordered[:32]])
        self.assertNotEqual(newest["rows"][0]["choice"], back["rows"][0]["choice"])
        stale = self._open_params(fixture, first, {"kind":"choice", "scope":newest["scope"], "choice":newest["rows"][0]["choice"]})
        self.assertIn("expired", first.response(first.send("session/open", **stale))["error"]["message"])
        oldest = first.call("saved/page", scope=newest["scope"], direction="oldest")
        self.assertEqual([row["name"] for row in oldest["rows"]], [row["name"] for row in ordered[-32:]])
        self.assertTrue(oldest["has_newer"])
        self.assertFalse(oldest["has_older"])
        jumped = first.call("saved/page", scope=newest["scope"], direction="newest")
        self.assertEqual([row["name"] for row in jumped["rows"]], [row["name"] for row in ordered[:32]])
        self.assertEqual(first.call("sessions/watch")["sessions"], [])
        self.assertEqual(fixture.saved(), [], "discovery never imports the catalogue")
        self.assertEqual(fixture.native.resumed, 0)
        self.assertEqual(fixture.native.started, 0)

    def test_pending_choice_and_explicit_import_share_one_native_open_and_recover_without_resend(self):
        fixture = self._open_fixture("dedupe")
        first, second = self._open_client(fixture, "first"), self._open_client(fixture, "second")
        page = self._open_page(fixture, first)
        selected = page["rows"][0]
        native = next(row["id"] for row in fixture.native.saved_rows if row["name"] == selected["name"])
        fixture.native.saved_hold_resume = True
        params = self._open_params(fixture, first, {"kind":"choice", "scope":page["scope"], "choice":selected["choice"]})
        opened = first.call("session/open", **params)
        self.assertEqual(opened["state"]["kind"], "pending")
        wait_for(lambda: (fixture.root / "saved-resume-held").exists(), "original native resume held")
        alias = fixture.root / "cwd-alias"
        alias.symlink_to(fixture.root, target_is_directory=True)
        self.addCleanup(lambda: alias.unlink(missing_ok=True))
        other = self._open_params(fixture, second, {"kind":"resume", "address":native}, cwd=str(alias))
        same = second.call("session/open", **other)
        self.assertEqual(same["session"], opened["session"])
        different_cwd = fixture.root / "other-project"
        different_cwd.mkdir()
        cross_cwd = self._open_params(fixture, second, {"kind":"resume", "address":native},
                                      counter=2, cwd=str(different_cwd))
        self.assertEqual(second.call("session/open", **cross_cwd)["session"], opened["session"],
                         "one configured Codex endpoint owns each native ID once across client cwds")
        invalid = self._open_params(fixture, second, {"kind":"resume", "address":native},
                                   counter=3, cwd=str(fixture.root / "missing-project"))
        self.assertIn("cwd", second.response(second.send("session/open", **invalid))["error"]["message"])
        self.assertEqual(fixture.native.resumed, 1)
        hello = first.hello
        first.close()
        resumed = self._open_client(fixture, "resumed", resume=hello)
        self.assertTrue(resumed.hello["resumed"])
        self.assertEqual(resumed.call("session/open", **params)["session"], opened["session"], "retry precedes expired picker lookup")
        self.assertEqual(fixture.native.resumed, 1)
        fixture.native.release_saved("resume")
        self.assertEqual(resumed.outcome(params["operation"])["state"]["kind"], "accepted")
        self.assertEqual(second.outcome(other["operation"])["state"]["kind"], "accepted")
        self.assertEqual(second.outcome(cross_cwd["operation"])["state"]["kind"], "accepted")
        snapshot = second.call("session/subscribe", session=opened["session"], since=None)["snapshot"]
        self.assertEqual(snapshot["info"]["cwd"], str(fixture.root), "the first owner's cwd remains authoritative")
        self.assertEqual(len(fixture.saved()), 1)
        self.assertEqual(fixture.saved()[0][0], opened["session"])
        self.assertEqual(fixture.saved()[0][2], native)
        self.assertFalse(any(message.get("method") == "operation/updated" and
            message["params"]["operation"]["client"] != second.hello["client"] for message in second.messages))
        self.assertEqual([request["params"]["threadId"] for request in fixture.native.requests
                          if request.get("method") == "thread/resume"], [native])

    def test_metadata_previews_are_bounded_but_open_keeps_the_complete_native_name(self):
        fixture = self._open_fixture("metadata", count=1)
        original = "Titre🦊" * 200
        fixture.native.saved_rows[0].update(name=original, preview="é🦊" * 10000)
        first = self._open_client(fixture, "first")
        page = self._open_page(fixture, first)
        row = page["rows"][0]
        self.assertLessEqual(len(row["name"].encode()), 512)
        self.assertLessEqual(len(row["preview"].encode()), 4096)
        self.assertTrue(original.startswith(row["name"]))
        self.assertNotIn("�", row["preview"])
        params = self._open_params(fixture, first, {"kind":"choice", "scope":page["scope"], "choice":row["choice"]})
        opened = first.call("session/open", **params)
        self.assertEqual(first.outcome(params["operation"])["state"]["kind"], "accepted")
        info = first.call("session/subscribe", session=opened["session"], since=None)["snapshot"]["info"]
        self.assertEqual(info["name"], original)
        self.assertEqual(fixture.saved()[0][1], original)

    def test_known_logical_address_wins_and_ignores_first_launch_choices(self):
        fixture = self._open_fixture("known", count=0)
        first = self._open_client(fixture, "first")
        native = "e" * 32
        params = self._open_params(fixture, first, {"kind":"resume", "address":native})
        opened = first.call("session/open", **params)
        self.assertEqual(first.outcome(params["operation"])["state"]["kind"], "accepted")
        self.assertNotEqual(opened["session"], native, "a native hex address is not guessed to be logical")
        self.assertFalse(any(request.get("method") == "thread/list" and "cwd" in request.get("params", {}) for request in fixture.native.requests), "explicit IDs bypass saved-session listing")
        other = self._open_client(fixture, "other")
        attached = other.call("session/open", operation=other.operation(1), backend="grok", cwd="/missing-client-cwd",
            target={"kind":"resume", "address":opened["session"]}, tool_labels=True,
            launch={"claude_context_window":"ignored-on-attach", "reasoning_summary":"detailed"})
        self.assertEqual(attached["session"], opened["session"])
        self.assertEqual(attached["state"]["kind"], "accepted")
        self.assertEqual(fixture.native.resumed, 1)
        snapshot = other.call("session/subscribe", session=opened["session"], since=None)["snapshot"]
        self.assertEqual(snapshot["info"]["backend"], "codex")
        self.assertFalse(other.call("settings/read", session=opened["session"])["settings"]["summaries_enabled"])

    def test_failed_import_never_resends_on_subscribe_or_reconnect_but_explicit_new_open_can_retry(self):
        fixture = self._open_fixture("failed", count=0)
        fixture.native.saved_fail_resume = True
        first = self._open_client(fixture, "first")
        params = self._open_params(fixture, first, {"kind":"resume", "address":"missing-native"})
        opened = first.call("session/open", **params)
        self.assertEqual(first.outcome(params["operation"])["state"]["kind"], "uncertain")
        self.assertEqual(fixture.native.resumed, 1)
        failed = first.response(first.send("session/subscribe", session=opened["session"], since=None))
        self.assertIn("unavailable", failed["error"]["message"])
        hello = first.hello
        first.close()
        resumed = self._open_client(fixture, "resumed", resume=hello)
        self.assertEqual(resumed.call("operation/read", operation=params["operation"])["state"]["kind"], "uncertain")
        self.assertIn("error", resumed.response(resumed.send("session/subscribe", session=opened["session"], since=None)))
        self.assertEqual(fixture.native.resumed, 1)
        fixture.native.saved_fail_resume = False
        retry = self._open_params(fixture, resumed, {"kind":"resume", "address":opened["session"]}, counter=2)
        self.assertEqual(resumed.call("session/open", **retry)["session"], opened["session"])
        self.assertEqual(resumed.outcome(retry["operation"])["state"]["kind"], "accepted")
        self.assertEqual(fixture.native.resumed, 2)

    def test_unconfirmed_import_is_not_saved_or_reopened_by_a_restart_subscription(self):
        fixture = self._open_fixture("failed-restart", count=0)
        fixture.native.saved_fail_resume = True
        first = self._open_client(fixture, "first")
        params = self._open_params(fixture, first, {"kind":"resume", "address":"unconfirmed-native"})
        failed = first.call("session/open", **params)["session"]
        self.assertEqual(first.outcome(params["operation"])["state"]["kind"], "uncertain")
        barrier = first.operation(2)
        confirmed = first.call("session/create", operation=barrier, backend="codex", cwd=str(fixture.root), name=None)["session"]
        self.assertEqual(first.outcome(barrier)["state"]["kind"], "accepted")
        self.assertEqual([row[0] for row in fixture.saved()], [confirmed],
                         "a confirmed create fences earlier disk jobs; failed native imports are not durable")
        hello = first.hello
        first.close()
        fixture.stop()
        fixture.start()
        resumed = self._open_client(fixture, "replacement", resume=hello)
        self.assertFalse(resumed.hello["resumed"])
        self.assertEqual([row["id"] for row in resumed.call("sessions/watch")["sessions"]], [confirmed])
        response = resumed.response(resumed.send("session/subscribe", session=failed, since=None))
        self.assertIn("error", response)
        self.assertEqual(fixture.native.resumed, 1, "restart subscribe never retries the unconfirmed native open")
        self.assertEqual(fixture.native.started, 1)

    def test_held_discovery_keeps_live_controls_responsive_and_close_cancels_its_original_reader(self):
        fixture = self._open_fixture("held")
        control = self._open_client(fixture, "control")
        create = control.operation(1)
        session = control.call("session/create", operation=create, backend="codex", cwd=str(fixture.root), name=None)["session"]
        self.assertEqual(control.outcome(create)["state"]["kind"], "accepted")
        control.call("session/subscribe", session=session, since=None)
        prompt = control.operation(2)
        control.call("session/send", session=session, operation=prompt, text="held")
        card = control.message(lambda message: message.get("params", {}).get("event", {}).get("kind") == "approval_requested")["params"]["event"]["approval"]
        turn = wait_for(lambda: control.call("session/subscribe", session=session, since=None)["snapshot"]["info"]["current_turn"], "live turn")
        fixture.native.saved_hold_list = True
        picker = self._open_client(fixture, "picker")
        pending = picker.send("saved/open", backend="codex", cwd=str(fixture.root), purpose="picker")
        wait_for(lambda: (fixture.root / "saved-list-held").exists(), "native catalogue read held")
        self.assertTrue(control.call("sessions/watch")["sessions"])
        approval, interrupt = control.operation(3), control.operation(4)
        control.call("session/approve", session=session, operation=approval, approval=card["id"], answer={"kind":"decision", "decision":"once"})
        control.call("session/interrupt", session=session, operation=interrupt, turn=turn)
        self.assertEqual(control.outcome(approval)["state"]["kind"], "accepted")
        self.assertEqual(control.outcome(interrupt)["state"]["kind"], "accepted")
        self.assertEqual(control.outcome(prompt)["state"]["kind"], "accepted")
        self.assertFalse(any(message.get("id") == pending for message in picker.messages))
        picker.call("saved/close", scope=None)
        self.assertIn("closed", picker.response(pending)["error"]["message"])
        wait_for(lambda: len(fixture.native.connections) == 1, "cancelled native reader closed")
        fixture.native.release_saved("list")
        self.assertEqual(fixture.native.started, 1)
        self.assertEqual(fixture.native.resumed, 0)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path, required=True)
    args, tests = parser.parse_known_args()
    support.BINARY = args.binary.resolve()
    support.ARTIFACTS = args.artifacts.resolve()
    unittest.main(argv=[__file__, *tests])
