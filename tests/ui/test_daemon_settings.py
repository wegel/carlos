"""Keep settings drafts private while the actual daemon saves exact native choices."""

import asyncio
import fcntl
import json
from pathlib import Path
import socket

from test_daemon_client import daemon_support

CTRL_M = b"\x1b[109;5u"
F6 = b"\x1b[17~"


class DaemonSettings:
    def settings_terminal(self, name, no_effort=False):
        def configure(fixture):
            fixture.native.settings_enabled = True
            fixture.native.models = [{"model":model, "displayName":model, "isDefault":index == 0,
                                      "defaultReasoningEffort":"low", "supportedReasoningEfforts":[
                                          {"reasoningEffort":"low"}, {"reasoningEffort":"high"}]}
                                     for index, model in enumerate(("fixture-model", "chosen", "other"))]
            if no_effort:
                fixture.native.models[1].update(defaultReasoningEffort=None, supportedReasoningEfforts=[])
        fixture, client, session = self.daemon_session(name, configure=configure)
        daemon_support.wait_for(lambda: not client.call("settings/read", session=session)["settings"]["native_pending"], "startup settings settle")

        async def fence():
            fixture.native.native_runtime.update(model="fixture-model", effort="low", summary=None)
            for link in list(fixture.native.connections):
                await fixture.native.notify(link, "thread/settings/updated", threadSettings=dict(fixture.native.native_runtime))
                await fixture.native.notify(link, "item/agentMessage/delta", itemId="settings-ui-fence", delta="settings UI fence")
        asyncio.run_coroutine_threadsafe(fence(), fixture.native.loop).result(timeout=3)
        daemon_support.wait_for(lambda: any("settings UI fence" in row["text"] for row in
                                client.call("session/subscribe", session=session, since=None)["snapshot"]["entries"]), "settings notice precedes its transcript fence")
        terminal = self.attach_daemon(fixture, session, name)
        terminal.contains("settings UI fence")
        fixture.settings_ui_start = len(fixture.native.requests)
        return fixture, client, session, terminal

    @staticmethod
    def open_daemon_settings(terminal):
        terminal.send(CTRL_M)
        terminal.contains("Thinking")
        terminal.contains("Use thread setting")
        terminal.contains("Tab field, arrows, Enter save")

    @staticmethod
    def settings_defaults(fixture):
        return Path(fixture.env["XDG_CONFIG_HOME"]) / "carlos/runtime-defaults.json"

    @staticmethod
    def settings_native_writes(fixture):
        return [request for request in fixture.native.requests[fixture.settings_ui_start:]
                if request.get("method") == "thread/settings/update"]

    def test_daemon_settings_keep_two_editors_private_and_refresh_without_pinning_unknown_summary(self):
        fixture, client, session, first = self.settings_terminal("settings-private")
        second = self.attach_daemon(fixture, session, "settings-private-second")
        second.contains("settings UI fence")
        first.paste("first private draft 🌍")
        second.paste("second private draft 日本語")
        self.open_daemon_settings(first)
        self.open_daemon_settings(second)
        first.send(b"\x1b[C\t\x1b[C")
        first.contains("chosen")
        first.send(b"\r")
        first.wait(lambda: first.input_is("first private draft 🌍")
                   and "Settings" not in first.text,
                   "accepted save closes the first editor and keeps its composer")
        self.assertIn("fixture-model", second.text)
        self.assertNotIn("chosen", second.text)
        second.send(b"\r")
        second.contains("settings changed")
        second.send(F6)
        second.contains("chosen")
        second.send(b"\r")
        second.wait(lambda:second.input_is("second private draft 日本語"), "fresh save keeps second composer")
        self.assertEqual([request["params"] for request in self.settings_native_writes(fixture)],
                         [{"threadId":"native-root", "model":"chosen", "effort":"high"}])
        self.assertEqual(json.loads(self.settings_defaults(fixture).read_text()),
                         {"model":"chosen", "effort":"high", "summary":None})
        self.assertEqual(first.requests("session/send"), [])
        self.assertEqual(second.requests("session/send"), [])

    def test_daemon_settings_rejection_keeps_original_draft_until_explicit_refresh_and_save(self):
        fixture, client, session, terminal = self.settings_terminal("settings-rejected-ui")
        fixture.native.settings_errors["thread/settings/update"] = {"code":-32602, "message":"MODEL_CHOICE_REFUSED"}
        self.open_daemon_settings(terminal)
        terminal.send(b"\x1b[C\t\x1b[C\r")
        terminal.contains("MODEL_CHOICE_REFUSED")
        self.assertIn("chosen", terminal.text)
        self.assertIn("high", terminal.text)
        self.assertFalse(self.settings_defaults(fixture).exists())
        reads = len(terminal.requests("settings/read"))
        terminal.send(b"\x1b")
        terminal.wait(lambda:"Thinking" not in terminal.text, "rejected settings close before the next Ctrl+M")
        terminal.send(CTRL_M)
        terminal.contains("Thinking")
        terminal.contains("MODEL_CHOICE_REFUSED")
        self.assertIn("chosen", terminal.text)
        self.assertIn("high", terminal.text)
        self.assertEqual(len(terminal.requests("settings/read")), reads)
        self.assertEqual(len(self.settings_native_writes(fixture)), 1)
        fixture.native.settings_errors.clear()
        terminal.send(F6)
        terminal.wait(lambda:len(terminal.requests("settings/read")) > reads, "F6 starts only a fresh read")
        terminal.contains("Tab field, arrows, Enter save")
        self.assertIn("chosen", terminal.text)
        self.assertIn("high", terminal.text)
        terminal.send(b"\r")
        terminal.wait(lambda:self.settings_defaults(fixture).exists(), "explicit save accepts retained choices")
        self.assertEqual([request["params"] for request in self.settings_native_writes(fixture)],
                         [{"threadId":"native-root", "model":"chosen", "effort":"high"}] * 2)
        self.assertEqual(terminal.requests("session/send"), [])

    def test_daemon_settings_held_save_keeps_input_and_native_controls_responsive(self):
        fixture, client, session, terminal = self.settings_terminal("settings-held-ui")
        self.open_daemon_settings(terminal)
        fixture.native.settings_hold.add("thread/settings/update")
        terminal.send(b"\x1b[C\r")
        terminal.contains("Settings save pending")
        daemon_support.wait_for(lambda: fixture.native.settings_pending, "original future reply held")
        terminal.send(b"\x1b")
        terminal.wait(lambda:"Thinking" not in terminal.text, "settings close before typing")
        terminal.paste("typing while settings wait 日本語")
        terminal.wait(lambda:terminal.input_is("typing while settings wait 日本語"), "held save keeps private typing responsive")
        self.assertFalse(self.settings_defaults(fixture).exists())

        async def controls():
            for link in list(fixture.native.connections):
                await fixture.native.held_facts(link)
        asyncio.run_coroutine_threadsafe(controls(), fixture.native.loop).result(timeout=3)
        card = client.message(lambda m: m.get("params", {}).get("event", {}).get("kind") == "approval_requested")["params"]["event"]["approval"]
        snapshot = client.call("session/subscribe", session=session, since=None)["snapshot"]
        interrupt, answer = client.operation(2), client.operation(3)
        client.call("session/interrupt", session=session, operation=interrupt, turn=snapshot["info"]["current_turn"])
        client.call("session/approve", session=session, operation=answer, approval=card["id"], answer={"kind":"decision", "decision":"once"})
        self.assertEqual(client.outcome(interrupt)["state"]["kind"], "accepted")
        self.assertEqual(client.outcome(answer)["state"]["kind"], "accepted")
        fixture.native.release_settings()
        terminal.wait(lambda:self.settings_defaults(fixture).exists(), "defaults written after native acceptance")
        terminal.wait(lambda:terminal.input_is("typing while settings wait 日本語"), "original result keeps edited composer")
        self.assertEqual(len(self.settings_native_writes(fixture)), 1)
        self.assertEqual(terminal.requests("session/send"), [])

    def test_daemon_settings_restart_keeps_choices_without_automatically_saving_again(self):
        fixture, client, session, terminal = self.settings_terminal("settings-restart-ui")
        self.open_daemon_settings(terminal)
        fixture.native.settings_hold.add("thread/settings/update")
        terminal.send(b"\x1b[C\r")
        daemon_support.wait_for(lambda: fixture.native.settings_pending, "old save awaits native reply")
        old = terminal.requests("settings/set")[0]["params"]["operation"]
        fixture.stop()
        terminal.contains("F5 reconnects")
        fixture.native.settings_hold.clear()
        fixture.native.settings_pending.clear()
        fixture.start()
        terminal.send(b"\x1b[15~")
        terminal.contains("Daemon changed")
        self.assertIn("chosen", terminal.text)
        self.assertEqual(len(terminal.requests("settings/set")), 1)
        terminal.send(F6)
        terminal.contains("Tab field, arrows, Enter save")
        self.assertIn("chosen", terminal.text)
        terminal.send(b"\r")
        terminal.wait(lambda:self.settings_defaults(fixture).exists(), "explicit new-daemon save writes defaults")
        new = terminal.requests("settings/set")[-1]["params"]["operation"]
        self.assertNotEqual(new["client"], old["client"])
        self.assertEqual(len(terminal.requests("settings/set")), 2)
        self.assertEqual(terminal.requests("session/send"), [])

    def test_daemon_settings_save_labels_and_device_themes_after_the_captured_model_change(self):
        fixture, client, session, terminal = self.settings_terminal("settings-labels-ui")
        device = terminal.root / "config/carlos/ui-settings.json"
        device.parent.mkdir(parents=True, exist_ok=True)
        device.write_text(json.dumps({"unknown_device_key":"keep me", "layout":"auto"}))
        self.open_daemon_settings(terminal)
        terminal.send(b"\x1b[C" + b"\t" * 5 + b"\x1b[C\t\x1b[C\r")
        terminal.wait(lambda:"Thinking" not in terminal.text, "combined save closes the original editor")
        terminal.wait(lambda:device.exists() and "themes" in json.loads(device.read_text()), "device appearance writer finishes")
        host = Path(fixture.env["XDG_CONFIG_HOME"]) / "carlos/ui-settings.json"
        self.assertTrue(json.loads(host.read_text())["ai_tool_labels"])
        self.assertTrue(client.call("settings/read", session=session)["settings"]["summaries_enabled"])
        saved = json.loads(device.read_text())
        self.assertEqual(saved["unknown_device_key"], "keep me")
        self.assertNotIn("layout", saved)
        self.assertNotIn("ai_tool_labels", saved, "device appearance does not rewrite the host flag")
        self.assertEqual(saved["themes"]["desktop"], "flow")
        methods = [m["method"] for m in terminal.messages("client") if m.get("method") in ("settings/set", "settings/tool-labels/set")]
        self.assertEqual(methods, ["settings/set", "settings/tool-labels/set"])
        self.assertEqual([request["params"] for request in self.settings_native_writes(fixture)],
                         [{"threadId":"native-root", "model":"chosen", "effort":"low"}])
        self.assertEqual(terminal.requests("session/send"), [])

    def test_daemon_settings_native_refusal_still_saves_independent_labels_and_appearance(self):
        fixture, client, session, terminal = self.settings_terminal("settings-partial-ui")
        fixture.native.settings_errors["thread/settings/update"] = {"code":-32602, "message":"MODEL_REFUSED_LABELS_OK"}
        self.open_daemon_settings(terminal)
        terminal.send(b"\x1b[C" + b"\t" * 5 + b"\x1b[C\r")
        terminal.contains("MODEL_REFUSED_LABELS_OK")
        terminal.contains("chosen")
        host = Path(fixture.env["XDG_CONFIG_HOME"]) / "carlos/ui-settings.json"
        terminal.wait(lambda:host.exists(), "independent host label default is saved")
        self.assertTrue(json.loads(host.read_text())["ai_tool_labels"])
        self.assertTrue(client.call("settings/read", session=session)["settings"]["summaries_enabled"])
        self.assertFalse(self.settings_defaults(fixture).exists())
        self.assertEqual(len(self.settings_native_writes(fixture)), 1)
        self.assertEqual(len(terminal.requests("settings/tool-labels/set")), 1)
        self.assertEqual(terminal.requests("session/send"), [])

    def test_daemon_settings_held_device_file_does_not_block_typing_or_rewrite_native_choices(self):
        fixture, client, session, terminal = self.settings_terminal("settings-appearance-held")
        device = terminal.root / "config/carlos/ui-settings.json"
        device.parent.mkdir(parents=True, exist_ok=True)
        device.write_text(json.dumps({"keep":"old field"}))
        lock = (device.parent / ".ui-settings.json.lock").open("w")
        self.addCleanup(lock.close)
        fcntl.flock(lock, fcntl.LOCK_EX)
        self.open_daemon_settings(terminal)
        terminal.send(b"\r")
        terminal.wait(lambda:"Thinking" not in terminal.text, "accepted native save closes without waiting for the local file")
        terminal.paste("typing during a device file wait 日本語")
        terminal.wait(lambda:terminal.input_is("typing during a device file wait 日本語"), "device disk wait keeps typing responsive", timeout=1)
        self.assertEqual(json.loads(device.read_text()), {"keep":"old field"})
        self.open_daemon_settings(terminal)
        terminal.send(b"\x1b[C\r")
        terminal.contains("Finishing the previous save; these choices are queued")
        terminal.contains("chosen")
        self.assertEqual(len(terminal.requests("settings/set")), 1)
        self.assertEqual(self.settings_native_writes(fixture), [])
        fcntl.flock(lock, fcntl.LOCK_UN)
        terminal.wait(lambda:"themes" in json.loads(device.read_text()), "unlock lets the same appearance job finish")
        terminal.wait(lambda:"Thinking" not in terminal.text and terminal.input_is("typing during a device file wait 日本語"),
                      "the explicit second Save finishes after the first device write")
        self.assertEqual(json.loads(device.read_text())["keep"], "old field")
        self.assertEqual([request["params"] for request in self.settings_native_writes(fixture)],
                         [{"threadId":"native-root", "model":"chosen", "effort":"low"}])
        self.assertEqual(len(terminal.requests("settings/set")), 2)
        self.assertEqual(terminal.requests("session/send"), [])

    def test_daemon_settings_no_effort_model_does_not_pin_the_previous_model_effort(self):
        fixture, client, session, terminal = self.settings_terminal("settings-no-effort-ui", no_effort=True)
        self.open_daemon_settings(terminal)
        terminal.send(b"\x1b[C")
        terminal.contains("chosen")
        terminal.wait(lambda:"Thinking" not in terminal.text, "model without reasoning hides the effort field")
        terminal.send(b"\r")
        terminal.wait(lambda:"Settings" not in terminal.text, "no-effort model save closes")
        self.assertEqual([request["params"] for request in self.settings_native_writes(fixture)],
                         [{"threadId":"native-root", "model":"chosen"}])
        self.assertEqual(json.loads(self.settings_defaults(fixture).read_text()),
                         {"model":"chosen", "effort":None, "summary":None})
        self.assertEqual(terminal.requests("session/send"), [])

    def test_daemon_settings_refusal_after_closing_the_popup_stays_visible_and_keeps_input(self):
        fixture, client, session, terminal = self.settings_terminal("settings-closed-refusal-ui")
        self.open_daemon_settings(terminal)
        fixture.native.settings_hold.add("thread/settings/update")
        terminal.send(b"\x1b[C\r")
        daemon_support.wait_for(lambda:fixture.native.settings_pending, "original settings reply held")
        terminal.send(b"\x1b")
        terminal.wait(lambda:"Thinking" not in terminal.text, "close before the native result")
        terminal.paste("draft before late refusal 日本語")
        terminal.wait(lambda:terminal.input_is("draft before late refusal 日本語"), "composer remains usable")
        fixture.native.settings_errors["thread/settings/update"] = {"code":-32602, "message":"CLOSED_POPUP_REFUSAL"}
        fixture.native.release_settings()
        terminal.contains("CLOSED_POPUP_REFUSAL")
        self.assertTrue(terminal.input_is("draft before late refusal 日本語"))
        terminal.send(CTRL_M)
        terminal.contains("Thinking")
        terminal.contains("CLOSED_POPUP_REFUSAL")
        terminal.contains("chosen")
        self.assertEqual(len(self.settings_native_writes(fixture)), 1)
        self.assertEqual(terminal.requests("session/send"), [])

    def test_daemon_settings_same_daemon_reconnect_recovers_the_original_result_without_resending(self):
        fixture, client, session, terminal = self.settings_terminal("settings-private-resume-ui")
        terminal.paste("draft through private resume 🌍")
        self.open_daemon_settings(terminal)
        fixture.native.settings_hold.add("thread/settings/update")
        terminal.send(b"\x1b[C\r")
        daemon_support.wait_for(lambda:fixture.native.settings_pending, "original settings reply held")
        original = terminal.requests("settings/set")[0]["params"]["operation"]
        for peer in list(terminal.daemon_recorder.peers):
            try:
                peer.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
        terminal.contains("F5 reconnects")
        fixture.native.release_settings()
        daemon_support.wait_for(lambda:self.settings_defaults(fixture).exists(), "daemon finishes the original save while its terminal is offline")
        terminal.send(b"\x1b[15~")
        terminal.wait(lambda:terminal.input_is("draft through private resume 🌍") and "Thinking" not in terminal.text, "private Accepted result closes only its original editor")
        self.assertEqual([request["params"]["operation"] for request in terminal.requests("settings/set")], [original])
        self.assertEqual(len(self.settings_native_writes(fixture)), 1)
        self.assertEqual(len(terminal.requests("hello")), 2)
        self.assertEqual(terminal.requests("session/send"), [])
