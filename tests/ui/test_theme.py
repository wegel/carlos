"""Check 3c text, gutter glyphs, compact columns, and context warnings."""


class Theme:
    def scanner_head(self, terminal, color):
        row = terminal.screen.lines - 2
        return next((x for x in range(terminal.screen.columns)
                     if terminal.screen.buffer[row][x].data == "━"
                     and terminal.screen.buffer[row][x].fg == color), None)

    def assert_scanner_moves(self, terminal, color):
        terminal.wait(lambda: self.scanner_head(terminal, color) is not None,
                      "K2000 scanner head is visible")
        first = self.scanner_head(terminal, color)
        terminal.wait(lambda: (head := self.scanner_head(terminal, color)) is not None
                      and head != first, "K2000 scanner moves without new backend output")

    def status_sweep_head(self, terminal, color):
        cells = terminal.screen.buffer[terminal.screen.lines - 2]
        return next((x for x in range(terminal.screen.columns - 1)
                     if cells[x].fg == color), None)

    def assert_status_sweep_moves(self, terminal, color):
        terminal.wait(lambda: self.status_sweep_head(terminal, color) is not None,
                      "status-line scanner head is visible")
        row = terminal.screen.display[-2]
        first = self.status_sweep_head(terminal, color)
        self.assertTrue(terminal.screen.buffer[terminal.screen.lines - 2][first].bold)
        terminal.wait(lambda: (head := self.status_sweep_head(terminal, color)) is not None
                      and head != first, "status-line scanner moves without backend output")
        self.assertEqual(terminal.screen.display[-2], row)
        self.assertFalse(any(c.data in "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"
                             for c in terminal.screen.buffer[terminal.screen.lines - 2].values()))

    def test_gutter_cursor_and_mouse_selection_follow_terminal_size(self):
        terminal = self.launch(resume=None)
        terminal.wait(lambda: terminal.input_is(""), "thread ready")
        terminal.paste("ABCDEFGHIJK")
        terminal.send(b"\r")
        terminal.contains("Answer: ABCDEFGHIJK")
        for width, height, content_x in [(100, 32, 2), (54, 32, 1), (100, 18, 1), (100, 32, 2)]:
            terminal.resize(width, height)
            terminal.wait(lambda: any(line.index("ABCDEFGHIJK") == content_x
                          for line in terminal.screen.display if "ABCDEFGHIJK" in line),
                          "transcript uses the expected gutter")
            terminal.paste("draft")
            terminal.wait(lambda: terminal.input_is("draft"), "composer uses the same gutter")
            self.assertEqual(terminal.screen.cursor.x, content_x + 5)
            terminal.send(b"\x01\x0b")
            row = next(y for y, line in enumerate(terminal.screen.display) if "ABCDEFGHIJK" in line)
            terminal.send(f"\x1b[<0;{content_x + 3};{row + 1}M".encode())
            terminal.send(f"\x1b[<32;{content_x + 5};{row + 1}M".encode())
            terminal.wait(lambda: terminal.screen.buffer[row][content_x + 2].bg == "6c7086",
                          "mouse drag highlights the intended character")
            self.assertEqual(terminal.screen.buffer[row][content_x + 2].data, "C")
            self.assertNotEqual(terminal.screen.buffer[row][content_x + 1].bg, "6c7086")
            terminal.snapshot(f"gutter-selection-{width}-{height}")
        self.assertEqual(len(terminal.requests("turn/start")), 1)
        self.assertEqual(terminal.requests("turn/steer"), [])

    def test_k2000_moves_during_quiet_turns_and_returns_to_the_idle_separator(self):
        terminal = self.launch(mode="tool-live-context", resume=None)
        self.start_tools(terminal)
        terminal.contains("91%")
        self.assert_scanner_moves(terminal, "cba6f7")
        terminal.snapshot("k2000-working-wide")
        self.tools_command(terminal, "/tools full", "full")
        for width, height in [(54, 32), (18, 32), (100, 18)]:
            terminal.resize(width, height)
            self.assert_status_sweep_moves(terminal, "cba6f7")
            terminal.snapshot(f"status-sweep-{width}-{height}")
        terminal.resize(100, 32)
        self.assert_scanner_moves(terminal, "cba6f7")
        terminal.contains("91%")
        self.assertEqual(self.source_cell(terminal, "91%", "91%").fg, "f38ba8")
        self.assertEqual(self.source_cell(terminal, "91%", "91k/100k").fg, "9399b2")
        terminal.send(b"\x1b")
        terminal.wait(lambda: terminal.requests("turn/interrupt"), "interrupt quiet turn")
        terminal.contains("Turn interrupted")
        self.assertIsNone(self.scanner_head(terminal, "cba6f7"))
        self.assert_status_separator(terminal)
        self.assertEqual(len(terminal.requests("turn/start")), 1)
        self.assertEqual(len(terminal.requests("turn/interrupt")), 1)
        self.assertEqual(terminal.requests("turn/steer"), [])
        terminal.snapshot("k2000-idle-separator")

    def test_k2000_keeps_its_pink_ralph_trail(self):
        terminal = self.launch(mode="tool-live-context", resume=None)
        terminal.wait(lambda: terminal.requests("model/list"), "thread ready")
        prompt = terminal.root / ".agents/ralph-prompt.md"
        prompt.parent.mkdir()
        prompt.write_text("inspect source")
        terminal.send(b"\x12")  # Ctrl+R starts Ralph with the fixed local peer.
        terminal.contains("Answer: inspect source")
        terminal.contains("RALPH MODE")
        self.assert_scanner_moves(terminal, "f5c2e7")
        cells = terminal.screen.buffer[terminal.screen.lines - 2]
        self.assertTrue(any(c.data == "━" and c.fg in ("f2cdcd", "cba6f7")
                            for c in cells.values()))
        self.assertEqual(len(terminal.requests("turn/start")), 1)
        self.assertEqual(terminal.requests("turn/steer"), [])
        terminal.snapshot("k2000-ralph")
        terminal.resize(54, 32)
        self.assert_status_sweep_moves(terminal, "f5c2e7")
        terminal.snapshot("status-sweep-ralph-phone")

    def test_phone_kitt_crosses_model_and_token_text_then_restores_idle_colors(self):
        terminal = self.launch(mode="tool-live-context", resume=None)
        self.start_tools(terminal)
        terminal.resize(54, 32)
        terminal.contains("91%")
        self.assert_status_sweep_moves(terminal, "cba6f7")
        row = terminal.screen.lines - 2
        label = terminal.screen.display[row].index("gpt-")
        terminal.wait(lambda: any(terminal.screen.buffer[row][x].fg == "cba6f7"
                      and terminal.screen.buffer[row][x].data.isalpha()
                      for x in range(label, terminal.screen.columns - 1)),
                      "KITT head sweeps over status text")
        head = self.status_sweep_head(terminal, "cba6f7")
        self.assertTrue(terminal.screen.buffer[row][head].bold)
        terminal.contains("91k/100k 91%")
        terminal.snapshot("phone-kitt-status-text")
        terminal.send(b"\x1b")
        terminal.wait(lambda: terminal.requests("turn/interrupt"), "interrupt quiet turn")
        terminal.contains("Turn interrupted")
        self.assertIsNone(self.status_sweep_head(terminal, "cba6f7"))
        self.assertEqual(self.source_cell(terminal, "91%", "91%").fg, "f38ba8")
        self.assertEqual(self.source_cell(terminal, "91%", "91k/100k").fg, "9399b2")
        self.assertFalse(self.source_cell(terminal, "91%", "91k/100k").bold)
        self.assert_status_separator(terminal)
        self.assertEqual(len(terminal.requests("turn/start")), 1)
        self.assertEqual(len(terminal.requests("turn/interrupt")), 1)
        self.assertEqual(terminal.requests("turn/steer"), [])

    def theme_row(self, terminal, label):
        terminal.contains(label)
        return next(y for y, line in enumerate(terminal.screen.display) if label in line)

    def assert_theme_row(self, terminal, label, icon, hue, foreground):
        row = self.theme_row(terminal, label)
        self.assertEqual(terminal.screen.display[row].index(label), terminal.content_x)
        self.assertEqual(terminal.screen.buffer[row][0].data, icon)
        self.assertEqual(terminal.screen.buffer[row][0].fg, hue)
        self.assertEqual(self.source_cell(terminal, label, label).fg, foreground)
        return row

    def test_3c_groups_blocks_and_keeps_phone_tool_counts_on_one_row(self):
        terminal = self.launch(mode="theme-70", width=54, layout="desktop")
        user = self.assert_theme_row(terminal, "Please inspect the theme", "❯", "89b4fa", "cdd6f4")
        second = self.theme_row(terminal, "Keep this second user")
        self.assertEqual(second, user + 2)
        self.assertEqual(terminal.screen.display[user + 1].strip(), "")
        self.assertTrue(terminal.screen.buffer[user][3].bold)
        self.assertTrue(all(terminal.screen.buffer[user][x].bg == "202235" for x in range(54)))
        reasoning = self.assert_theme_row(terminal, "Checking the palette", "◇", "cba6f7", "ac9fd1")
        self.assertTrue(terminal.screen.buffer[reasoning][3].italics)
        self.assertFalse(terminal.screen.buffer[reasoning][3].bold)
        continuation = self.theme_row(terminal, "Checking the spacing")
        self.assertEqual(terminal.screen.buffer[continuation][0].data, "┃")
        self.assert_theme_row(terminal, "I will check", "•", "f5c2e7", "dbcfef")
        read = self.assert_theme_row(terminal, "palette.rs", "→", "74c7ec", "9399b2")
        search = self.assert_theme_row(terminal, "src [pattern=palette]", "✱", "94e2d5", "9399b2")
        diff = self.assert_theme_row(terminal, "palette.rs +1 −1", "Δ", "f9e2af", "9399b2")
        self.assertEqual(search, read + 1)
        self.assertEqual(diff, search + 1)
        self.assertEqual(self.source_cell(terminal, "palette.rs +1 −1", "+1").fg, "a6e3a1")
        self.assertEqual(self.source_cell(terminal, "palette.rs +1 −1", "−1").fg, "f38ba8")
        self.assert_theme_row(terminal, "theme-check", "$", "f38ba8", "9399b2")
        failed = self.source_cell(terminal, "theme-check", "failed")
        self.assertEqual(failed.fg, "f38ba8")
        self.assertTrue(failed.bold)
        self.assert_folded_row(terminal, "palette.rs")
        long = self.theme_row(terminal, "printf")
        self.assertIn("…", terminal.screen.display[long])
        self.assertRegex(terminal.screen.display[long], r"\d+\s*$")
        self.assert_theme_row(terminal, "The colors are ready.", "✓", "a6e3a1", "cdd6f4")
        answer = self.theme_row(terminal, "The details remain")
        self.assertEqual(terminal.screen.buffer[answer][0].data, " ")
        self.assertNotIn("rows hidden", terminal.text)
        self.assertNotIn("─", terminal.text)
        self.assertEqual(terminal.requests("turn/start"), [])
        self.assertTrue(terminal.requests("thread/resume"))
        terminal.snapshot("3c-phone")
        self.tools_command(terminal, "/tools full", "full")
        terminal.contains("let tone_7 = 7;")
        terminal.contains("check failed detail")
        self.assertEqual(terminal.requests("turn/start"), [])

    def test_active_reasoning_has_a_spinner_and_returns_to_italic_when_stopped(self):
        terminal = self.launch(mode="theme-active", resume=None, width=54, layout="desktop")
        terminal.wait(lambda: terminal.input_is(""), "thread ready")
        terminal.paste("check theme")
        terminal.send(b"\r")
        self.assert_theme_row(terminal, "Analyzing key config files", "◆", "cba6f7", "cba6f7")
        text = self.source_cell(terminal, "Analyzing key config files", "Analyzing")
        self.assertTrue(text.bold)
        self.assertFalse(text.italics)
        self.assertRegex(terminal.text, "[⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏]")
        terminal.snapshot("3c-active-step")
        terminal.resize(18, 32)
        terminal.wait(lambda: terminal.screen.columns == 18 and
                      sum("\n".join(terminal.screen.display[:-2]).count(c) for c in "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏") == 1,
                      "one spinner after the wrapped active step")
        terminal.snapshot("3c-active-step-narrow")
        terminal.resize(54, 32)
        terminal.contains("Analyzing key config files")
        terminal.send(b"\x1b")
        terminal.wait(lambda: terminal.requests("turn/interrupt"), "interrupt active turn")
        terminal.contains("Turn interrupted")
        self.assert_theme_row(terminal, "Analyzing key config files", "◇", "cba6f7", "ac9fd1")
        self.assertTrue(self.source_cell(terminal, "Analyzing key config files", "Analyzing").italics)
        self.assertEqual(len(terminal.requests("turn/start")), 1)

    def assert_status_separator(self, terminal):
        cells = terminal.screen.buffer[terminal.screen.lines - 2]
        separator = [cell for cell in cells.values() if cell.data == "━"]
        self.assertTrue(separator)
        self.assertTrue(all(cell.fg == "725f8e" for cell in separator))

    def test_context_percentage_warns_while_separator_keeps_one_color(self):
        case_root = None
        for percent, color in [(70, "89b4fa"), (71, "f9e2af"), (90, "f9e2af"), (91, "f38ba8")]:
            terminal = self.launch(mode=f"theme-{percent}", width=100,
                                   directory=case_root / str(percent) if case_root else None)
            case_root = case_root or terminal.root
            row = self.theme_row(terminal, f"{percent}%")
            self.assertEqual(self.source_cell(terminal, f"{percent}%", f"{percent}%").fg, color)
            self.assertEqual(self.source_cell(terminal, f"{percent}%", f"{percent}k/100k").fg, "9399b2")
            self.assert_status_separator(terminal)
            self.assertEqual(terminal.requests("turn/start"), [])
            model = self.source_cell(terminal, f"{percent}%", "gpt-5.6-sol")
            self.assertEqual(model.fg, "6c7086")
            terminal.resize(40, 32)
            terminal.contains(f"{percent}%")
            self.assert_status_separator(terminal)
            self.assertEqual(self.source_cell(terminal, f"{percent}%", f"{percent}%").fg, color)
            terminal.snapshot(f"3c-context-phone-{percent}")
            terminal.snapshot(f"3c-context-{percent}")
            terminal.close()
