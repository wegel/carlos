"""Exercise context choices, startup flags, cancellation, and short-screen focus."""

import json

SETTINGS = b"\x1b[109;5u"


class ContextSettings:
    def open_context(self, terminal, backend, expected="Restart Carlos"):
        terminal.send(SETTINGS)
        terminal.contains("Settings")
        terminal.send(b"\t" * (2 if backend == "claude" else 3))
        terminal.contains(expected)

    def context_file(self, terminal, backend):
        return terminal.root / f"config/carlos/{backend}-context.json"

    def wait_saved_context(self, terminal, backend, tokens):
        path = self.context_file(terminal, backend)
        terminal.wait(lambda: path.is_file() and json.loads(path.read_text())["tokens"] == tokens,
                      "context choice saved for the next launch")

    def test_context_claude_cancel_custom_save_restart_and_auto_reset(self):
        terminal = self.launch(backend="claude", resume=None)
        self.claude_ready(terminal)
        terminal.paste("composer stays here")
        self.open_context(terminal, "claude")
        terminal.paste("500k")
        terminal.send(b"\x1b")
        terminal.wait(lambda: terminal.input_is("composer stays here"), "cancel preserves composer")
        self.assertFalse(self.context_file(terminal, "claude").exists())
        self.open_context(terminal, "claude")
        terminal.paste("500k")
        terminal.send(b"\r")
        self.wait_saved_context(terminal, "claude", 500000)
        terminal.wait(lambda: terminal.input_is("composer stays here"), "save preserves composer")
        self.assertEqual(self.helper_requests(terminal, "prompt"), [])
        self.assertEqual(len([frame for frame in terminal.messages() if frame.get("event") == "launch"]), 1)
        self.assertFalse(self.context_file(terminal, "codex").exists())
        terminal.snapshot("claude-context-saved-for-restart")
        terminal.close()

        restarted = self.launch(backend="claude", resume=None, directory=terminal.root)
        self.claude_ready(restarted)
        launch = [frame for frame in restarted.messages() if frame.get("event") == "launch"][-1]
        self.assertEqual(json.loads(launch["argv"][launch["argv"].index("--settings") + 1]), {"autoCompactWindow": 500000})
        self.open_context(restarted, "claude")
        restarted.contains("500k")
        restarted.send(b"\x1b[C\x1b[C\x1b[C\r")  # 500k -> 750k -> 1M -> Auto.
        self.wait_saved_context(restarted, "claude", None)
        restarted.close()
        inherited = self.launch(backend="claude", resume=None, directory=terminal.root)
        self.claude_ready(inherited)
        self.assertNotIn("contextLimit", self.helper_requests(inherited, "start")[-1]["params"])

    def test_context_invalid_and_failed_save_keep_the_editor_open_without_sending_a_prompt(self):
        terminal = self.launch(backend="claude", resume=None)
        self.claude_ready(terminal)
        self.open_context(terminal, "claude")
        terminal.paste("99k")
        terminal.send(b"\r")
        terminal.contains("Choose Auto or 100k to 1M")
        self.assertFalse(self.context_file(terminal, "claude").exists())
        terminal.send(b"\x1b")
        terminal.wait(lambda: "Settings" not in terminal.text, "cancel invalid size")
        self.context_file(terminal, "claude").mkdir(parents=True)  # A directory cannot receive the saved file.
        self.open_context(terminal, "claude")
        terminal.paste("500k")
        terminal.send(b"\r")
        terminal.contains("Save Carlos context settings")
        terminal.contains("Settings")
        self.assertTrue(self.context_file(terminal, "claude").is_dir())
        self.assertEqual(self.helper_requests(terminal, "prompt"), [])
        terminal.snapshot("context-save-failed")

    def test_context_claude_shell_override_is_reported_without_saving_an_ignored_size(self):
        terminal = self.launch(backend="claude", resume=None,
                               extra_env={"CLAUDE_CODE_AUTO_COMPACT_WINDOW": "200000"})
        self.claude_ready(terminal)
        self.open_context(terminal, "claude", expected="Shell sets size")
        terminal.paste("500k")
        terminal.send(b"\r")
        terminal.contains("Unset CLAUDE_CODE_AUTO_COMPACT_WINDOW")
        self.assertFalse(self.context_file(terminal, "claude").exists())
        self.assertEqual(self.helper_requests(terminal, "prompt"), [])

    def test_context_codex_saves_for_restart_without_changing_native_global_settings(self):
        terminal = self.launch(width=120)
        terminal.contains("Answer: second")
        self.open_context(terminal, "codex")
        terminal.paste("500k")
        terminal.send(b"\r")
        self.wait_saved_context(terminal, "codex", 500000)
        terminal.contains("restart Carlos to apply it")
        self.assertEqual(terminal.requests("config/value/write"), [])
        self.assertEqual(terminal.requests("config/batchWrite"), [])
        self.assertEqual(terminal.requests("turn/start"), [])
        self.assertFalse(self.context_file(terminal, "claude").exists())
        terminal.close()

        restarted = self.launch(width=120, directory=terminal.root)
        restarted.contains("Answer: second")
        launches = [json.loads(line) for line in (restarted.root / "codex-launches.jsonl").read_text().splitlines()]
        self.assertNotIn("model_context_window=500000", launches[0])
        self.assertIn("model_context_window=500000", launches[-1])
        self.assertIn("model_auto_compact_token_limit=450000", launches[-1])
        self.open_context(restarted, "codex")
        restarted.contains("500k")
        restarted.snapshot("codex-context-after-restart")
        restarted.send(b"\x1b[C\x1b[C\x1b[C\r")
        self.wait_saved_context(restarted, "codex", None)
        restarted.close()
        inherited = self.launch(width=120, directory=terminal.root)
        inherited.contains("Answer: second")
        launch = json.loads((inherited.root / "codex-launches.jsonl").read_text().splitlines()[-1])
        self.assertFalse(any(arg.startswith("model_context_window=") for arg in launch))

    def test_context_presets_and_other_settings_remain_reachable_on_a_short_phone(self):
        terminal = self.launch(width=40)
        terminal.contains("Answer: second")
        self.open_context(terminal, "codex")
        terminal.resize(40, 12)
        terminal.contains("Context")
        terminal.send(b"\x1b[C")
        terminal.contains("100k")
        terminal.snapshot("phone-context")
        terminal.send(b"\t" * 6)
        terminal.contains("Model")
        terminal.send(b"\x1b[Z")
        terminal.contains("Tool output")
        self.assertLess(terminal.screen.cursor.y, terminal.screen.lines - 1)
        terminal.snapshot("phone-last-settings-row")
        terminal.send(b"\x1b")
        self.assertFalse(self.context_file(terminal, "codex").exists())
