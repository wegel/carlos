"""Check Claude and Grok settings through sockets and the compiled native Claude controller."""

import json
from pathlib import Path
import unittest

import support
from support import Client, Fixture, wait_for


class BackendSettingsTests:
    def clients(self, name, backend="claude"):
        fixture = Fixture(name, mode="claude-settings-normal" if backend == "claude" else "modern")
        self.addCleanup(fixture.close)
        first, other = Client(fixture, "settings-owner"), Client(fixture, "settings-observer")
        self.addCleanup(first.close)
        self.addCleanup(other.close)
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend=backend, cwd=str(fixture.root), name=None)["session"]
        self.assertEqual(first.outcome(create)["state"]["kind"], "accepted")
        first.call("session/subscribe", session=session, since=None)
        other.call("session/subscribe", session=session, since=None)
        wait_for(lambda: first.call("settings/read", session=session)["settings"]["model_count"] >= 2, "native model catalogue")
        return fixture, first, other, session

    @staticmethod
    def preferences(fixture):
        return Path(fixture.env["XDG_CONFIG_HOME"]) / "carlos/runtime-defaults.json"

    @staticmethod
    def native_controls(fixture, subtype):
        path = fixture.root / "rpc.jsonl"
        return [record["message"] for line in path.read_text().splitlines()
                if (record := json.loads(line))["direction"] == "client"
                and record["message"].get("request", {}).get("subtype") == subtype]

    @staticmethod
    def helper_requests(fixture, method):
        path = fixture.root / "claude-controller.jsonl"
        return [record["message"] for line in path.read_text().splitlines()
                if (record := json.loads(line))["direction"] == "client" and record["message"].get("method") == method]

    @staticmethod
    def params(client, session, change=None, counter=2, save=True):
        facts = client.call("settings/read", session=session)["settings"]
        return dict(session=session, operation=client.operation(counter), expected=facts["revision"],
                    change=change if change is not None else {"model":{"kind":"value", "value":"sdk-opus"},
                                                            "effort":{"kind":"value", "value":"high"}}, save_default=save)

    def test_claude_exact_native_choices_private_result_and_captured_defaults(self):
        fixture, first, other, session = self.clients("claude-settings-save")
        params = self.params(first, session)
        first.call("settings/set", **params)
        outcome = first.outcome(params["operation"])
        self.assertEqual(outcome["state"]["kind"], "accepted")
        self.assertEqual([r["params"] for r in self.helper_requests(fixture, "model.set")], [{"model":"sdk-opus"}])
        self.assertEqual([r["params"] for r in self.helper_requests(fixture, "effort.set")], [{"effort":"high"}])
        self.assertEqual(json.loads(self.preferences(fixture).read_text()), {"model":"sdk-opus", "effort":"high", "summary":None})
        self.assertEqual(first.call("settings/set", **params), outcome)
        other.call("sessions/watch")
        self.assertFalse(any(m.get("method") == "operation/updated" for m in other.messages))
        current = other.call("settings/read", session=session)["settings"]["current"]
        self.assertEqual(current, {"model":"sdk-opus", "effort":"high", "summary":None})

    def test_claude_appearance_and_explicit_inherit_keep_native_in_charge(self):
        fixture, first, _, session = self.clients("claude-settings-inherit")
        params = self.params(first, session, {})
        first.call("settings/set", **params)
        self.assertEqual(first.outcome(params["operation"])["state"]["kind"], "accepted")
        self.assertEqual(json.loads(self.preferences(fixture).read_text()), {"model":None, "effort":None, "summary":None})
        self.assertEqual(self.helper_requests(fixture, "model.set"), [])
        explicit = self.params(first, session, counter=3)
        first.call("settings/set", **explicit)
        self.assertEqual(first.outcome(explicit["operation"])["state"]["kind"], "accepted")
        inherit = self.params(first, session, {"model":{"kind":"inherit"}, "effort":{"kind":"inherit"}}, counter=4)
        first.call("settings/set", **inherit)
        self.assertEqual(first.outcome(inherit["operation"])["state"]["kind"], "accepted")
        self.assertEqual(self.helper_requests(fixture, "model.set")[-1]["params"], {"model":None})
        self.assertEqual(self.helper_requests(fixture, "effort.set")[-1]["params"], {"effort":None})
        self.assertEqual(json.loads(self.preferences(fixture).read_text()), {"model":None, "effort":None, "summary":None})

    def test_claude_partial_refusal_keeps_confirmed_model_and_never_saves_defaults(self):
        fixture, first, _, session = self.clients("claude-settings-partial")
        (fixture.root / "settings-config.json").write_text(json.dumps({"reject":["apply_flag_settings"]}))
        params = self.params(first, session)
        first.call("settings/set", **params)
        outcome = first.outcome(params["operation"])["state"]
        self.assertEqual(outcome["kind"], "uncertain")
        self.assertIn("confirmed model", outcome["message"])
        self.assertIn("PRIVATE_SETTINGS_REJECTION", outcome["message"])
        current = first.call("settings/read", session=session)["settings"]["current"]
        self.assertEqual(current["model"], "sdk-opus")
        self.assertIsNone(current["effort"])
        self.assertFalse(self.preferences(fixture).exists())

    def test_claude_held_change_keeps_controls_fifo_and_reconnect_without_resend(self):
        fixture, first, other, session = self.clients("claude-settings-held")
        send = first.operation(2)
        first.call("session/send", session=session, operation=send, text="hold this turn")
        self.assertEqual(first.outcome(send)["state"]["kind"], "accepted")
        turn = wait_for(lambda: other.call("session/subscribe", session=session, since=None)["snapshot"]["info"]["current_turn"], "running turn")
        (fixture.root / "settings-config.json").write_text(json.dumps({"hold":["set_model"], "approval_on_hold":True}))
        params = self.params(first, session, counter=3)
        first.call("settings/set", **params)
        wait_for(lambda: (fixture.root / "settings-held").exists(), "held native model")
        self.assertTrue(other.call("settings/read", session=session)["settings"]["native_pending"])
        queued = other.operation(1)
        other.call("session/send", session=session, operation=queued, text="after settings")
        card = other.message(lambda message: message.get("params", {}).get("event", {}).get("kind") == "approval_requested")["params"]["event"]["approval"]
        approval = other.operation(2)
        other.call("session/approve", session=session, operation=approval, approval=card["id"],
                   answer={"kind":"decision", "decision":"once"})
        self.assertEqual(other.outcome(approval)["state"]["kind"], "accepted")
        interrupt = other.operation(3)
        other.call("session/interrupt", session=session, operation=interrupt, turn=turn)
        self.assertEqual(other.outcome(interrupt)["state"]["kind"], "accepted")
        self.assertEqual(len(self.native_controls(fixture, "interrupt")), 1)
        self.assertIn(other.call("operation/read", operation=queued)["state"]["kind"], ("pending", "queued"))
        hello = first.hello
        first.close()
        resumed = Client(fixture, "settings-resumed", resume=hello)
        self.addCleanup(resumed.close)
        (fixture.root / "release-settings").write_text("go")
        self.assertEqual(resumed.outcome(params["operation"])["state"]["kind"], "accepted")
        self.assertEqual(other.outcome(queued)["state"]["kind"], "accepted")
        self.assertEqual(resumed.call("settings/set", **params)["state"]["kind"], "accepted")
        self.assertEqual(len(self.helper_requests(fixture, "model.set")), 1)

    def test_claude_disconnect_is_uncertain_and_disk_failure_remains_native_success(self):
        for failure in ("closed", "disk"):
            with self.subTest(failure=failure):
                fixture, first, _, session = self.clients("claude-settings-" + failure)
                if failure == "closed":
                    (fixture.root / "settings-config.json").write_text(json.dumps({"close":["set_model"]}))
                else:
                    path = self.preferences(fixture)
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.mkdir()
                params = self.params(first, session)
                first.call("settings/set", **params)
                outcome = first.outcome(params["operation"])["state"]
                self.assertEqual(outcome["kind"], "uncertain" if failure == "closed" else "accepted")
                self.assertFalse(self.preferences(fixture).is_file())
                if failure == "disk":
                    self.assertIn("not saved", outcome["notice"])
                self.assertEqual(len(self.helper_requests(fixture, "model.set")), 1)

    def test_claude_root_reset_retires_the_original_settings_without_defaults_or_effort(self):
        fixture, first, _, session = self.clients("claude-settings-reset")
        (fixture.root / "settings-config.json").write_text(json.dumps({"reset_on_model":True}))
        params = self.params(first, session)
        first.call("settings/set", **params)
        self.assertEqual(first.outcome(params["operation"])["state"]["kind"], "uncertain")
        self.assertEqual(len(self.helper_requests(fixture, "model.set")), 1)
        self.assertEqual(self.helper_requests(fixture, "effort.set"), [])
        self.assertFalse(self.preferences(fixture).exists())

    def test_grok_keeps_future_choices_until_prompt_and_rejects_unsupported_fields(self):
        fixture, first, _, session = self.clients("grok-settings-future", backend="grok")
        params = self.params(first, session, {"model":{"kind":"value", "value":"grok-4.5"},
                                            "effort":{"kind":"value", "value":"low"}})
        first.call("settings/set", **params)
        outcome = first.outcome(params["operation"])["state"]
        self.assertEqual(outcome["kind"], "accepted")
        self.assertIn("next prompt", outcome["notice"])
        self.assertEqual(fixture.native_requests("session/set_model"), [])
        facts = first.call("settings/read", session=session)["settings"]
        self.assertEqual(facts["pending"]["model"], "grok-4.5")
        self.assertNotEqual(facts["current"]["model"], "grok-4.5")
        self.assertEqual(json.loads(self.preferences(fixture).read_text()), {"model":"grok-4.5", "effort":"low", "summary":None})
        for counter, change in enumerate(({"summary":{"kind":"value", "value":"auto"}}, {"model":{"kind":"inherit"}}), 3):
            invalid = self.params(first, session, change, counter=counter)
            first.call("settings/set", **invalid)
            self.assertEqual(first.outcome(invalid["operation"])["state"]["kind"], "rejected")
        send = first.operation(5)
        first.call("session/send", session=session, operation=send, text="use future settings")
        self.assertEqual(first.outcome(send)["state"]["kind"], "accepted")
        self.assertEqual(fixture.native_requests("session/set_model")[-1]["params"]["modelId"], "grok-4.5")


if __name__ == "__main__":
    class StandaloneBackendSettingsTests(BackendSettingsTests, unittest.TestCase):
        pass

    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=support.BINARY)
    parser.add_argument("--artifacts", type=Path, default=support.ARTIFACTS)
    arguments, remaining = parser.parse_known_args()
    support.BINARY, support.ARTIFACTS = arguments.binary.resolve(), arguments.artifacts.resolve()
    unittest.main(argv=[__file__, *remaining])
