"""Check host defaults, shared label facts and the actual daemon-owned label worker."""

import asyncio
from contextlib import closing

import fcntl
import json
from pathlib import Path
import sqlite3
import unittest

import support
from native_tool_labels import configure, LABEL, OUTPUT
from support import Client, Fixture, alive, wait_for


class ToolLabelTests:
    def label_clients(self, name, override="on", saved=None, create_override=None, history=False):
        fixture = Fixture(name, codex=True, tool_summaries=override)
        self.addCleanup(fixture.close)
        configure(fixture)
        fixture.native.summary_history = history
        if saved is not None:
            path = self.label_preferences(fixture)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(saved)
        first, other = Client(fixture, "label-owner"), Client(fixture, "label-observer")
        self.addCleanup(first.close)
        self.addCleanup(other.close)
        create = first.operation(1)
        extra = {} if create_override is None else {"tool_labels":create_override}
        session = first.call("session/create", operation=create, backend="codex", cwd=str(fixture.root), name=None, **extra)["session"]
        self.assertEqual(first.outcome(create)["state"]["kind"], "accepted")
        first.call("session/subscribe", session=session, since=None)
        other.call("session/subscribe", session=session, since=None)
        return fixture, first, other, session

    @staticmethod
    def label_preferences(fixture):
        return Path(fixture.env["XDG_CONFIG_HOME"]) / "carlos/ui-settings.json"

    @staticmethod
    def label_params(client, session, enabled, counter=2, save=True):
        facts = client.call("settings/read", session=session)["settings"]
        return dict(session=session, operation=client.operation(counter), expected=facts["revision"],
                    enabled=enabled, save_default=save)

    @staticmethod
    def label_rows(client, session):
        snapshot = client.call("session/subscribe", session=session, since=None)["snapshot"]
        rows = []
        for row in snapshot["entries"]:
            if (row.get("tool") or {}).get("has_output"):
                source = client.call("entry/source", session=session, entry=row["id"], cursor=None, limit=65536)["text"]
                if "original output marker" in source:
                    rows.append(row)
        return rows

    def label_send(self, client, session, counter=2):
        operation = client.operation(counter)
        client.call("session/send", session=session, operation=operation, text="summarize command")
        self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
        return wait_for(lambda: self.label_rows(client, session), "original eligible tool row")[0]

    def label_controls(self, fixture, client, session):
        async def active_native():
            for link in list(fixture.native.connections):
                await fixture.native.held_facts(link)
        asyncio.run_coroutine_threadsafe(active_native(), fixture.native.loop).result(timeout=3)
        card = client.message(lambda message: message.get("params", {}).get("event", {}).get("kind") == "approval_requested")["params"]["event"]["approval"]
        turn = client.call("session/subscribe", session=session, since=None)["snapshot"]["info"]["current_turn"]
        approval, interrupt = client.operation(1), client.operation(2)
        client.call("session/approve", session=session, operation=approval, approval=card["id"], answer={"kind":"decision", "decision":"once"})
        client.call("session/interrupt", session=session, operation=interrupt, turn=turn)
        self.assertEqual(client.outcome(approval)["state"]["kind"], "accepted")
        self.assertEqual(client.outcome(interrupt)["state"]["kind"], "accepted")
        self.assertEqual((fixture.native.answers, fixture.native.interrupts), (1, 1))

    def test_tool_labels_host_flag_environment_and_first_launch_override_keep_legacy_rules(self):
        cases = [(None, None, None, True), (None, '{"ai_tool_labels":false}', None, False),
                 (None, 'malformed', None, True), ("on", '{"ai_tool_labels":false}', None, True),
                 ("off", '{"ai_tool_labels":true}', None, False), ("TRUE", None, None, False),
                 ("off", None, True, True), ("on", None, False, False)]
        for index, (override, saved, explicit, expected) in enumerate(cases):
            with self.subTest(override=override, saved=saved, explicit=explicit):
                fixture, first, _, session = self.label_clients(f"label-startup-{index}", override, saved, explicit)
                facts = first.call("settings/read", session=session)["settings"]
                self.assertEqual(facts["summaries_enabled"], expected)
                self.assertEqual(fixture.native.prompts, 0)
                self.assertFalse((fixture.root / "summary-requests.jsonl").exists())

    def test_tool_labels_change_shared_facts_private_results_and_only_the_saved_flag(self):
        saved = json.dumps({"ai_tool_labels":True, "tool_output":"compact", "themes":{"phone":"flow"}, "future":19})
        fixture, first, other, session = self.label_clients("label-preference", None, saved)
        params = self.label_params(first, session, False)
        first.call("settings/tool-labels/set", **params)
        result = first.outcome(params["operation"])
        self.assertEqual(result["state"]["kind"], "accepted")
        self.assertEqual(first.call("settings/tool-labels/set", **params), result)
        self.assertFalse(other.call("settings/read", session=session)["settings"]["summaries_enabled"])
        self.assertEqual(json.loads(self.label_preferences(fixture).read_text()),
                         {"ai_tool_labels":False, "tool_output":"compact", "themes":{"phone":"flow"}, "future":19})
        other.call("sessions/watch")
        self.assertFalse(any(message.get("method") == "operation/updated" for message in other.messages))
        stale = {**params, "operation":first.operation(3), "enabled":True}
        first.call("settings/tool-labels/set", **stale)
        self.assertEqual(first.outcome(stale["operation"])["state"]["kind"], "rejected")
        self.assertFalse(json.loads(self.label_preferences(fixture).read_text())["ai_tool_labels"])
        self.assertEqual(fixture.native.prompts, 0)

    def test_saved_default_keeps_other_live_sessions_and_explicit_create_choice_independent(self):
        fixture, first, other, session = self.label_clients("label-default-scope", None)
        create = other.operation(1)
        second = other.call("session/create", operation=create, backend="codex", cwd=str(fixture.root), name=None)["session"]
        self.assertEqual(other.outcome(create)["state"]["kind"], "accepted")
        other.call("session/subscribe", session=second, since=None)
        params = self.label_params(first, session, False)
        first.call("settings/tool-labels/set", **params)
        self.assertEqual(first.outcome(params["operation"])["state"]["kind"], "accepted")
        self.assertTrue(other.call("settings/read", session=second)["settings"]["summaries_enabled"])
        explicit = other.operation(2)
        third = other.call("session/create", operation=explicit, backend="codex", cwd=str(fixture.root), name=None, tool_labels=True)["session"]
        self.assertEqual(other.outcome(explicit)["state"]["kind"], "accepted")
        other.call("session/subscribe", session=third, since=None)
        self.assertTrue(other.call("settings/read", session=third)["settings"]["summaries_enabled"])
        self.assertFalse(json.loads(self.label_preferences(fixture).read_text())["ai_tool_labels"])
        fixture.stop()
        fixture.start()
        restored = Client(fixture, "scope-restored")
        self.addCleanup(restored.close)
        restored.call("session/subscribe", session=second, since=None)
        self.assertFalse(restored.call("settings/read", session=second)["settings"]["summaries_enabled"])
        self.assertEqual(fixture.native.prompts, 0)

    def test_tool_labels_held_default_write_keeps_reads_controls_and_resume_without_resend(self):
        fixture, first, other, session = self.label_clients("label-default-held")
        path = self.label_preferences(fixture)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text('{"tool_output":"compact"}')
        lock = path.with_name(".ui-settings.json.lock").open("a+")
        self.addCleanup(lock.close)
        fcntl.flock(lock, fcntl.LOCK_EX)
        params = self.label_params(first, session, False)
        first.call("settings/tool-labels/set", **params)
        wait_for(lambda: not other.call("settings/read", session=session)["settings"]["summaries_enabled"], "accepted in-memory flag")
        self.assertIn(first.call("operation/read", operation=params["operation"])["state"]["kind"], ("pending", "queued"))
        self.label_controls(fixture, other, session)
        # Simulate the device writer publishing its unrelated field while it owns the same lock.
        path.write_text('{"tool_output":"full","themes":{"phone":"flow"}}')
        hello = first.hello
        first.close()
        resumed = Client(fixture, "label-resumed", resume=hello)
        self.addCleanup(resumed.close)
        self.assertTrue(resumed.hello["resumed"])
        fcntl.flock(lock, fcntl.LOCK_UN)
        result = resumed.outcome(params["operation"])
        self.assertEqual(result["state"]["kind"], "accepted")
        self.assertEqual(resumed.call("settings/tool-labels/set", **params), result)
        self.assertEqual(json.loads(path.read_text()), {"tool_output":"full","themes":{"phone":"flow"},"ai_tool_labels":False})
        self.assertEqual(fixture.native.prompts, 0)

    def test_daemon_worker_updates_both_clients_and_preserves_exact_source_and_cached_resume(self):
        fixture, first, other, session = self.label_clients("label-worker-cache", None)
        row = self.label_send(first, session)
        source = first.call("entry/source", session=session, entry=row["id"], cursor=None, limit=65536)["text"]
        self.assertIn(OUTPUT, source)
        wait_for(lambda: (fixture.root / "summary-requests.jsonl").exists(), "daemon-owned label child")
        self.assertEqual(len((fixture.root / "summary-requests.jsonl").read_text().splitlines()), 1)
        (fixture.root / "release-summary").touch()
        generated = wait_for(lambda: next((row for row in self.label_rows(other, session) if row["tool"]["label_source"] == "ai"), None), "shared generated label")
        self.assertIn(LABEL, generated["tool"]["label"])
        self.assertEqual(first.call("entry/source", session=session, entry=generated["id"], cursor=None, limit=65536)["text"], source)
        disabled = self.label_params(first, session, False, counter=3, save=False)
        first.call("settings/tool-labels/set", **disabled)
        self.assertEqual(first.outcome(disabled["operation"])["state"]["kind"], "accepted")
        self.assertTrue(all(row["tool"]["label_source"] != "ai" for row in self.label_rows(other, session)))
        enabled = self.label_params(first, session, True, counter=4, save=False)
        first.call("settings/tool-labels/set", **enabled)
        self.assertEqual(first.outcome(enabled["operation"])["state"]["kind"], "accepted")
        wait_for(lambda: any(row["tool"]["label_source"] == "ai" for row in self.label_rows(other, session)), "known label restored without inference")
        fixture.stop()
        fixture.start()
        restored = Client(fixture, "label-restored")
        self.addCleanup(restored.close)
        restored.call("session/subscribe", session=session, since=None)
        wait_for(lambda: any(row["tool"]["label_source"] == "ai" for row in self.label_rows(restored, session)), "cached native history label")
        self.assertEqual(len((fixture.root / "summary-requests.jsonl").read_text().splitlines()), 1)
        self.assertEqual(fixture.native.prompts, 1)

    def test_disable_cancels_the_label_child_and_never_saves_its_late_result(self):
        fixture, first, other, session = self.label_clients("label-worker-cancel")
        self.label_send(first, session)
        wait_for(lambda: (fixture.root / "summary-requests.jsonl").exists(), "held label child")
        self.label_controls(fixture, other, session)
        params = self.label_params(first, session, False, counter=3, save=False)
        first.call("settings/tool-labels/set", **params)
        self.assertEqual(first.outcome(params["operation"])["state"]["kind"], "accepted")
        (fixture.root / "release-summary").touch()
        self.assertFalse(other.call("settings/read", session=session)["settings"]["summaries_enabled"])
        self.assertTrue(all(row["tool"]["label_source"] != "ai" for row in self.label_rows(other, session)))
        database = Path(fixture.env["XDG_STATE_HOME"]) / "carlos/tool-summaries.sqlite3"
        with closing(sqlite3.connect(database)) as connection:
            self.assertEqual(connection.execute("SELECT (SELECT count(*) FROM summaries) + (SELECT count(*) FROM source_labels)").fetchone()[0], 0)
        child = json.loads((fixture.root / "summary-child.json").read_text())["pid"]
        wait_for(lambda: not alive(child), "disabled label child was killed and reaped")
        self.assertEqual(fixture.native.prompts, 1)

    def test_uncached_history_and_reenable_never_start_new_label_inference(self):
        fixture, first, other, session = self.label_clients("label-uncached-history", "off", history=True)
        rows = wait_for(lambda: self.label_rows(first, session), "replayed literal tool")
        self.assertTrue(all(row["tool"]["label_source"] != "ai" for row in rows))
        params = self.label_params(first, session, True, save=False)
        first.call("settings/tool-labels/set", **params)
        self.assertEqual(first.outcome(params["operation"])["state"]["kind"], "accepted")
        self.assertTrue(other.call("settings/read", session=session)["settings"]["summaries_enabled"])
        self.assertTrue(all(row["tool"]["label_source"] != "ai" for row in self.label_rows(other, session)))
        self.assertFalse((fixture.root / "summary-requests.jsonl").exists())
        self.assertEqual(fixture.native.prompts, 0)

    def test_tool_labels_disk_error_keeps_live_preference_and_saved_default_applies_on_restore(self):
        fixture, first, _, session = self.label_clients("label-default-error", None)
        path = self.label_preferences(fixture)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.mkdir()
        params = self.label_params(first, session, False)
        first.call("settings/tool-labels/set", **params)
        result = first.outcome(params["operation"])["state"]
        self.assertEqual(result["kind"], "accepted")
        self.assertIn("not saved", result["notice"])
        self.assertFalse(first.call("settings/read", session=session)["settings"]["summaries_enabled"])
        path.rmdir()
        params = self.label_params(first, session, False, counter=3)
        first.call("settings/tool-labels/set", **params)
        self.assertEqual(first.outcome(params["operation"])["state"]["kind"], "accepted")
        fixture.stop()
        fixture.start()
        restored = Client(fixture, "default-restored")
        self.addCleanup(restored.close)
        restored.call("session/subscribe", session=session, since=None)
        self.assertFalse(restored.call("settings/read", session=session)["settings"]["summaries_enabled"])
        self.assertFalse((fixture.root / "summary-requests.jsonl").exists())


if __name__ == "__main__":
    class StandaloneToolLabelTests(ToolLabelTests, unittest.TestCase):
        pass

    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=support.BINARY)
    parser.add_argument("--artifacts", type=Path, default=support.ARTIFACTS)
    arguments, remaining = parser.parse_known_args()
    support.BINARY, support.ARTIFACTS = arguments.binary.resolve(), arguments.artifacts.resolve()
    unittest.main(argv=[__file__, *remaining])
