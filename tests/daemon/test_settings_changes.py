"""Check private settings intents, native scopes and saved defaults through real sockets."""

import asyncio
import json
from pathlib import Path

from support import Client, Fixture, wait_for


class SettingsChangeTests:
    def settings_clients(self, name):
        fixture = Fixture(name, codex=True)
        self.addCleanup(fixture.close)
        fixture.native.settings_enabled = True
        fixture.native.models = [{"model":name, "displayName":name, "isDefault":index == 0,
                                  "supportedReasoningEfforts":[{"reasoningEffort":"low"}, {"reasoningEffort":"high"}]}
                                 for index, name in enumerate(("fixture-model", "chosen", "other"))]
        first, other = Client(fixture, "settings-owner"), Client(fixture, "settings-observer")
        self.addCleanup(first.close)
        self.addCleanup(other.close)
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="codex", cwd=str(fixture.root), name=None)["session"]
        self.assertEqual(first.outcome(create)["state"]["kind"], "accepted")
        first.call("session/subscribe", session=session, since=None)
        other.call("session/subscribe", session=session, since=None)
        wait_for(lambda: first.call("settings/read", session=session)["settings"]["model_count"] == 3, "startup catalog")
        self.settings_announce(fixture, model="fixture-model", effort="low", summary=None)
        async def barrier():
            for link in list(fixture.native.connections):
                await fixture.native.notify(link, "item/agentMessage/delta", itemId="settings-fence", delta="settings startup fence")
        asyncio.run_coroutine_threadsafe(barrier(), fixture.native.loop).result(timeout=3)
        wait_for(lambda: any("settings startup fence" in row["text"] for row in
                            first.call("session/subscribe", session=session, since=None)["snapshot"]["entries"]),
                 "native settings notice precedes its transcript barrier")
        facts = wait_for(lambda: self.settings_current(first, session, "fixture-model"), "native settings fence")
        fixture.settings_startup_writes = len([r for r in fixture.native.requests if r.get("method") == "thread/settings/update"])
        return fixture, first, other, session, facts

    @staticmethod
    def settings_announce(fixture, **choices):
        async def announce():
            fixture.native.native_runtime.update(choices)
            for link in list(fixture.native.connections):
                await fixture.native.notify(link, "thread/settings/updated", threadSettings=choices)
        asyncio.run_coroutine_threadsafe(announce(), fixture.native.loop).result(timeout=3)

    @staticmethod
    def settings_current(client, session, model):
        facts = client.call("settings/read", session=session)["settings"]
        return facts if facts["current"]["model"] == model and not facts["native_pending"] else None

    @staticmethod
    def settings_path(fixture):
        return Path(fixture.env["XDG_CONFIG_HOME"]) / "carlos/runtime-defaults.json"

    @staticmethod
    def settings_writes(fixture):
        writes = [r for r in fixture.native.requests if r.get("method") == "thread/settings/update"]
        return writes[fixture.settings_startup_writes:]

    @staticmethod
    def settings_params(client, session, facts, counter=2, save=True):
        return dict(session=session, operation=client.operation(counter), expected=facts["revision"],
                    change={"model":{"kind":"value", "value":"chosen"}, "effort":{"kind":"value", "value":"high"}}, save_default=save)

    def test_settings_mask_and_idempotent_result_preserve_unknown_summary_and_private_defaults(self):
        fixture, first, other, session, facts = self.settings_clients("settings-save")
        params = self.settings_params(first, session, facts)
        first.call("settings/set", **params)
        result = first.outcome(params["operation"])
        self.assertEqual(result["state"]["kind"], "accepted")
        self.assertIn("Saved defaults", result["state"]["notice"])
        self.assertEqual(first.call("settings/set", **params), result)
        writes = self.settings_writes(fixture)
        self.assertEqual([r["params"] for r in writes], [{"threadId":"native-root", "model":"chosen", "effort":"high"}])
        current = wait_for(lambda: self.settings_current(other, session, "chosen"), "shared accepted settings")
        self.assertIsNone(current["current"]["summary"])
        self.assertEqual(json.loads(self.settings_path(fixture).read_text()), {"model":"chosen", "effort":"high", "summary":None})
        other.call("sessions/watch")
        self.assertFalse(any(m.get("method") == "operation/updated" for m in other.messages))

    def test_settings_reject_invalid_or_stale_drafts_before_native_writes(self):
        fixture, first, _, session, facts = self.settings_clients("settings-invalid")
        for counter, change in enumerate((
            {"model":{"kind":"value", "value":"not-advertised"}},
            {"effort":{"kind":"value", "value":"impossible"}},
            {"summary":{"kind":"value", "value":"unknown"}},
            {"model":{"kind":"inherit"}},
        ), start=2):
            params = self.settings_params(first, session, facts, counter)
            params["change"] = change
            first.call("settings/set", **params)
            self.assertEqual(first.outcome(params["operation"])["state"]["kind"], "rejected")
        self.settings_announce(fixture, model="other", effort="low")
        wait_for(lambda: self.settings_current(first, session, "other"), "new settings precondition")
        params = self.settings_params(first, session, facts, 6)
        first.call("settings/set", **params)
        self.assertIn("settings changed", first.outcome(params["operation"])["state"]["message"])
        self.assertEqual(self.settings_writes(fixture), [])
        self.assertFalse(self.settings_path(fixture).exists())

    def test_held_settings_keep_controls_and_resume_alive_without_overtaking_or_resending(self):
        fixture, first, other, session, _ = self.settings_clients("settings-held")
        async def facts_before_save():
            for link in list(fixture.native.connections):
                await fixture.native.held_facts(link)
        asyncio.run_coroutine_threadsafe(facts_before_save(), fixture.native.loop).result(timeout=3)
        card = other.message(lambda m: m.get("params", {}).get("event", {}).get("kind") == "approval_requested")["params"]["event"]["approval"]
        facts = first.call("settings/read", session=session)["settings"]
        fixture.native.settings_hold.add("thread/settings/update")
        fixture.native.settings_errors["turn/settings/update"] = {"code":-32602, "message":"live choice rejected"}
        params = self.settings_params(first, session, facts)
        first.call("settings/set", **params)
        wait_for(lambda: fixture.native.settings_pending, "native future reply held")
        queued = other.operation(1)
        other.call("session/send", session=session, operation=queued, text="after settings")
        turn = other.call("session/subscribe", session=session, since=None)["snapshot"]["info"]["current_turn"]
        interrupt, answer = other.operation(2), other.operation(3)
        other.call("session/interrupt", session=session, operation=interrupt, turn=turn)
        other.call("session/approve", session=session, operation=answer, approval=card["id"], answer={"kind":"decision", "decision":"once"})
        self.assertEqual(other.outcome(interrupt)["state"]["kind"], "accepted")
        self.assertEqual(other.outcome(answer)["state"]["kind"], "accepted")
        self.assertFalse(any(r.get("method") in ("turn/start", "turn/steer") for r in fixture.native.requests))
        self.assertFalse(self.settings_path(fixture).exists())
        first.close()
        resumed = Client(fixture, "settings-resumed", resume=first.hello)
        self.addCleanup(resumed.close)
        self.assertEqual(resumed.call("operation/read", operation=params["operation"])["state"]["kind"], "pending")
        fixture.native.release_settings()
        result = resumed.outcome(params["operation"])["state"]
        self.assertEqual(result["kind"], "accepted")
        self.assertIn("live choice rejected", result["notice"])
        self.assertEqual(other.outcome(queued)["state"]["kind"], "accepted")
        self.assertEqual(len(self.settings_writes(fixture)), 1)

    def test_rejected_settings_do_not_save_defaults_or_change_shared_choices(self):
        fixture, first, other, session, facts = self.settings_clients("settings-rejected")
        fixture.native.settings_errors["thread/settings/update"] = {"code":-32602, "message":"PRIVATE_SETTINGS_REJECTION"}
        params = self.settings_params(first, session, facts)
        first.call("settings/set", **params)
        result = first.outcome(params["operation"])["state"]
        self.assertEqual(result["kind"], "rejected")
        self.assertIn("PRIVATE_SETTINGS_REJECTION", result["message"])
        self.assertTrue(self.settings_current(other, session, "fixture-model"))
        self.assertFalse(self.settings_path(fixture).exists())

    def test_settings_defaults_failure_keeps_native_success_and_newer_notice_cannot_rewrite_captured_defaults(self):
        fixture, first, other, session, facts = self.settings_clients("settings-captured")
        fixture.native.settings_hold.add("thread/settings/update")
        params = self.settings_params(first, session, facts)
        first.call("settings/set", **params)
        wait_for(lambda: fixture.native.settings_pending, "requested choices captured")
        self.settings_announce(fixture, model="other", effort="low", summary=None)
        wait_for(lambda: other.call("settings/read", session=session)["settings"]["current"]["model"] == "other", "newer notice")
        fixture.native.release_settings()
        self.assertEqual(first.outcome(params["operation"])["state"]["kind"], "accepted")
        self.assertTrue(self.settings_current(other, session, "other"))
        self.assertEqual(json.loads(self.settings_path(fixture).read_text())["model"], "chosen")
        self.settings_path(fixture).unlink()
        self.settings_path(fixture).mkdir()
        facts = other.call("settings/read", session=session)["settings"]
        params = self.settings_params(first, session, facts, 3)
        first.call("settings/set", **params)
        result = first.outcome(params["operation"])["state"]
        self.assertEqual(result["kind"], "accepted")
        self.assertIn("defaults were not saved", result["notice"])
        self.assertTrue(self.settings_path(fixture).is_dir())

    def test_settings_malformed_ack_or_connection_loss_stays_uncertain_without_saving_or_resending(self):
        for name, disconnect in (("settings-malformed", False), ("settings-disconnected", True)):
            with self.subTest(name=name):
                fixture, first, other, session, facts = self.settings_clients(name)
                if disconnect:
                    fixture.native.settings_hold.add("thread/settings/update")
                else:
                    async def malformed(link, request):
                        await fixture.native.send(link, {"id":request["id"], "result":"not an acknowledgement"})
                    fixture.native.settings_reply = malformed
                params = self.settings_params(first, session, facts)
                first.call("settings/set", **params)
                if disconnect:
                    wait_for(lambda: fixture.native.settings_pending, "native write awaits reply")
                    fixture.native.disconnect()
                state = first.outcome(params["operation"])["state"]
                self.assertEqual(state["kind"], "uncertain")
                self.assertFalse(self.settings_path(fixture).exists())
                first.close()
                resumed = Client(fixture, name + "-resumed", resume=first.hello)
                self.addCleanup(resumed.close)
                self.assertEqual(resumed.call("operation/read", operation=params["operation"])["state"], state)
                other.call("sessions/watch")
                self.assertEqual(len(self.settings_writes(fixture)), 1)

    def test_settings_unsupported_api_keeps_only_explicit_next_turn_overrides(self):
        fixture, first, other, session, facts = self.settings_clients("settings-legacy")
        fixture.native.settings_errors["thread/settings/update"] = {"code":-32601, "message":"method not found"}
        params = self.settings_params(first, session, facts, save=False)
        first.call("settings/set", **params)
        state = first.outcome(params["operation"])["state"]
        self.assertEqual(state["kind"], "accepted")
        self.assertIn("direct next-turn overrides", state["notice"])
        current = wait_for(lambda: self.settings_current(other, session, "chosen"), "local future fallback")
        self.assertTrue(current["legacy_overrides"])
        send = first.operation(3)
        first.call("session/send", session=session, operation=send, text="legacy settings prompt")
        self.assertEqual(first.outcome(send)["state"]["kind"], "accepted")
        prompt = next(r for r in fixture.native.requests if r.get("method") == "turn/start")
        self.assertEqual((prompt["params"]["model"], prompt["params"]["effort"]), ("chosen", "high"))
        self.assertNotIn("summary", prompt["params"])
        self.assertFalse(self.settings_path(fixture).exists())
