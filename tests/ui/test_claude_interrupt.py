"""Keep the latest Claude reply visible when Escape stops native work."""

import time

from fake_claude_interrupt import LATEST, PROMPT


class ClaudeInterrupt:
    def interrupt_history(self, mode):
        terminal = self.launch(mode=mode, backend="claude", resume=None,
                               directory=self.summary_root() / mode)
        self.claude_ready(terminal)
        terminal.paste("Earlier human prompt")
        terminal.send(b"\r")
        terminal.contains("Answer: Earlier human prompt")
        terminal.paste(PROMPT)
        terminal.send(b"\r")
        terminal.contains(LATEST)
        self.assertNotIn(PROMPT, terminal.text)
        terminal.send(b"\x1b")
        terminal.contains("Turn interrupted")
        prompt_id = self.helper_requests(terminal, "prompt")[-1]["params"]["uuid"]
        terminal.wait(lambda: any(frame.get("type") == "result"
                                 and frame.get("user_message_uuid") == prompt_id
                                 for frame in terminal.messages("server"))
                      and "Running" not in terminal.text, "native interrupt result and idle")
        return terminal

    def test_claude_single_interrupt_keeps_latest_reply_in_both_ack_orders(self):
        for mode in ("claude-interrupt-before-ack", "claude-interrupt-after-ack"):
            with self.subTest(mode=mode):
                terminal = self.interrupt_history(mode)
                self.assertIn(LATEST, terminal.text)
                self.assertTrue(terminal.input_is(""))
                self.assertEqual(len(self.native_controls(terminal, "interrupt")), 1)
                self.assertEqual(len(self.helper_requests(terminal, "prompt")), 2)
                terminal.snapshot(mode)
                terminal.close()

    def test_claude_escape_burst_after_interrupt_does_not_enter_rewind(self):
        terminal = self.interrupt_history("claude-interrupt-after-ack")
        terminal.send(b"\x1b")
        terminal.pump(.02)
        terminal.send(b"\x1b")
        deadline = time.monotonic() + .2
        while time.monotonic() < deadline:
            terminal.pump(.01)
        self.assertIn(LATEST, terminal.text)
        self.assertTrue(terminal.input_is(""))
        self.assertEqual(len(self.native_controls(terminal, "interrupt")), 1)
        self.assertEqual(len(self.helper_requests(terminal, "prompt")), 2)
        terminal.snapshot("interrupt-escape-burst")
        deadline = time.monotonic() + .8
        while time.monotonic() < deadline:
            terminal.pump(.02)
        terminal.rewind(PROMPT)
        terminal.send(b"\x1b")
        terminal.contains(LATEST)
        self.assertTrue(terminal.input_is(""))

    def test_claude_interrupted_reply_matches_resumed_history(self):
        terminal = self.interrupt_history("claude-interrupt-after-ack")
        session = next(frame["session_id"] for frame in terminal.messages("server")
                       if frame.get("type") == "system" and frame.get("subtype") == "init")
        terminal.close()

        resumed = self.launch(backend="claude", resume=session, directory=terminal.root)
        self.claude_ready(resumed)
        resumed.contains(LATEST)
        self.assertTrue(resumed.input_is(""))
        self.assertEqual(resumed.text.count(LATEST), 1)
        self.assertTrue(self.helper_requests(resumed, "sessions.read"))
        self.assertEqual(len(self.helper_requests(resumed, "prompt")), 2)
        resumed.snapshot("interrupted-reply-resumed")
