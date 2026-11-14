"""Keep a saved Grok transcript readable while native history and an early send wait."""


class GrokStartup:
    def test_grok_restores_more_than_one_native_queue_window_before_sending_queued_text(self):
        terminal = self.launch(backend="grok", resume="grok-saved")
        terminal.wait(lambda: terminal.requests("session/load"), "native restore waits at its gate")
        terminal.wait(lambda: terminal.input_is(""), "terminal accepts input while native restore waits")
        terminal.paste("after restore")
        terminal.wait(lambda: terminal.input_is("after restore"), "human can type while native restore waits")
        terminal.send(b"\r")
        terminal.wait(lambda: terminal.input_is(""), "human text is accepted into the pending queue")
        self.assertEqual(terminal.requests("session/prompt"), [])
        terminal.snapshot("human-input-queued-during-native-restore")
        (terminal.root / "release-history").write_text("go")
        terminal.contains("Answer: after restore")
        self.assertEqual(len(terminal.requests("session/load")), 1)
        prompts = terminal.requests("session/prompt")
        self.assertEqual(len(prompts), 1)
        self.assertEqual(prompts[0]["params"]["sessionId"], "grok-saved")
        self.assertEqual(prompts[0]["params"]["prompt"][0]["text"], "after restore")
        self.assertNotIn("Grok closed", terminal.text)
        terminal.snapshot("restored-history-and-one-human-turn")
