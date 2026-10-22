"""Run native Codex monitor and session-name checks with a local, fixed-reply model peer."""

import argparse
import json
from pathlib import Path
import shutil
import tempfile
import unittest

from mock_responses import Model, answer, outputs, tool
from terminal import Terminal


class NativeMonitor(unittest.TestCase):
    def test_rename_uses_native_codex_metadata_and_survives_reload(self):
        terminal, model = self.launch(lambda _body: [answer("Saved conversation")])
        terminal.paste("/rename Before the first prompt")
        terminal.send(b"\r")
        terminal.wait(lambda: terminal.placeholder_is("Before the first prompt"), "new session name")
        self.assertEqual(model.requests, [], "naming a new session must not invoke the model")
        terminal.paste("hello")
        terminal.send(b"\r")
        terminal.contains("Saved conversation")
        terminal.paste("/rename Native session name")
        terminal.send(b"\r")
        terminal.wait(lambda: terminal.placeholder_is("Native session name"), "saved name")
        terminal.send(b"\r")
        self.assertEqual(len(terminal.requests("turn/start")), 1)
        self.assertEqual(len(model.requests), 1, "rename must not invoke the model")
        self.assertEqual(len(terminal.requests("thread/name/set")), 2)
        thread_id = terminal.requests("turn/start")[0]["params"]["threadId"]
        terminal.close()
        resume_root = terminal.root / "resumed"
        resume_root.mkdir()
        (resume_root / "codex-home").symlink_to(terminal.root / "codex-home", target_is_directory=True)
        resumed = Terminal(BINARY, resume_root, resume=thread_id, codex=CODEX)
        self.addCleanup(resumed.close)
        resumed.contains("Saved conversation")
        resumed.wait(lambda: resumed.placeholder_is("Native session name"), "restored name")

    def launch(self, reply):
        root = ARTIFACTS / self._testMethodName
        root.mkdir(parents=True)
        model = Model(root, reply)
        self.addCleanup(model.close)
        terminal = Terminal(BINARY, root, resume=None, codex=CODEX)
        self.addCleanup(terminal.close)
        terminal.wait(lambda: terminal.input_is(""), "composer ready")
        return terminal, model

    def test_native_sleep_wakes_on_user_input_without_an_extra_turn(self):
        def reply(body):
            if "monitor-sleep" in outputs(body):
                return [answer("Checked immediately after your message")]
            return [tool("monitor-sleep", "clock", "sleep", {"duration_ms": 120000})]

        terminal, model = self.launch(reply)
        terminal.paste("Monitor cleanup")
        terminal.send(b"\r")
        terminal.contains("Wait up to 120 s")
        terminal.snapshot("native-wait")
        terminal.paste("Check now")
        terminal.send(b"\r")
        terminal.contains("Checked immediately after your message")
        self.assertEqual(len(terminal.requests("turn/start")), 1)
        self.assertEqual(len(terminal.requests("turn/steer")), 1)
        self.assertEqual(terminal.text.count("Wait up to 120 s"), 1)
        self.assertIn("Sleep interrupted by new input", json.dumps(model.requests))

    def test_native_child_finishes_after_parent_answer_and_survives_reload(self):
        def reply(body):
            text = json.dumps(body.get("input", []))
            if "monitor-spawn" not in text and "native-monitor-child" in text:
                self.assertTrue(model.release.wait(15), "test must release the child")
                return [answer("Cleanup finished")]
            if "Cleanup finished" in text:
                return [answer("Monitor report: cleanup finished")]
            if "Read the monitor result" in text:
                return []  # Let Codex drain the queued child message at its next safe point.
            if "monitor-spawn" in outputs(body):
                return [answer("Monitor running")]
            return [tool("monitor-spawn", "collaboration", "spawn_agent", {
                "task_name": "monitor", "message": "native-monitor-child", "fork_turns": "none"})]

        terminal, model = self.launch(reply)
        terminal.paste("Start a background monitor")
        terminal.send(b"\r")
        terminal.contains("Monitor running")
        terminal.contains("Agent /root/monitor started")
        terminal.wait(lambda: any(message.get("method") == "turn/completed"
                      for message in terminal.messages("server")), "parent finished")
        model.release.set()
        terminal.contains("Agent /root/monitor completed")
        terminal.snapshot("native-child-completed")
        self.assertEqual(terminal.text.count("Agent /root/monitor completed"), 1)
        self.assertNotIn("Cleanup finished", terminal.text,
                         "child messages must not appear as the parent's answer")
        self.assertEqual(len(terminal.requests("turn/start")), 1,
                         "Carlos must not invent a new turn on child completion")
        terminal.paste("Read the monitor result")
        terminal.send(b"\r")
        terminal.contains("Monitor report: cleanup finished")
        thread_id = terminal.requests("turn/start")[0]["params"]["threadId"]
        terminal.close()
        resume_root = terminal.root / "resumed"
        resume_root.mkdir()
        (resume_root / "codex-home").symlink_to(terminal.root / "codex-home", target_is_directory=True)
        resumed = Terminal(BINARY, resume_root, resume=thread_id, codex=CODEX)
        self.addCleanup(resumed.close)
        resumed.contains("Monitor report: cleanup finished")
        resumed.contains("Agent /root/monitor completed")
        self.assertEqual(resumed.text.count("Agent /root/monitor completed"), 1)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--codex", default="codex")
    parser.add_argument("--artifacts", type=Path)
    args, remaining = parser.parse_known_args()
    BINARY = args.binary.resolve()
    CODEX = Path(shutil.which(args.codex) or args.codex).resolve()
    ARTIFACTS = (args.artifacts or Path(tempfile.mkdtemp(prefix="carlos-native-monitor-"))).resolve()
    unittest.main(argv=[__file__, *remaining])
