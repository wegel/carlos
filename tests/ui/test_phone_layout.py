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

    def open_theme_settings(self, terminal, profile):
        terminal.send(b"\x1b[109;5u")
        terminal.contains("Settings")
        terminal.contains("Desktop")
        terminal.contains("Phone")
        terminal.send(b"\x1b[Z" * (3 if profile == "desktop" else 2))
        terminal.contains("Theme for wide screens" if profile == "desktop" else "Theme for small screens")

    def save_themes(self, terminal, desktop, phone):
        terminal.send(b"\r")
        settings = terminal.root / "config/carlos/ui-settings.json"
        terminal.wait(lambda: settings.exists() and json.loads(settings.read_text()).get("themes")
                      == {"desktop": desktop, "phone": phone} and "Settings" not in terminal.text,
                      "themes saved and applied")

    def assert_inline_tool_block(self, terminal, labels):
        def ready():
            row = next((y for y, line in enumerate(terminal.screen.display) if labels[0] in line), None)
            return row is not None and row + len(labels) < terminal.screen.lines and all(
                label in terminal.screen.display[row + offset] for offset, label in enumerate(labels))
        terminal.wait(ready, "complete tool block frame")
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

    def test_per_screen_themes_cancel_save_restart_resize_and_keep_tool_mode(self):
        terminal = self.launch(mode="tool-history")
        self.tools_command(terminal, "/tools full", "full")
        settings = terminal.root / "config/carlos/ui-settings.json"
        saved = settings.read_bytes()
        self.open_theme_settings(terminal, "desktop")
        terminal.send(b"\x1b[C")  # Gutter -> Flow, still a draft.
        terminal.send(b"\x1b")
        terminal.wait(lambda: "Settings" not in terminal.text, "cancel theme draft")
        self.assertNotIn("→ source.rs", terminal.text)
        self.assertEqual(settings.read_bytes(), saved)
        self.open_theme_settings(terminal, "desktop")
        terminal.send(b"\x1b[C")
        self.save_themes(terminal, "flow", "flow")
        for width, height in [(100, 32), (54, 32), (40, 18)]:
            terminal.resize(width, height)
            if height >= 22:
                terminal.contains("→ source.rs")
            terminal.contains("let read_7 = 7;")
            terminal.wait(lambda: terminal.input_is(""), "Flow composer")
            self.assertEqual(terminal.screen.buffer[0][0].data, " ")
        self.open_theme_settings(terminal, "phone")  # Both rows fit on a short phone.
        terminal.resize(40, 12)
        terminal.contains("Desktop")
        terminal.contains("Phone")
        terminal.contains("Theme for small screens")
        terminal.snapshot("themes-minimum-height")
        terminal.send(b"\x1b")
        terminal.close()
        resumed = self.launch(mode="tool-history", directory=terminal.root)
        resumed.contains("→ source.rs")
        self.open_theme_settings(resumed, "phone")
        resumed.send(b"\x1b[C")  # Only the small-screen choice changes.
        self.save_themes(resumed, "flow", "gutter")
        resumed.contains("→ source.rs")
        for width, height in [(54, 32), (100, 18)]:
            resumed.resize(width, height)
            resumed.wait(lambda: "→ source.rs" not in resumed.text and "let read_7" in resumed.text,
                         "small screen uses Gutter")
            row = self.theme_row(resumed, "let read_7")
            self.assertEqual(resumed.screen.buffer[row][0].data, "┃")
            self.assertEqual(resumed.screen.display[row].index("let"), 2)
        resumed.resize(100, 32)
        resumed.contains("→ source.rs")
        self.open_theme_settings(resumed, "desktop")
        resumed.send(b"\x1b[C")
        self.save_themes(resumed, "gutter", "gutter")
        resumed.wait(lambda: "→ source.rs" not in resumed.text, "wide screen uses Gutter")
        self.assertEqual(json.loads(settings.read_text())["tool_output"], "full")
        self.assertEqual(resumed.requests("turn/start"), [])
        self.assertEqual(resumed.requests("turn/steer"), [])
        self.assertTrue(resumed.requests("thread/resume"))
        resumed.snapshot("independent-screen-themes")

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
        self.assert_status_sweep_moves(terminal, "cba6f7")
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
