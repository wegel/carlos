"""Check source token colors in the real terminal, including saved reads."""

import json


class SourceColors:
    def test_compact_diff_counts_include_header_like_and_colored_source_lines(self):
        cases = [
            ("header-like.rs", "--- a/header-like.rs\n+++ b/header-like.rs\n@@ -1 +1 @@\n--- old source\n+++ new source\n", "+1 −1"),
            ("colored.rs", "diff --git a/colored.rs b/colored.rs\n--- a/colored.rs\n+++ b/colored.rs\n@@ -1 +1 @@\n\x1b[31m-old\x1b[0m\n\x1b[32m+new 🌍\x1b[0m\n", "+1 −1"),
            ("empty.rs", "--- a/empty.rs\n+++ b/empty.rs\n@@ -0,0 +0,0 @@\n--- a/next.rs\n+++ b/next.rs\n@@ -0,0 +1 @@\n+new\n", "+1 −0"),
        ]
        items = [{"type":"toolOutput", "id":path, "files":[{"filePath":path, "diff":diff}]} for path, diff, _ in cases]
        directory = self.summary_root()
        (directory / "server-state.json").write_text(json.dumps({"thread-original":{
            "id":"thread-original", "turns":[{"id":"diff-counts", "status":"completed", "items":items}],
        }}))
        terminal = self.launch(directory=directory, width=54)
        terminal.wait_ready_composer()
        for path, _, counts in cases:
            terminal.contains(f"{path} {counts}")
        self.assertTrue(terminal.requests("thread/resume"))
        self.assertTrue(terminal.requests("thread/turns/list"))
        self.assertEqual(terminal.requests("turn/start"), [])
        self.assertEqual(terminal.requests("turn/steer"), [])
        terminal.snapshot("compact-native-diff-counts")

    def source_cell(self, terminal, line_text, token):
        row = next(y for y, line in enumerate(terminal.screen.display) if line_text in line)
        column = terminal.screen.display[row].index(token)
        return terminal.screen.buffer[row][column]

    def assert_source_colors(self, terminal):
        terminal.contains('1:let greeting = "hello";')
        terminal.contains('+let value = "after";')
        keyword = self.source_cell(terminal, '1:let greeting', 'let')
        string = self.source_cell(terminal, '1:let greeting', 'hello')
        self.assertEqual(keyword.fg, 'cba6f7')
        self.assertEqual(string.fg, 'a6e3a1')
        added = self.source_cell(terminal, '+let value', 'let')
        plus = self.source_cell(terminal, '+let value', '+')
        minus = self.source_cell(terminal, '-let value', '-')
        self.assertEqual(added.fg, keyword.fg)
        self.assertEqual(added.bg, '181825')
        self.assertEqual(plus.fg, 'a6e3a1')
        self.assertEqual(minus.fg, 'f38ba8')

    def test_live_read_and_diff_show_source_colors(self):
        terminal = self.launch(mode="source-colors", resume=None)
        terminal.wait_ready_composer()
        terminal.paste("show source")
        terminal.send(b"\r")
        terminal.contains("Answer: show source")
        self.assert_source_colors(terminal)
        requests = terminal.requests("turn/start")
        self.assertEqual(len(requests), 1)
        self.assertEqual(requests[0]["params"]["threadId"], "thread-new")
        self.assertEqual(requests[0]["params"]["input"][0]["text"], "show source")
        terminal.snapshot("source-colors-live")
        self.tools_command(terminal, "/tools full", "full")
        terminal.resize(54, 32)
        self.assert_source_colors(terminal)
        terminal.snapshot("source-colors-narrow")

    def test_resumed_command_read_and_diff_show_source_colors(self):
        terminal = self.launch(mode="source-history")
        self.assert_source_colors(terminal)
        terminal.contains("$ cat main.rs")
        self.assertTrue(terminal.requests("thread/resume"))
        self.assertTrue(terminal.requests("thread/turns/list"))
        self.assertEqual(terminal.requests("turn/start"), [])
        terminal.snapshot("source-colors-resumed")
