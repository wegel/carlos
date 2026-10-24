"""Exercise phone-sized terminals, reversible folds, and local tool commands."""

import json


class ToolOutput:
    def open_tool_settings(self, terminal, expected):
        terminal.send(b"\x1b[109;5u")  # Ctrl+M, encoded distinctly from Enter.
        terminal.contains("Settings")
        terminal.contains("Tool output")
        terminal.contains(expected)
        terminal.send(b"\x1b[Z")  # Backtab from Model selects Tool output.

    def test_ctrl_m_edits_tool_output_and_leaves_activity_line_clear(self):
        terminal = self.launch(mode="tool-history", width=54)
        self.assert_compact_tools(terminal)
        self.assertNotIn("tools:", terminal.screen.display[-2])
        self.open_tool_settings(terminal, "Auto (Compact)")
        terminal.send(b"\x1b[C\x1b[C")  # Auto -> Compact -> Full, still a draft.
        terminal.contains("Full")
        self.assertFalse((terminal.root / "config/carlos/ui-settings.json").exists())
        terminal.send(b"\r")
        self.assert_full_tools(terminal)
        self.assertNotIn("Settings", terminal.text)
        self.assertNotIn("tools:", terminal.screen.display[-2])
        self.assertEqual(json.loads((terminal.root / "config/carlos/ui-settings.json").read_text())["tool_output"], "full")
        self.open_tool_settings(terminal, "Full")
        terminal.send(b"\x1b[D")  # Draft Compact.
        terminal.contains("Compact")
        terminal.send(b"\x1b")  # Cancel the draft.
        terminal.wait(lambda: "Settings" not in terminal.text, "settings dismissed")
        self.assert_full_tools(terminal)
        self.assertEqual(terminal.requests("turn/start"), [])
        self.assertEqual(terminal.requests("turn/steer"), [])
        terminal.snapshot("tool-output-settings")

    def test_settings_fit_a_short_phone_screen(self):
        terminal = self.launch(mode="tool-history", width=40)
        terminal.resize(40, 18)
        self.assert_compact_tools(terminal)
        self.open_tool_settings(terminal, "Auto (Compact)")
        terminal.send(b"\x1b[D")  # Auto -> Full.
        terminal.contains("Full")
        terminal.send(b"\r")
        terminal.contains("+let after_3 = 3;")
        self.assertNotIn("rows hidden", terminal.text)
        self.assertNotIn("tools:", terminal.screen.display[-2])
        self.assertEqual(terminal.requests("turn/start"), [])

    def test_compact_hides_short_commands_and_multiline_scripts(self):
        terminal = self.launch(mode="tool-short", resume=None, width=54)
        self.start_tools(terminal)
        self.assertNotIn("from pathlib import Path", terminal.text)
        self.assertNotIn("short script marker", terminal.text)
        self.assertNotIn("$ git add source.rs", terminal.text)
        self.assertNotIn("exit code: 0", terminal.text)
        terminal.contains("git add source.rs")
        self.tools_command(terminal, "/tools full", "full")
        terminal.contains("from pathlib import Path")
        terminal.contains("$ git add source.rs")
        terminal.contains("short script marker")
        self.assertEqual(len(terminal.requests("turn/start")), 1)

    def test_claude_reads_remain_available_when_switching_modes(self):
        terminal = self.launch(backend="claude", resume=None, width=54)
        terminal.wait(lambda: any(message.get("type") == "system" for message in terminal.messages("server")), "Claude ready")
        terminal.paste("read source")
        terminal.send(b"\r")
        terminal.contains("Answer: read source")
        terminal.contains("rows hidden")
        self.assertNotIn("let claude_read_7", terminal.text)
        self.tools_command(terminal, "/tools full", "full")
        terminal.contains("let claude_read_7 = 7;")
        requests = [message for message in terminal.messages() if message.get("type") == "user"]
        self.assertEqual(len(requests), 1)

    def tools_command(self, terminal, command, mode):
        terminal.paste(command)
        terminal.wait(lambda: terminal.input_is(command), "local command draft")
        terminal.send(b"\r")
        settings = terminal.root / "config/carlos/ui-settings.json"
        terminal.wait(lambda: terminal.input_is("") and settings.exists()
                      and json.loads(settings.read_text())["tool_output"] == mode,
                      "local tools command saved")

    def start_tools(self, terminal):
        terminal.wait(lambda: terminal.input_is(""), "thread ready")
        terminal.paste("inspect source")
        terminal.send(b"\r")
        terminal.contains("Answer: inspect source")

    def assert_full_tools(self, terminal):
        terminal.contains("let read_7 = 7;")
        terminal.contains("+let after_3 = 3;")
        self.assertNotIn("rows hidden", terminal.text)

    def assert_compact_tools(self, terminal):
        terminal.contains("→ Read source.rs")
        terminal.contains("Δ Diff source.rs (+4, -4)")
        terminal.contains("rows hidden")
        self.assertNotIn("let read_7", terminal.text)
        self.assertNotIn("let after_3", terminal.text)

    def test_auto_folds_on_resize_and_manual_full_survives_phone_size(self):
        terminal = self.launch(mode="tool-output", resume=None)
        self.start_tools(terminal)
        self.assert_full_tools(terminal)
        terminal.resize(54, 32)
        self.assert_compact_tools(terminal)
        self.assertFalse((terminal.root / "config/carlos/ui-settings.json").exists())
        terminal.snapshot("auto-phone")
        self.tools_command(terminal, "/tools full", "full")
        self.assert_full_tools(terminal)
        terminal.resize(60, 32)
        self.assert_full_tools(terminal)
        self.tools_command(terminal, "/tools auto", "auto")
        self.assert_compact_tools(terminal)
        terminal.resize(100, 32)
        self.assert_full_tools(terminal)
        self.assertEqual(len(terminal.requests("turn/start")), 1)
        self.assertEqual(terminal.requests("turn/steer"), [])

    def test_small_startup_can_unfold_saved_reads_and_diffs(self):
        terminal = self.launch(mode="tool-history", width=54)
        self.assert_compact_tools(terminal)
        self.tools_command(terminal, "/tools full", "full")
        self.assert_full_tools(terminal)
        self.assertTrue(terminal.requests("thread/resume"))
        self.assertTrue(terminal.requests("thread/turns/list"))
        self.assertEqual(terminal.requests("turn/start"), [])
        terminal.snapshot("saved-tools-unfolded")

    def test_short_terminal_compacts_and_compact_choice_survives_restart(self):
        terminal = self.launch(mode="tool-output", resume=None)
        self.start_tools(terminal)
        terminal.resize(100, 18)
        self.assert_compact_tools(terminal)
        self.tools_command(terminal, "/tools compact", "compact")
        terminal.resize(100, 32)
        self.assert_compact_tools(terminal)
        settings = terminal.root / "config/carlos/ui-settings.json"
        self.assertEqual(json.loads(settings.read_text()), {"tool_output": "compact"})
        terminal.close()
        resumed = self.launch(mode="tool-output", resume="thread-new", directory=terminal.root)
        self.assert_compact_tools(resumed)
        self.tools_command(resumed, "/tools full", "full")
        self.assert_full_tools(resumed)
        self.assertEqual(len(resumed.requests("turn/start")), 1)

    def test_tool_commands_during_a_live_turn_do_not_steer_the_model(self):
        terminal = self.launch(mode="tool-live", resume=None)
        self.start_tools(terminal)
        self.tools_command(terminal, "/tools compact", "compact")
        self.assert_compact_tools(terminal)
        self.tools_command(terminal, "/tools full", "full")
        self.assert_full_tools(terminal)
        self.assertEqual(len(terminal.requests("turn/start")), 1)
        self.assertEqual(terminal.requests("turn/steer"), [])
        self.assertEqual(terminal.requests("turn/interrupt"), [])

    def test_compact_marks_failures_and_hides_their_details(self):
        terminal = self.launch(mode="tool-failure", resume=None, width=54)
        self.start_tools(terminal)
        terminal.contains("Failed:")
        terminal.contains("rows hidden")
        self.assertNotIn("failure detail 0", terminal.text)
        self.assertNotIn("failure detail 7", terminal.text)
        self.tools_command(terminal, "/tools full", "full")
        terminal.contains("failure detail 0")
        terminal.contains("failure detail 7")
        terminal.contains("exit code: 2")
        self.assertEqual(len(terminal.requests("turn/start")), 1)

    def test_bare_tools_cycles_and_unknown_arguments_stay_local(self):
        terminal = self.launch(resume=None)
        terminal.wait(lambda: terminal.input_is(""), "thread ready")
        for label in ["compact", "full", "auto"]:
            self.tools_command(terminal, "/tools", label)
        terminal.paste("/tools invalid")
        terminal.send(b"\r")
        terminal.contains("Use /tools auto, /tools compact, or /tools full")
        self.assertTrue(terminal.input_is("/tools invalid"))
        self.assertEqual(terminal.requests("turn/start"), [])
