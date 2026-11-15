"""Check next-launch context defaults in the actual socket settings editor."""

import json
from pathlib import Path
import sys

from test_daemon_client import daemon_support
from test_daemon_settings import CTRL_M, F6


class DaemonSettingsContext:
    def backend_settings_terminal(self, name, backend="claude", shell=None):
        runner = sys.modules[type(self).__module__]
        daemon_support.BINARY = runner.BINARY.parent / "carlosd"
        daemon_support.ARTIFACTS = runner.ARTIFACTS / "daemon"
        fixture = daemon_support.Fixture(name, mode="claude-settings-normal" if backend == "claude" else "modern")
        self.addCleanup(fixture.close)
        fixture.stop()
        fixture.env.pop("CLAUDE_CODE_AUTO_COMPACT_WINDOW", None)
        if shell is not None:
            fixture.env["CLAUDE_CODE_AUTO_COMPACT_WINDOW"] = shell
        fixture.start()
        client = daemon_support.Client(fixture, "settings-context-owner")
        self.addCleanup(client.close)
        create = client.operation(1)
        session = client.call("session/create", operation=create, backend=backend,
                              cwd=str(fixture.root), name="Context fixture")["session"]
        self.assertEqual(client.outcome(create)["state"]["kind"], "accepted")
        daemon_support.wait_for(lambda: client.call("session/subscribe", session=session, since=None)["snapshot"]["info"]["availability"] == "ready"
                                and not client.call("settings/read", session=session)["settings"]["native_pending"], "backend ready before the context editor")
        terminal = self.attach_daemon(fixture, session, name)
        terminal.contains("Idle")
        return fixture, client, session, terminal

    @staticmethod
    def daemon_context_file(fixture, backend="claude"):
        return Path(fixture.env["XDG_CONFIG_HOME"]) / f"carlos/{backend}-context.json"

    @staticmethod
    def context_helper_requests(fixture, method):
        path = fixture.root / "claude-controller.jsonl"
        return [record["message"] for line in path.read_text().splitlines()
                if (record := json.loads(line))["direction"] == "client"
                and record["message"].get("method") == method]

    @staticmethod
    def open_remote_context(terminal):
        terminal.send(CTRL_M)
        terminal.contains("Tab field, arrows, Enter save")
        terminal.send(b"\t\t")
        terminal.contains("Next backend launch applies size")

    def test_daemon_context_cancel_refresh_and_explicit_save_keep_the_process_and_composer(self):
        fixture, client, session, terminal = self.backend_settings_terminal("context-ui-private")
        terminal.paste("private context composer 日本語")
        self.open_remote_context(terminal)
        terminal.paste("500k")
        terminal.send(b"\x1b")
        terminal.wait(lambda:terminal.input_is("private context composer 日本語"), "cancel keeps private composer")
        self.assertFalse(self.daemon_context_file(fixture).exists())
        self.open_remote_context(terminal)
        terminal.contains("Auto")
        self.assertNotIn("500k", terminal.text)
        terminal.paste("500k")
        path = self.daemon_context_file(fixture)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps({"tokens":750000}))
        reads = len(terminal.requests("settings/context/read"))
        terminal.send(F6)
        terminal.wait(lambda:len(terminal.requests("settings/context/read")) > reads, "explicit refresh issues a fresh context read")
        refreshed = terminal.requests("settings/context/read")[-1]["id"]
        terminal.wait(lambda:any(message.get("id") == refreshed and message.get("result", {}).get("saved_tokens") == 750000
                                   for message in terminal.messages("server")), "fresh context reply reports the independently changed host default")
        terminal.contains("Next backend launch applies size")
        terminal.contains("500k")
        self.assertEqual(terminal.requests("settings/context/set"), [])
        terminal.send(b"\r")
        terminal.wait(lambda:path.exists() and json.loads(path.read_text()) == {"tokens":500000}, "explicit save writes only the daemon host context default")
        terminal.wait(lambda:terminal.input_is("private context composer 日本語"), "saved context keeps composer")
        self.assertEqual(len(self.context_helper_requests(fixture, "start")), 1)
        self.assertEqual(self.context_helper_requests(fixture, "model.set"), [])
        self.assertEqual(self.context_helper_requests(fixture, "effort.set"), [])
        self.assertEqual(self.context_helper_requests(fixture, "prompt"), [])
        self.assertFalse((terminal.root / "config/carlos/claude-context.json").exists())
        methods = [m["method"] for m in terminal.messages("client") if m.get("method") in ("settings/context/set", "settings/set")]
        self.assertEqual(methods, ["settings/context/set", "settings/set"])
        self.assertEqual(terminal.requests("session/send"), [])

    def test_daemon_context_invalid_and_unreadable_defaults_require_an_explicit_repair(self):
        fixture, client, session, terminal = self.backend_settings_terminal("context-ui-invalid")
        path = self.daemon_context_file(fixture)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("not JSON")
        terminal.send(CTRL_M)
        terminal.contains("Tab field, arrows, Enter save")
        terminal.send(b"\t\t")
        terminal.contains("Read Carlos context settings")
        terminal.paste("99k")
        terminal.send(b"\r")
        terminal.contains("Choose Auto or 100k to 1M")
        self.assertEqual(path.read_text(), "not JSON")
        self.assertEqual(terminal.requests("settings/context/set"), [])
        self.assertEqual(terminal.requests("settings/set"), [])
        terminal.send(b"\x7f" * 3)
        terminal.paste("500k")
        terminal.send(b"\r")
        terminal.wait(lambda:path.read_text().startswith("{") and json.loads(path.read_text()) == {"tokens":500000}, "explicit valid edit repairs the malformed host file")
        self.assertEqual(len(terminal.requests("settings/context/set")), 1)
        self.assertEqual(self.context_helper_requests(fixture, "model.set"), [])

    def test_daemon_context_uses_the_daemon_shell_policy_and_allows_auto(self):
        fixture, client, session, terminal = self.backend_settings_terminal("context-ui-shell", shell="200000")
        path = self.daemon_context_file(fixture)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps({"tokens":500000}))
        terminal.send(CTRL_M)
        terminal.contains("Tab field, arrows, Enter save")
        terminal.send(b"\t\t")
        terminal.contains("Daemon shell sets size; choose Auto")
        terminal.paste("750k")
        terminal.send(b"\r")
        terminal.contains("Unset CLAUDE_CODE_AUTO_COMPACT_WINDOW")
        self.assertEqual(terminal.requests("settings/context/set"), [])
        terminal.send(b"\x7f" * 4)
        terminal.paste("Auto")
        terminal.send(b"\r")
        terminal.wait(lambda:json.loads(path.read_text()) == {"tokens":None}, "Auto clears a saved default despite the daemon shell override")
        self.assertEqual(len(terminal.requests("settings/context/set")), 1)
        self.assertEqual(terminal.requests("session/send"), [])

    def test_daemon_context_attached_codex_is_readonly_and_grok_hides_the_unsupported_field(self):
        fixture, client, session, attached = self.settings_terminal("context-ui-attached")
        self.open_daemon_settings(attached)
        attached.send(b"\t" * 3)
        attached.contains("Attached Codex server owns the size")
        attached.paste("500k")
        attached.send(b"\r")
        attached.wait(lambda:"Thinking" not in attached.text, "readonly size permits an unrelated settings save")
        self.assertEqual(attached.requests("settings/context/set"), [])
        self.assertFalse(self.daemon_context_file(fixture, "codex").exists())
        other, _, _, grok = self.backend_settings_terminal("context-ui-grok", backend="grok")
        grok.send(CTRL_M)
        grok.contains("Tab field, arrows, Enter save")
        self.assertNotIn("┃  Context ", grok.text)
        self.assertEqual(grok.requests("settings/context/set"), [])
        grok.send(b"\x1b")

    def test_daemon_settings_claude_popup_saves_exact_native_model_and_effort(self):
        fixture, client, session, terminal = self.backend_settings_terminal("settings-ui-claude-native")
        terminal.send(CTRL_M)
        terminal.contains("Tab field, arrows, Enter save")
        terminal.send(b"\x1b[C\t\x1b[C\r")
        path = self.settings_defaults(fixture)
        terminal.wait(lambda:path.exists(), "Claude popup saves the captured defaults")
        self.assertEqual(json.loads(path.read_text()), {"model":"sdk-opus", "effort":"high", "summary":None})
        self.assertEqual([request["params"] for request in self.context_helper_requests(fixture, "model.set")], [{"model":"sdk-opus"}])
        self.assertEqual([request["params"] for request in self.context_helper_requests(fixture, "effort.set")], [{"effort":"high"}])
        self.assertEqual(terminal.requests("settings/context/set"), [])
        self.assertEqual(terminal.requests("session/send"), [])

    def test_daemon_settings_grok_popup_keeps_choices_for_the_next_prompt(self):
        fixture, client, session, terminal = self.backend_settings_terminal("settings-ui-grok-future", backend="grok")
        terminal.send(CTRL_M)
        terminal.contains("Tab field, arrows, Enter save")
        terminal.send(b"\x1b[C")
        terminal.contains("grok-4.5")
        terminal.send(b"\t\x1b[C")
        terminal.contains("medium")
        terminal.send(b"\r")
        terminal.contains("next prompt")
        self.assertEqual(json.loads(self.settings_defaults(fixture).read_text()),
                         {"model":"grok-4.5", "effort":"medium", "summary":None})
        self.assertEqual(fixture.native_requests("session/set_model"), [])
        terminal.paste("future choice prompt")
        terminal.send(b"\r")
        terminal.wait(lambda:fixture.native_requests("session/set_model"), "next prompt applies the captured Grok model")
        terminal.contains("future choice prompt")
        self.assertEqual(fixture.native_requests("session/set_model")[-1]["params"]["modelId"], "grok-4.5")
        self.assertEqual(len(terminal.requests("session/send")), 1)
