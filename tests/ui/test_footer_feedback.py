"""Keep copy confirmations temporary and native activity labels steady on the actual terminal."""

import time

from test_daemon_selection_copy import DaemonSelectionCopy


class FooterFeedback:
    def test_claude_large_thinking_keeps_the_footer_while_a_new_line_waits_for_source(self):
        fixture, _, client, session, _, terminal = self.claude_history_terminal("claude-large-footer-source")
        terminal.paste("private large-thinking draft 🌍")
        sent = client.operation(2)
        client.call("session/send", session=session, operation=sent, text="native large thinking")
        self.assertEqual(client.outcome(sent)["state"]["kind"], "accepted")
        terminal.contains("Visible streaming thought 日本語")
        terminal.contains("Thinking ~300 tokens")
        terminal.daemon_recorder.pause_sources()
        (fixture.root / "append-thinking-line").touch()
        terminal.wait(lambda: terminal.daemon_recorder.held_sources, "the appended thinking source is held")
        terminal.contains("Thinking ~301 tokens")
        self.assertIn("Visible streaming thought 日本語", terminal.text)
        self.assertNotIn("Next thinking line 🌍", terminal.text)
        self.assertNotIn("Loading visible source", terminal.screen.display[-2])
        terminal.daemon_recorder.resume_sources()
        terminal.contains("Next thinking line 🌍")
        self.assertTrue(terminal.input_is("private large-thinking draft 🌍"))
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("large-thinking-source-read-keeps-the-normal-footer")

    def test_claude_thinking_text_appends_without_replacing_the_footer(self):
        fixture, _, client, session, _, terminal = self.claude_history_terminal("claude-footer-source")
        terminal.paste("private draft during native thinking 🌍")
        sent = client.operation(2)
        client.call("session/send", session=session, operation=sent, text="native streaming thinking")
        self.assertEqual(client.outcome(sent)["state"]["kind"], "accepted")
        terminal.contains("Visible streaming thought 日本語")
        terminal.contains("Thinking ~300 tokens")
        before = len((terminal.root / "terminal.ansi").read_bytes())
        (fixture.root / "append-thinking-line").touch()
        terminal.contains("Next thinking line 🌍")
        terminal.contains("Thinking ~301 tokens")
        self.assertNotIn("Loading visible source", terminal.screen.display[-2])
        raw = (terminal.root / "terminal.ansi").read_bytes()[before:]
        self.assertNotIn(b"Loading visible source", raw, "even an intermediate frame must keep the real footer")
        self.assertTrue(terminal.input_is("private draft during native thinking 🌍"))
        terminal.contains("Visible streaming thought 日本語")
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertEqual(terminal.requests("session/history"), [])
        terminal.snapshot("visible-native-thought-and-steady-footer-after-append")

    def footer_copy_fixture(self, name):
        terminal = self.launch(backend="claude", resume=None, directory=self.summary_root()/name,
                               extra_env={"SSH_TTY":"/dev/pts/fixture"})
        self.claude_ready(terminal)
        terminal.paste("copy-notice-token")
        terminal.send(b"\r")
        terminal.contains("Answer: copy-notice-token")
        terminal.wait(lambda: "Idle" in terminal.screen.display[-2], "completed turn footer")
        return terminal

    def assert_footer_restored_after_copy(self, terminal, message):
        terminal.wait(lambda: message in terminal.screen.display[-2], "copy confirmation")
        self.assertIn(b"\x1b]52;", (terminal.root/"terminal.ansi").read_bytes(), "copy used actual OSC52")
        terminal.wait(lambda: message not in terminal.screen.display[-2]
                      and "sdk-sonnet" in terminal.screen.display[-2]
                      and "Idle" in terminal.screen.display[-2], "copy notice expires without another key", timeout=4)
        self.assertEqual(len(self.helper_requests(terminal, "prompt")), 1)
        terminal.snapshot("normal-footer-restored-after-copy")

    def test_selection_copy_notice_expires_and_restores_the_normal_footer(self):
        terminal = self.footer_copy_fixture("selection")
        start = DaemonSelectionCopy.point(terminal, "Answer: copy-notice-token")
        end = DaemonSelectionCopy.point(terminal, "Answer: copy-notice-token", end=True)
        DaemonSelectionCopy.mouse(terminal, 0, start)
        DaemonSelectionCopy.mouse(terminal, 32, end)
        DaemonSelectionCopy.mouse(terminal, 0, end, release=True)
        self.assert_footer_restored_after_copy(terminal, "copied selection")

    def test_original_answer_copy_notice_expires_and_restores_the_normal_footer(self):
        terminal = self.footer_copy_fixture("original")
        terminal.send(b"\x19")
        self.assert_footer_restored_after_copy(terminal, "copied last assistant message")

    def test_native_requesting_is_quiet_and_keeps_the_real_thinking_count(self):
        terminal = self.launch(backend="claude", resume=None, width=100)
        self.claude_ready(terminal)
        terminal.paste("native requesting")
        terminal.send(b"\r")
        terminal.wait(lambda: any(frame.get("message", {}).get("status") == "requesting"
                      for frame in self.helper_messages(terminal, "server")), "actual SDK requesting status")
        terminal.paste("private requesting draft")
        terminal.wait(lambda: terminal.input_is("private requesting draft"), "composer after native status")
        terminal.wait(lambda: "Thinking ~43 tokens" in terminal.screen.display[-2], "real thinking count survives generic requesting status")
        self.assertNotIn("Requesting", terminal.screen.display[-2])
        self.assertEqual(len(self.helper_requests(terminal, "prompt")), 1)
        terminal.snapshot("requesting-does-not-replace-real-status")

    def test_flow_footer_animation_keeps_label_colors_and_weight_steady(self):
        terminal = self.launch(backend="claude", resume=None, width=36, layout="flow")
        self.claude_ready(terminal)
        terminal.paste("native thinking")
        terminal.send(b"\r")
        terminal.wait(lambda: "Thinking ~43 tokens" in terminal.screen.display[-2]
                      and "26%" in terminal.screen.display[-2], "stable native footer facts")
        styles = {}
        deadline = time.monotonic() + 4
        while time.monotonic() < deadline:
            terminal.pump(.025)
            for column, char in enumerate(terminal.screen.display[-2]):
                if char not in (" ", "━"):
                    cell = terminal.screen.buffer[terminal.screen.lines-2][column]
                    styles.setdefault((column, char), set()).add((cell.fg, cell.bold))
        self.assertTrue(styles)
        changed = {key:values for key, values in styles.items() if len(values) > 1}
        self.assertEqual(changed, {}, "the scanner may animate the separator, never flash status labels")
        self.assertEqual(len(self.helper_requests(terminal, "prompt")), 1)
        terminal.snapshot("steady-flow-footer-labels")
