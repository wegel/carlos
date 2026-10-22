"""Executable UI checks. Run via scripts/test.sh or pass --binary directly."""

import argparse
import json
from pathlib import Path
import tempfile
import unittest

from terminal import Terminal
from test_session_names import SessionNames


class Workflows(SessionNames, unittest.TestCase):
    def test_native_monitor_activity_after_parent_completion_is_not_lost_or_duplicated(self):
        terminal = self.launch(mode="native-activity", resume=None)
        terminal.wait(lambda: terminal.input_is(""), "new thread ready")
        terminal.paste("Monitor cleanup")
        terminal.send(b"\r")
        terminal.contains("Answer: Monitor cleanup")
        terminal.contains("Wait up to 30 s")
        terminal.contains("Agent /root/monitor started")
        terminal.contains("Agent /root/monitor completed")
        self.assertEqual(terminal.text.count("Agent /root/monitor completed"), 1)
        self.assertEqual(len(terminal.requests("turn/start")), 1)
        self.assertEqual(terminal.requests("turn/start")[0]["params"]["threadId"], "thread-new")
        terminal.snapshot("native-monitor-completed")

    def launch(self, mode="modern", resume="thread-original", width=100, directory=None, backend="codex"):
        directory = directory or ARTIFACTS / self._testMethodName
        terminal = Terminal(BINARY, directory, mode, resume, width, backend)
        self.addCleanup(terminal.close)
        return terminal

    def edit(self, terminal, original, edited):
        terminal.rewind(original)
        terminal.replace_input(edited)
        terminal.send(b"\r")
        terminal.contains("Answer: " + edited)
        terminal.snapshot(edited.replace(" ", "-"))

    def test_edit_previous_message_and_edit_again(self):
        terminal = self.launch()
        terminal.contains("Answer: second")
        self.edit(terminal, "second", "edited second")
        self.assertNotIn("Answer: second", terminal.text)
        self.edit(terminal, "edited second", "edited again")
        forks = terminal.requests("thread/fork")
        self.assertEqual(len(forks), 2)
        self.assertEqual(forks[0]["params"]["beforeTurnId"], "turn-second")
        self.assertEqual(len(terminal.requests("thread/rollback")), 0)
        self.assertEqual(terminal.text.count("Answer: edited again"), 1)

    def test_edit_first_message(self):
        terminal = self.launch()
        terminal.contains("Answer: second")
        terminal.rewind("second")
        terminal.send(b"\x1b[A")
        terminal.wait(lambda: terminal.input_is("first"),
                      "select first message")
        terminal.replace_input("new first")
        terminal.send(b"\r")
        terminal.contains("Answer: new first")
        self.assertNotIn("Answer: first", terminal.text)
        self.assertNotIn("Answer: second", terminal.text)

    def test_edit_uses_turn_ids_when_automatic_turns_have_no_prompt(self):
        terminal = self.launch(mode="automatic")
        terminal.contains("Answer: second")
        terminal.rewind("second")
        terminal.send(b"\x1b[A")
        terminal.wait(lambda: terminal.input_is("first"), "select first message")
        terminal.replace_input("replace all")
        terminal.send(b"\r")
        terminal.contains("Answer: replace all")
        self.assertNotIn("Answer: first", terminal.text)
        self.assertNotIn("Answer: second", terminal.text)
        self.assertEqual(terminal.requests("thread/fork")[0]["params"]["beforeTurnId"], "turn-first")

    def test_cancel_rewind_preserves_thread_and_allows_next_turn(self):
        terminal = self.launch()
        terminal.contains("Answer: second")
        terminal.rewind("second")
        terminal.replace_input("discarded edit")
        terminal.send(b"\x1b")
        terminal.wait(lambda: "discarded edit" not in terminal.text, "cancel edit")
        terminal.paste("third")
        terminal.send(b"\r")
        terminal.contains("Answer: third")
        self.assertEqual(terminal.requests("thread/fork"), [])

    def test_legacy_server_can_still_edit(self):
        terminal = self.launch(mode="legacy")
        terminal.contains("Answer: second")
        self.edit(terminal, "second", "legacy edit")
        self.assertEqual(len(terminal.requests("thread/rollback")), 1)

    def test_paginated_resume_and_fork_merge_overlapping_pages(self):
        terminal = self.launch(mode="paginated")
        terminal.contains("Answer: second")
        self.assertEqual(terminal.text.count("Answer: first"), 1)
        self.edit(terminal, "second", "paged edit")
        self.assertEqual(terminal.text.count("Answer: first"), 1)
        self.assertNotIn("Answer: second", terminal.text)
        pages = terminal.requests("thread/turns/list")
        self.assertTrue(any(page["params"]["cursor"] == "page-2" for page in pages))
        self.assertTrue(all(page["params"]["itemsView"] == "full" for page in pages))

    def test_failed_fork_keeps_draft_and_original_thread(self):
        terminal = self.launch(mode="fork-error")
        terminal.contains("Answer: second")
        terminal.rewind("second")
        terminal.replace_input("retry this edit")
        terminal.send(b"\r")
        terminal.contains("Could not edit message:")
        self.assertEqual(terminal.requests("turn/start"), [])
        self.assertIn("retry this edit", terminal.text)
        terminal.send(b"\r")
        terminal.contains("Answer: retry this edit")
        self.assertEqual(len(terminal.requests("thread/fork")), 2)

    def test_failed_turn_keeps_edited_draft_for_retry(self):
        terminal = self.launch(mode="start-error")
        terminal.contains("Answer: second")
        terminal.rewind("second")
        terminal.replace_input("retry my prompt")
        terminal.send(b"\r")
        terminal.contains("Could not send message:")
        self.assertTrue(terminal.input_is("retry my prompt"))
        self.assertNotIn("Answer: second", terminal.text)
        terminal.send(b"\r")
        terminal.contains("Answer: retry my prompt")
        self.assertEqual(len(terminal.requests("thread/fork")), 1)
        self.assertEqual(terminal.text.count("Answer: retry my prompt"), 1)

    def test_failed_new_turn_does_not_add_an_unsent_rewind_entry(self):
        terminal = self.launch(mode="start-error")
        terminal.contains("Answer: second")
        terminal.paste("third")
        terminal.send(b"\r")
        terminal.contains("Could not send message:")
        terminal.send(b"\r")
        terminal.contains("Answer: third")
        terminal.rewind("third")
        terminal.send(b"\x1b[A")
        terminal.wait(lambda: terminal.input_is("second"), "previous sent message, not failed draft")

    def test_replayed_start_does_not_block_editing_a_completed_turn(self):
        terminal = self.launch(mode="replayed-start")
        terminal.contains("Answer: second")
        terminal.paste("third")
        terminal.send(b"\r")
        terminal.contains("Answer: third")

        self.edit(terminal, "third", "edited third")

        self.assertEqual(terminal.requests("turn/interrupt"), [])
        self.assertEqual(terminal.text.count("Answer: edited third"), 1)

    def test_claude_interrupt_allows_rewind_and_another_message(self):
        terminal = self.launch(backend="claude", resume=None)
        terminal.wait(lambda: any(message.get("type") == "system"
                                 for message in terminal.messages("server")), "Claude startup")
        terminal.paste("hold this turn")
        terminal.send(b"\r")
        terminal.contains("Answer: hold this turn")

        terminal.send(b"\x1b")
        terminal.wait(lambda: any(message.get("signal") == "SIGINT"
                                 for message in terminal.messages()), "interrupt Claude process")
        terminal.contains("Turn interrupted")
        terminal.rewind("hold this turn")
        terminal.send(b"\x1b")
        terminal.wait(lambda: terminal.input_is(""), "cancel rewind")
        terminal.paste("after interrupt")
        terminal.send(b"\r")
        terminal.contains("Answer: after interrupt")

        interrupts = [message for message in terminal.messages() if message.get("signal") == "SIGINT"]
        self.assertEqual(len(interrupts), 1)

    def test_new_thread_streams_and_resumes(self):
        terminal = self.launch(resume=None, width=72)
        terminal.wait(lambda: terminal.requests("model/list"), "startup")
        terminal.paste("fresh prompt")
        terminal.send(b"\r")
        terminal.contains("Answer: fresh prompt")
        terminal.resize(120, 40)
        terminal.contains("Answer: fresh prompt")
        self.assertEqual(terminal.text.count("Answer: fresh prompt"), 1)
        state_path = terminal.root / "server-state.json"
        terminal.wait(lambda: "fresh prompt" in state_path.read_text(), "persist new turn")
        state = json.loads(state_path.read_text())
        resumed_dir = ARTIFACTS / "resumed"
        resumed_dir.mkdir()
        (resumed_dir / "server-state.json").write_text(json.dumps(state))
        resumed = self.launch(resume="thread-new", directory=resumed_dir)
        resumed.contains("Answer: fresh prompt")
        self.assertEqual(resumed.text.count("Answer: fresh prompt"), 1)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path)
    options, test_args = parser.parse_known_args()
    BINARY = options.binary.resolve()
    ARTIFACTS = (options.artifacts or Path(tempfile.mkdtemp(prefix="carlos-ui-"))).resolve()
    ARTIFACTS.mkdir(parents=True, exist_ok=True)
    print("UI artifacts:", ARTIFACTS, flush=True)
    unittest.main(argv=[__file__, *test_args])
