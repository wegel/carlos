#!/usr/bin/env python3
"""Check saved startup choices through the real daemon and deterministic native adapters."""

import argparse
import json
import os
from pathlib import Path
import unittest

import support
from support import Client, Fixture, wait_for


class StartupDefaultTests(unittest.TestCase):
    def fixture(self, name, codex=True):
        fixture = Fixture("startup-" + name, codex=codex)
        self.addCleanup(fixture.close)
        fixture.stop()
        fixture.env.pop("CARLOS_REASONING_SUMMARY", None)
        fixture.start()
        if fixture.native:
            fixture.native.settings_enabled = True
        return fixture

    def preferences(self, fixture, value):
        path = Path(fixture.env["XDG_CONFIG_HOME"]) / "carlos/runtime-defaults.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(value) if isinstance(value, dict) else value)
        return path

    def client(self, fixture, name="reader"):
        client = Client(fixture, name)
        self.addCleanup(client.close)
        return client

    def create(self, fixture, client, backend="codex", counter=1):
        operation = client.operation(counter)
        session = client.call("session/create", operation=operation, backend=backend,
                              cwd=str(fixture.root), name=None)["session"]
        self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
        return session

    def settled(self, client, session, expected):
        def read():
            page = client.call("settings/read", session=session)
            settings = page["settings"]
            snapshot = client.call("session/subscribe", session=session, since=None)["snapshot"]
            return page if (settings["current"] == expected and not settings["native_pending"]
                            and not any(settings["pending"].values())
                            and snapshot["info"]["availability"] == "ready") else None
        return wait_for(read, "startup settings settle to " + repr(expected))

    def updates(self, fixture):
        return [request for request in fixture.native.requests
                if request.get("method") == "thread/settings/update"]

    def test_new_codex_applies_saved_compatible_choices_and_keeps_native_summary(self):
        fixture = self.fixture("saved")
        fixture.native.models += [model("saved", ["low", "max"])]
        fixture.native.native_runtime["summary"] = "native-summary"
        self.preferences(fixture, {"model":"saved", "effort":"MAX", "summary":"detailed"})
        client = self.client(fixture)
        session = self.create(fixture, client)
        self.settled(client, session, {"model":"saved", "effort":"max", "summary":"native-summary"})
        self.assertEqual([request["params"] for request in self.updates(fixture)],
                         [{"threadId":"native-root", "model":"saved", "effort":"max"}])
        self.assertEqual(fixture.native.settings_confirmed, [self.updates(fixture)[0]["params"]])
        send = client.operation(2)
        client.call("session/send", session=session, operation=send, text="saved startup")
        self.assertEqual(client.outcome(send)["state"]["kind"], "accepted")
        prompt = next(request for request in fixture.native.requests if request.get("method") == "turn/start")
        self.assertFalse(any(key in prompt["params"] for key in ("model", "effort", "summary")))
        self.assertEqual(len(self.updates(fixture)), 1)

    def test_new_codex_filters_unsupported_and_unadvertised_effort(self):
        for efforts, requested, expected in [(["low", "high"], "ultra", "high"), ([], "high", None)]:
            with self.subTest(efforts=efforts):
                fixture = self.fixture("effort-" + str(len(efforts)))
                fixture.native.models = [model("selected", efforts)]
                fixture.native.native_runtime["summary"] = "concise"
                self.preferences(fixture, {"model":"selected", "effort":requested})
                client = self.client(fixture)
                session = self.create(fixture, client)
                # A model without effort keeps the observed native low fact, but receives no override.
                current_effort = "low" if expected is None else expected
                self.settled(client, session, {"model":"selected", "effort":current_effort, "summary":"concise"})
                params = self.updates(fixture)[0]["params"]
                self.assertEqual(params["model"], "selected")
                self.assertEqual(params.get("effort"), expected)
                self.assertEqual(len(self.updates(fixture)), 1)

    def test_missing_or_malformed_defaults_fill_only_the_existing_concise_summary(self):
        for contents in (None, "not JSON"):
            with self.subTest(contents=contents):
                fixture = self.fixture("fallback-" + str(contents is None))
                if contents is not None:
                    self.preferences(fixture, contents)
                client = self.client(fixture)
                session = self.create(fixture, client)
                self.settled(client, session, {"model":"fixture-model", "effort":"low", "summary":"concise"})
                self.assertEqual([request["params"] for request in self.updates(fixture)],
                                 [{"threadId":"native-root", "summary":"concise"}])

    def test_resumed_codex_ignores_saved_choices_except_explicit_environment_summary(self):
        for override in (None, " none "):
            with self.subTest(override=override):
                fixture = self.fixture("resume-" + str(override is None))
                fixture.native.models += [model("saved", ["low", "high"])]
                self.preferences(fixture, {"model":"saved", "effort":"high", "summary":"detailed"})
                client = self.client(fixture)
                session = self.create(fixture, client)
                self.settled(client, session, {"model":"saved", "effort":"high", "summary":"detailed"})
                fixture.stop()
                fixture.native.native_runtime = {"model":"fixture-model", "effort":"low", "summary":None}
                self.preferences(fixture, {"model":"saved", "effort":"high", "summary":"auto"})
                before = len(self.updates(fixture))
                if override is not None:
                    fixture.env["CARLOS_REASONING_SUMMARY"] = override
                fixture.start()
                restored = self.client(fixture, "restored")
                restored.call("session/subscribe", session=session, since=None)
                summary = "none" if override is not None else None
                self.settled(restored, session, {"model":"fixture-model", "effort":"low", "summary":summary})
                writes = self.updates(fixture)[before:]
                self.assertEqual([request["params"] for request in writes],
                                 [{"threadId":"native-root", "summary":"none"}] if override else [])
                self.assertEqual(fixture.native.started, 1)
                self.assertEqual(fixture.native.resumed, 1)

    def test_empty_codex_catalog_uses_the_same_legacy_synthetic_fallback(self):
        fixture = self.fixture("empty-catalog")
        fixture.native.models = []
        fixture.native.native_runtime["summary"] = "concise"
        client = self.client(fixture)
        session = self.create(fixture, client)
        page = self.settled(client, session, {"model":"gpt-5.6-sol", "effort":"high", "summary":"concise"})
        self.assertEqual(page["settings"]["model_count"], 3)
        self.assertEqual([request["params"] for request in self.updates(fixture)],
                         [{"threadId":"native-root", "model":"gpt-5.6-sol", "effort":"high"}])

    def test_a_blocked_defaults_read_leaves_manager_and_existing_owner_readable(self):
        fixture = self.fixture("blocked-file")
        first = self.client(fixture)
        existing = self.create(fixture, first)
        self.settled(first, existing, {"model":"fixture-model", "effort":"low", "summary":"concise"})
        path = self.preferences(fixture, {})
        path.unlink()
        os.mkfifo(path)
        operation = first.operation(2)
        pending = first.call("session/create", operation=operation, backend="codex",
                             cwd=str(fixture.root), name=None)["session"]
        descriptor = None
        try:
            def writer_ready():
                try:
                    return os.open(path, os.O_WRONLY | os.O_NONBLOCK)
                except OSError:
                    return None
            descriptor = wait_for(writer_ready, "lifecycle worker entered the defaults read")
            second = self.client(fixture, "while-blocked")
            rows = second.call("sessions/watch")["sessions"]
            self.assertEqual(next(row for row in rows if row["id"] == pending)["availability"], "starting")
            self.assertEqual(first.call("operation/read", operation=operation)["state"]["kind"], "pending")
            self.assertEqual(second.call("settings/read", session=existing)["settings"]["current"]["model"], "fixture-model")
            os.write(descriptor, b"{}")
        finally:
            if descriptor is not None:
                os.close(descriptor)
            path.unlink(missing_ok=True)
            path.write_text("{}")
        self.assertEqual(first.outcome(operation)["state"]["kind"], "accepted")
        self.settled(first, pending, {"model":"fixture-model", "effort":"low", "summary":"concise"})

    def test_claude_and_grok_keep_their_existing_saved_choice_rules_on_create_and_restore(self):
        for backend, name, effort in [("claude", "sdk-opus", "max"), ("grok", "grok-4.5", "low")]:
            with self.subTest(backend=backend):
                fixture = self.fixture(backend, codex=False)
                self.preferences(fixture, {"model":name, "effort":effort, "summary":"detailed"})
                client = self.client(fixture)
                session = self.create(fixture, client, backend)
                expected = {"model":name, "effort":effort, "summary":None}
                self.settled(client, session, expected)
                self.assert_native_prompt(fixture, client, session, backend, name, effort, 2)
                fixture.stop()
                (fixture.root / "release-history").touch()
                fixture.start()
                restored = self.client(fixture, "restored")
                restored.call("session/subscribe", session=session, since=None)
                self.settled(restored, session, expected)
                self.assert_native_prompt(fixture, restored, session, backend, name, effort, 1)

    def assert_native_prompt(self, fixture, client, session, backend, name, effort, counter):
        operation = client.operation(counter)
        client.call("session/send", session=session, operation=operation, text="saved " + backend)
        self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
        if backend == "grok":
            writes = fixture.native_requests("session/set_model")
            self.assertEqual(writes[-1]["params"]["modelId"], name)
        else:
            def controls(subtype):
                records = [json.loads(line) for line in (fixture.root / "rpc.jsonl").read_text().splitlines()]
                return [row["message"] for row in records if row["direction"] == "client"
                        and row["message"].get("request", {}).get("subtype") == subtype]
            self.assertEqual(controls("set_model")[-1]["request"]["model"], name)
            self.assertEqual(controls("apply_flag_settings")[-1]["request"]["settings"]["effortLevel"], effort)


def model(name, efforts):
    return {"model":name, "displayName":name, "isDefault":True,
            "defaultReasoningEffort":"low", "supportedReasoningEfforts":[
                {"reasoningEffort":effort} for effort in efforts]}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--artifacts", required=True, type=Path)
    args, tests = parser.parse_known_args()
    support.BINARY = args.binary.resolve()
    support.ARTIFACTS = args.artifacts.resolve()
    unittest.main(argv=[__file__, *tests])
