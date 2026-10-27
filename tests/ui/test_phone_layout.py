"""Check phone section headers, local layout choices, and saved overrides."""

import json


class PhoneLayout:
    def test_desktop_spacing_groups_activity_and_separates_other_messages(self):
        terminal = self.launch(mode="desktop-spacing", width=54, layout="desktop")
        labels = ["palette.rs", "second.rs", "src [pattern=palette]", "Checking results",
                  "Checking matches", "palette.rs +1 −1", "theme-check", "printf"]
        self.assert_inline_tool_block(terminal, labels)
        for label in ["Keep this second user", "Checking the palette", "First detailed thought",
                      "I will check the tool colors.", "Here is a second update.", "The colors are ready."]:
            row = self.theme_row(terminal, label)
            self.assertEqual(terminal.screen.display[row - 1].strip(), "")
        self.assertEqual(self.source_cell(terminal, "Checking results", "Checking").fg, "ac9fd1")
        self.assertTrue(self.source_cell(terminal, "Checking results", "Checking").italics)
        self.assertNotIn("─", terminal.text)
        self.assertEqual(terminal.requests("turn/start"), [])
        self.assertEqual(terminal.requests("turn/steer"), [])
        self.assertTrue(terminal.requests("thread/resume"))
        terminal.snapshot("desktop-shared-spacing")

    def open_layout_settings(self, terminal):
        terminal.send(b"\x1b[109;5u")
        terminal.contains("Settings")
        terminal.contains("Layout")
        terminal.send(b"\x1b[Z\x1b[Z")  # Model -> Tool output -> Layout.

    def save_layout(self, terminal, mode):
        terminal.send(b"\r")
        settings = terminal.root / "config/carlos/ui-settings.json"
        terminal.wait(lambda: settings.exists() and json.loads(settings.read_text()).get("layout") == mode
                      and "Settings" not in terminal.text, "layout saved and applied")

    def assert_inline_tool_block(self, terminal, labels):
        row = self.theme_row(terminal, labels[0])
        self.assertEqual(terminal.screen.display[row - 1].strip(), "")
        for offset, label in enumerate(labels):
            self.assertIn(label, terminal.screen.display[row + offset])
        self.assertEqual(terminal.screen.display[row + len(labels)].strip(), "")

    def test_phone_sections_keep_glyphs_colors_padding_and_grouped_reads(self):
        terminal = self.launch(mode="phone-history", width=54)
        for label, color in [("❯ you", "89b4fa"), ("◇ reasoning", "cba6f7"),
                             ("• commentary", "f5c2e7"), ("→ palette.rs", "74c7ec"),
                             ("✱ src [pattern=palette]", "94e2d5"), ("Δ palette.rs", "f9e2af"),
                             ("✓ answer", "a6e3a1")]:
            terminal.contains(label)
            self.assertEqual(self.source_cell(terminal, label, label[0]).fg, color)
            if label.startswith(("◇", "•", "✓")):
                row = self.theme_row(terminal, label)
                self.assertEqual(terminal.screen.display[row - 1].strip(), "")
        terminal.contains("second.rs")
        self.assertNotIn("→ read", terminal.text)
        self.assertNotIn("Δ diff/edit", terminal.text)
        self.assertNotIn("$ run", terminal.text)
        self.assertNotIn("✱ search", terminal.text)
        self.assertEqual(terminal.text.count("❯ you"), 1)
        row = self.theme_row(terminal, "Please inspect the theme")
        self.assertEqual(terminal.screen.display[row + 1].strip(), "")
        self.assertIn("Keep this second user", terminal.screen.display[row + 2])
        self.assert_inline_tool_block(terminal, ["→ palette.rs", "→ second.rs",
            "✱ src [pattern=palette]", "◇ Checking results", "Δ palette.rs", "$ theme-check", "$ printf"])
        self.assertEqual(self.source_cell(terminal, "◇ Checking results", "◇").fg, "cba6f7")
        self.assertTrue(self.source_cell(terminal, "◇ Checking results", "Checking").italics)
        self.assertNotIn("┃", terminal.text)
        for row in range(terminal.screen.lines - 2):
            self.assertEqual(terminal.screen.buffer[row][0].data, " ")
            self.assertEqual(terminal.screen.buffer[row][53].data, " ")
        body = self.theme_row(terminal, "Please inspect the theme")
        self.assertEqual(terminal.screen.display[body].index("Please"), 1)
        self.assertEqual(self.source_cell(terminal, "$ theme-check", "$").fg, "f38ba8")
        self.assert_folded_row(terminal, "palette.rs")
        terminal.snapshot("phone-sections")
        self.tools_command(terminal, "/tools full", "full")
        terminal.resize(54, 60)
        terminal.contains("let tone_7 = 7;")
        terminal.contains("let second = 2;")
        terminal.contains("check failed detail")
        self.assertEqual(self.source_cell(terminal, "let tone_7", "let").fg, "cba6f7")
        self.assertEqual(terminal.requests("turn/start"), [])
        self.assertTrue(terminal.requests("thread/resume"))
        terminal.snapshot("phone-full-output")

    def test_layout_settings_cancel_save_restart_and_stay_independent_of_tool_mode(self):
        terminal = self.launch(mode="tool-history")
        self.tools_command(terminal, "/tools full", "full")
        settings = terminal.root / "config/carlos/ui-settings.json"
        saved = settings.read_bytes()
        self.open_layout_settings(terminal)
        terminal.contains("Auto (Desktop)")
        terminal.send(b"\x1b[D")  # Auto -> Phone, still a draft.
        terminal.contains("Phone")
        terminal.send(b"\x1b")
        terminal.wait(lambda: "Settings" not in terminal.text, "cancel layout draft")
        self.assertNotIn("→ source.rs", terminal.text)
        self.assertEqual(settings.read_bytes(), saved)
        self.open_layout_settings(terminal)
        terminal.send(b"\x1b[D")
        self.save_layout(terminal, "phone")
        terminal.contains("→ source.rs")
        terminal.contains("let read_7 = 7;")
        terminal.resize(40, 18)
        terminal.wait(lambda: terminal.screen.cursor.y == 17 and terminal.input_is(""),
                      "resized phone frame reaches the composer")
        self.open_layout_settings(terminal)
        terminal.contains("Phone")
        terminal.send(b"\x1b")
        terminal.wait(lambda: "Settings" not in terminal.text, "close phone settings")
        self.assertEqual(json.loads(settings.read_text())["tool_output"], "full")
        terminal.close()
        resumed = self.launch(mode="tool-history", directory=terminal.root)
        resumed.contains("→ source.rs")
        resumed.contains("let read_7 = 7;")
        self.open_layout_settings(resumed)
        resumed.send(b"\x1b[D")  # Phone -> Desktop.
        self.save_layout(resumed, "desktop")
        resumed.resize(54, 32)
        resumed.wait(lambda: "→ source.rs" not in resumed.text and "let read_7" in resumed.text,
                     "desktop override keeps the gutter on a phone")
        row = self.theme_row(resumed, "let read_7")
        self.assertEqual(resumed.screen.buffer[row][0].data, "┃")
        self.assertEqual(resumed.screen.display[row].index("let"), 2)
        self.open_layout_settings(resumed)
        resumed.send(b"\x1b[D")  # Desktop -> Auto.
        self.save_layout(resumed, "auto")
        resumed.contains("→ source.rs")
        resumed.resize(100, 32)
        resumed.wait(lambda: "→ source.rs" not in resumed.text, "Auto returns to desktop")
        self.assertEqual(resumed.requests("turn/start"), [])
        self.assertEqual(resumed.requests("turn/steer"), [])

    def test_phone_reasoning_symbol_and_heading_follow_activity_and_width(self):
        terminal = self.launch(mode="theme-active", resume=None, width=54)
        terminal.wait(lambda: terminal.input_is(""), "thread ready")
        terminal.paste("inspect config")
        terminal.send(b"\r")
        terminal.contains("◆ Analyzing key config files")
        self.assertNotIn("reasoning", terminal.text)
        row = self.theme_row(terminal, "◆ Analyzing")
        self.assertRegex(terminal.screen.display[row], "[⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏]")
        self.assertTrue(self.source_cell(terminal, "◆ Analyzing", "Analyzing").bold)
        self.assert_status_spinner_moves(terminal, "89dceb")
        terminal.resize(18, 32)
        terminal.contains("◆ reasoning")
        terminal.resize(54, 32)
        terminal.contains("◆ Analyzing key config files")
        self.assertNotIn("┃", terminal.text)
        terminal.send(b"\x1b")
        terminal.contains("Turn interrupted")
        terminal.contains("◇ Analyzing key config files")
        self.assertNotIn("◆ reasoning", terminal.text)
        self.assertNotIn("◆ Analyzing", terminal.text)
        self.assertTrue(self.source_cell(terminal, "◇ Analyzing", "Analyzing").italics)
        self.assertEqual(len(terminal.requests("turn/start")), 1)
        self.assertEqual(len(terminal.requests("turn/interrupt")), 1)
