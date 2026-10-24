"""Check source token colors in the real terminal, including saved reads."""


class SourceColors:
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
        terminal.wait(lambda: terminal.input_is(""), "new thread ready")
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
