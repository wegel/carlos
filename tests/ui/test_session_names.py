"""Session-name screen and backend checks using the deterministic app-server peer."""


class SessionNames:
    def test_rename_prefix_is_not_a_command_and_multiline_names_are_rejected(self):
        terminal = self.launch(mode="named")
        self.assert_placeholder(terminal, "Cleanup monitor")
        terminal.paste("/renameish ordinary text")
        terminal.send(b"\r")
        terminal.contains("Answer: /renameish ordinary text")
        terminal.paste("/rename invalid\nsecond line")
        terminal.send(b"\r")
        terminal.contains("Could not rename session:")
        self.assertEqual(terminal.requests("thread/name/set"), [])
        self.assertEqual(len(terminal.requests("turn/start")), 1)

    def assert_placeholder(self, terminal, name):
        terminal.wait(lambda: terminal.placeholder_is(name) and terminal.screen.cursor.x == 2,
                      "session name placeholder: " + name)
        self.assertTrue(terminal.screen.display[-1].endswith(name + " "), "name must meet the right margin")
        self.assertEqual(terminal.screen.cursor.x, 2, "the empty composer cursor stays at the left")
        self.assertIn(b"\x1b[2m", (terminal.root / "terminal.ansi").read_bytes())

    def test_name_is_a_dim_placeholder_not_an_editable_or_submitted_prompt(self):
        terminal = self.launch(mode="named")
        self.assert_placeholder(terminal, "Cleanup monitor")
        terminal.send(b"\r")
        terminal.paste("draft")
        terminal.wait(lambda: terminal.input_is("draft"), "draft replaces placeholder")
        self.assertNotEqual(terminal.screen.buffer[terminal.screen.lines - 1][2].fg, "6c7086")
        terminal.send(b"\x01\x0b")
        self.assert_placeholder(terminal, "Cleanup monitor")
        self.assertEqual(terminal.requests("turn/start"), [])
        self.assertEqual(terminal.requests("turn/steer"), [])
        terminal.snapshot("dim-session-name")

    def test_rename_saves_name_without_changing_transcript_or_rewind_history(self):
        terminal = self.launch(mode="named")
        self.assert_placeholder(terminal, "Cleanup monitor")
        terminal.paste("/rename Release checks")
        terminal.send(b"\r")
        self.assert_placeholder(terminal, "Release checks")
        self.assertEqual(terminal.requests("thread/name/set")[0]["params"],
                         {"threadId": "thread-original", "name": "Release checks"})
        self.assertEqual(terminal.requests("turn/start"), [])
        self.assertIn("Answer: second", terminal.text)
        terminal.rewind("second")

    def test_bare_rename_loads_the_current_name_for_editing(self):
        terminal = self.launch(mode="named")
        self.assert_placeholder(terminal, "Cleanup monitor")
        terminal.paste("/rename")
        terminal.send(b"\r")
        terminal.wait(lambda: terminal.input_is("/rename Cleanup monitor"), "edit current name")
        self.assertNotEqual(terminal.screen.buffer[terminal.screen.lines - 1][2].fg, "6c7086")
        self.assertEqual(terminal.requests("thread/name/set"), [])
        terminal.replace_input("/rename Updated name")
        terminal.send(b"\r")
        self.assert_placeholder(terminal, "Updated name")

    def test_failed_rename_keeps_the_name_and_draft_for_retry(self):
        terminal = self.launch(mode="named-error")
        self.assert_placeholder(terminal, "Cleanup monitor")
        terminal.paste("/rename Retry name")
        terminal.send(b"\r")
        terminal.contains("Could not rename session:")
        self.assertTrue(terminal.input_is("/rename Retry name"))
        terminal.send(b"\x01\x0b")
        self.assert_placeholder(terminal, "Cleanup monitor")
        terminal.paste("/rename Retry name")
        terminal.send(b"\r")
        self.assert_placeholder(terminal, "Retry name")
        self.assertEqual(len(terminal.requests("thread/name/set")), 2)
        self.assertEqual(terminal.requests("turn/start"), [])

    def test_name_updates_from_codex_and_foreign_updates_are_ignored(self):
        terminal = self.launch(mode="named-external")
        self.assert_placeholder(terminal, "Cleanup monitor")
        terminal.paste("hello")
        terminal.send(b"\r")
        terminal.contains("Answer: hello")
        self.assert_placeholder(terminal, "Changed in native Codex")
        self.assertEqual(len(terminal.requests("turn/start")), 1)
        self.assertEqual(terminal.requests("thread/name/set"), [])

    def test_rename_does_not_steer_an_active_turn(self):
        terminal = self.launch(mode="named-live")
        self.assert_placeholder(terminal, "Cleanup monitor")
        terminal.paste("Monitor cleanup")
        terminal.send(b"\r")
        terminal.contains("Monitor cleanup")
        self.assert_placeholder(terminal, "Cleanup monitor")
        terminal.paste("/rename Live monitor")
        terminal.send(b"\r")
        self.assert_placeholder(terminal, "Live monitor")
        self.assertEqual(len(terminal.requests("turn/start")), 1)
        self.assertEqual(terminal.requests("turn/steer"), [])
        self.assertEqual(terminal.requests("turn/interrupt"), [])

    def test_unnamed_fork_does_not_show_the_parent_name(self):
        terminal = self.launch(mode="named-fork")
        self.assert_placeholder(terminal, "Cleanup monitor")
        self.edit(terminal, "second", "edited second")
        terminal.wait(lambda: terminal.screen.display[-1].strip() == ">", "fork has no name")
        terminal.paste("/rename Fork name")
        terminal.send(b"\r")
        self.assert_placeholder(terminal, "Fork name")
        self.assertEqual(terminal.requests("thread/name/set")[0]["params"]["threadId"], "thread-fork-1")

    def test_long_unicode_name_clips_when_resizing_and_reappears_after_typing(self):
        terminal = self.launch(mode="named")
        self.assert_placeholder(terminal, "Cleanup monitor")
        name = "Résumé 漢字 🦀 " * 12
        terminal.paste("/rename " + name.strip())
        terminal.send(b"\r")
        terminal.wait(lambda: "Résumé" in terminal.screen.display[-1], "unicode title")
        terminal.resize(28, 24)
        terminal.wait(lambda: "Résumé" in terminal.screen.display[-1], "clipped unicode title")
        self.assertTrue(terminal.placeholder_is())
        terminal.paste("x")
        terminal.wait(lambda: terminal.input_is("x"), "typing hides title")
        terminal.send(b"\x7f")
        terminal.wait(lambda: "Résumé" in terminal.screen.display[-1], "clearing restores title")
        self.assertEqual(terminal.requests("turn/start"), [])
