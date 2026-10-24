"""Check RTK labels, numbered source colors, complete commands, and saved output."""

from rtk_fixture import CHAIN_COMMAND, READ_COMMAND, TEST_COMMAND


class RtkWorkflows:
    def assert_rtk_full(self, terminal):
        for command in [READ_COMMAND, TEST_COMMAND, CHAIN_COMMAND]:
            terminal.contains("$ " + command)
        terminal.contains('2 │ let value = "rtk hello";')
        terminal.contains("chain status marker")
        terminal.contains("chain diff marker")
        keyword = self.source_cell(terminal, "2 │ let value", "let")
        string = self.source_cell(terminal, "2 │ let value", "rtk hello")
        gutter = self.source_cell(terminal, "2 │ let value", "│")
        self.assertEqual(keyword.fg, "cba6f7")
        self.assertEqual(string.fg, "a6e3a1")
        self.assertEqual(gutter.fg, "a6adc8")

    def test_rtk_wrappers_keep_original_commands_and_color_native_reads(self):
        terminal = self.launch(mode="rtk", resume=None)
        self.start_tools(terminal)
        self.assert_rtk_full(terminal)
        terminal.resize(54, 32)
        terminal.contains("→ Read main file.rs")
        terminal.contains("Test cargo test -p demo")
        self.assertNotIn("rtk hello", terminal.text)
        self.assertNotIn("chain diff marker", terminal.text)
        self.tools_command(terminal, "/tools full", "full")
        terminal.resize(100, 32)
        self.assert_rtk_full(terminal)
        self.assertEqual(len(terminal.requests("turn/start")), 1)
        self.assertEqual(terminal.requests("turn/steer"), [])
        terminal.snapshot("rtk-original-commands-and-colors")

    def test_saved_rtk_reads_and_chained_commands_survive_unfolding(self):
        terminal = self.launch(mode="rtk-history", width=54)
        terminal.contains("→ Read main file.rs")
        terminal.contains("Test cargo test -p demo")
        self.tools_command(terminal, "/tools full", "full")
        terminal.resize(100, 32)
        self.assert_rtk_full(terminal)
        self.assertTrue(terminal.requests("thread/resume"))
        self.assertEqual(terminal.requests("turn/start"), [])

    def test_failed_wrapped_reads_keep_error_labels_and_received_log_references(self):
        terminal = self.launch(mode="rtk-failure", resume=None, width=54)
        self.start_tools(terminal)
        terminal.contains("Failed: → Read missing.rs")
        self.assertNotIn("head: missing.rs", terminal.text)
        self.tools_command(terminal, "/tools full", "full")
        terminal.contains("$ rtk --verbose proxy head -n 20 missing.rs")
        terminal.contains("head: missing.rs: no such file")
        terminal.contains("[full output: /tmp/rtk-read.log]")
        self.assertEqual(len(terminal.requests("turn/start")), 1)
