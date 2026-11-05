"""Check right-aligned quota labels, pace colors, live updates, and a withheld read."""


class WeeklyUsage:
    def weekly_row(self, terminal, label):
        terminal.wait(lambda: terminal.screen.display[-2].rstrip().endswith(label),
                      "weekly quota is at the far right")
        row = terminal.screen.lines - 2
        start = terminal.screen.columns - 1 - len(label)
        self.assertEqual(terminal.screen.display[row][start:start+len(label)], label)
        return row, start

    def assert_weekly(self, terminal, label, color):
        row, start = self.weekly_row(terminal, label)
        cells = terminal.screen.buffer[row]
        self.assertEqual(cells[start].fg, "6c7086")
        for x in range(start+1, start+label.index("/")):
            self.assertEqual(cells[x].fg, color)
        for x in range(start+label.index("/"), start+len(label)):
            self.assertEqual(cells[x].fg, "6c7086")

    def test_weekly_quota_keeps_one_format_and_its_right_edge_when_resizing(self):
        terminal = self.launch(mode="weekly-blue")
        self.assert_weekly(terminal, "W40%/2.1d", "89b4fa")
        terminal.contains("91k/100k 91%")
        for width in [54, 40, 32, 18, 12, 8, 100]:
            terminal.resize(width, 32)
            if width >= 12:
                self.assert_weekly(terminal, "W40%/2.1d", "89b4fa")
                terminal.snapshot(f"weekly-{width}")
            else:
                terminal.wait(lambda: terminal.screen.columns == width, "very narrow screen")
            if width == 40:
                terminal.contains("91k/100k 91%")
                self.assertIn("…", terminal.screen.display[-2])
                self.assertNotIn("gpt-5.6-sol/high/concise", terminal.screen.display[-2])
        reads = terminal.requests("account/rateLimits/read")
        self.assertEqual(len(reads), 1)
        self.assertEqual(reads[0]["params"], {})
        self.assertIsInstance(reads[0]["id"], str)
        self.assertEqual(terminal.requests("turn/start"), [])

    def test_weekly_percentage_warns_by_pace_and_preserves_sparse_updates(self):
        terminal = self.launch(mode="weekly-yellow")
        self.assert_weekly(terminal, "W40%/3.5d", "f9e2af")
        terminal.close()
        terminal = self.launch(mode="weekly-red", directory=terminal.root / "red")
        self.assert_weekly(terminal, "W40%/6.0d", "f38ba8")
        terminal.close()
        terminal = self.launch(mode="weekly-updates", directory=terminal.root / "updates")
        self.assert_weekly(terminal, "W40%/2.1d", "89b4fa")
        for prompt, label, color in [
            ("update quota", "W40%/2.1d", "89b4fa"),
            ("use quota", "W10%/2.1d", "f38ba8"),
        ]:
            terminal.paste(prompt)
            terminal.send(b"\r")
            terminal.contains("Answer: " + prompt)
            self.assert_weekly(terminal, label, color)
        terminal.paste("expire quota")
        terminal.send(b"\r")
        terminal.contains("Answer: expire quota")
        terminal.wait(lambda: "W10%" not in terminal.screen.display[-2], "expired quota is hidden")
        self.assertEqual(len(terminal.requests("turn/start")), 3)
        self.assertEqual(terminal.requests("turn/steer"), [])

    def test_quota_read_does_not_block_input_or_send_a_model_turn(self):
        terminal = self.launch(mode="weekly-delayed", resume=None, width=40)
        terminal.wait(lambda: terminal.requests("account/rateLimits/read"), "quota read queued")
        terminal.paste("keep typing")
        terminal.wait(lambda: terminal.input_is("keep typing"), "draft stays editable during read")
        self.assertEqual(terminal.requests("turn/start"), [])
        terminal.send(b"\r")
        terminal.contains("Answer: keep typing")
        self.assert_weekly(terminal, "W40%/2.1d", "89b4fa")
        self.assertEqual(len(terminal.requests("turn/start")), 1)
        self.assertEqual(terminal.requests("turn/steer"), [])

    def test_missing_expired_or_unavailable_quotas_do_not_show_invented_values(self):
        parent = None
        for mode in ["weekly-missing", "weekly-expired", "weekly-error"]:
            terminal = self.launch(mode=mode, directory=parent / mode if parent else None)
            parent = parent or terminal.root
            terminal.wait(lambda: terminal.requests("account/rateLimits/read"), "quota read sent")
            terminal.paste("still editable")
            terminal.wait(lambda: terminal.input_is("still editable"), "composer remains usable")
            self.assertNotIn("W40%", terminal.screen.display[-2])
            self.assertNotIn("W100%", terminal.screen.display[-2])
            self.assertEqual(terminal.requests("turn/start"), [])
            terminal.close()
