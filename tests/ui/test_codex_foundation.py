"""Check the native handshake, complete item pages and visible pipe failures."""


class CodexFoundation:
    def test_codex_current_history_uses_metadata_and_overlapping_item_pages(self):
        terminal = self.launch(mode="codex-current")
        terminal.contains("Answer: first")
        terminal.contains("Answer: second")
        self.assertEqual(terminal.text.count("Answer: first"),1)
        self.assertEqual(terminal.text.count("Answer: second"),1)
        methods = [message.get("method") for message in terminal.messages()]
        self.assertLess(methods.index("initialize"),methods.index("initialized"))
        self.assertLess(methods.index("initialized"),methods.index("thread/resume"))
        self.assertTrue(terminal.requests("thread/resume")[0]["params"]["excludeTurns"])
        self.assertTrue(terminal.requests("thread/read"))
        self.assertTrue(all(row["params"]["includeTurns"] is False for row in terminal.requests("thread/read")),"metadata refresh must never fetch deprecated full history")
        self.assertTrue(all(page["params"]["itemsView"] == "full" for page in terminal.requests("thread/turns/list")))
        self.assertTrue(any(page["params"]["cursor"] == "items-page-2" for page in terminal.requests("thread/items/list")))
        terminal.snapshot("current-paged-history")
        self.edit(terminal,"second","current edit")
        self.assertTrue(terminal.requests("thread/fork")[0]["params"]["excludeTurns"])
        self.assertEqual(terminal.text.count("Answer: first"),1)
        self.assertNotIn("Answer: second",terminal.text)

    def test_codex_full_turn_history_is_atomic_visible_and_avoids_item_rpc_amplification(self):
        terminal = self.launch(mode="codex-current-full")
        terminal.contains("Answer: first")
        terminal.contains("Answer: second")
        self.assertEqual(terminal.text.count("Answer: first"),1)
        self.assertEqual(terminal.text.count("Answer: second"),1)
        self.assertEqual(terminal.requests("thread/items/list"),[])
        self.assertEqual(len(terminal.requests("thread/turns/list")),1)
        self.assertEqual(terminal.requests("thread/turns/list")[0]["params"]["itemsView"],"full")
        terminal.rewind("second")
        self.assertEqual(terminal.requests("turn/start"),[])
        self.assertEqual(terminal.requests("thread/fork"),[])
        terminal.snapshot("batched-canonical-history")

    def test_codex_large_history_error_is_visible_and_does_not_leave_connecting_forever(self):
        terminal = self.launch(mode="codex-history-error",width=72)
        terminal.contains("Reconnect failed:")
        terminal.wait(lambda:"Fixture history denied:" in " ".join(terminal.text.split()),"visible native failure reason across wrapped lines")
        self.assertNotIn("irrelevant-large-error-data",terminal.text)
        terminal.paste("keep failure draft")
        terminal.wait(lambda:terminal.input_is("keep failure draft"),"recoverable composer")
        self.assertNotIn("Codex connecting",terminal.text)
        self.assertEqual(terminal.requests("turn/start"),[])
        terminal.snapshot("bounded-history-failure")

    def test_codex_pipe_eof_preserves_the_draft_and_does_not_queue_future_prompts(self):
        terminal = self.launch(mode="codex-pipe-eof")
        terminal.contains("Answer: second")
        terminal.paste("recover me 日本語")
        terminal.send(b"\r")
        terminal.contains("Codex stopped:")
        terminal.contains("stdout closed")
        self.assertTrue(terminal.input_is("recover me 日本語"))
        terminal.send(b"\r")
        terminal.contains("Could not send message:")
        self.assertTrue(terminal.input_is("recover me 日本語"))
        self.assertEqual(len(terminal.requests("turn/start")),1)
        terminal.snapshot("closed-draft-kept")

    def test_codex_oversized_native_frame_fails_visibly_without_losing_input(self):
        terminal = self.launch(mode="codex-pipe-oversize",width=72)
        terminal.contains("Answer: second")
        terminal.paste("keep this draft")
        terminal.send(b"\r")
        terminal.contains("stdout frame exceeds 8 MiB")
        self.assertTrue(terminal.input_is("keep this draft"))
        self.assertEqual(len(terminal.requests("turn/start")),1)
        terminal.snapshot("bounded-native-frame")
