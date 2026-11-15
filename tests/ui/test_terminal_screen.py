"""Check Unicode cell redraws and child cleanup without a backend or a model."""

import codecs
import os
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock

import pyte
from wcwidth import wcswidth

from terminal import Terminal
from terminal_screen import TerminalScreen


class TerminalScreenTests(unittest.TestCase):
    def render(self, text):
        screen = TerminalScreen(10, 2)
        stream = pyte.Stream(screen)
        decoder = codecs.getincrementaldecoder("utf-8")()
        for byte in text.encode():
            stream.feed(decoder.decode(bytes([byte])))
            self.assertTrue(all(wcswidth(line) == 10 for line in screen.display))
        return screen.display[0]

    def test_overwriting_either_half_erases_the_other_half_of_a_wide_glyph(self):
        for column, replacement, expected in [
            (1, "a", "a 本語    "),
            (2, "a", " a本語    "),
            (1, "ab", "ab本語    "),
            (3, "a", "日a 語    "),
        ]:
            with self.subTest(column=column, replacement=replacement):
                self.assertEqual(self.render(f"日本語\x1b[1;{column}H{replacement}"), expected)

    def test_intact_wide_glyphs_combining_accents_and_color_survive(self):
        screen = TerminalScreen(10, 2)
        pyte.Stream(screen).feed("\x1b[31me\u0301日本語")
        self.assertEqual(screen.display[0], "é日本語   ")
        self.assertEqual(screen.buffer[0][1].fg, "red")
        self.assertEqual(screen.buffer[0][2].data, "")

    def test_wide_glyph_accents_survive_until_either_half_is_overwritten(self):
        self.assertEqual(self.render("日\u0301"), "日\u0301        ")
        self.assertEqual(self.render("日\u0301\x1b[1;1Ha"), "a         ")
        self.assertEqual(self.render("日\u0301\x1b[1;2Ha"), " a        ")

    def test_snapshot_failure_still_closes_terminal_files_and_propagates(self):
        terminal = Terminal.__new__(Terminal)
        terminal.owned_daemon = None
        terminal.output = tempfile.TemporaryFile()
        terminal.fd = os.open(os.devnull, os.O_RDWR)
        self.addCleanup(terminal.output.close)
        terminal.process = SimpleNamespace(returncode=0, poll=lambda: 0)
        terminal.snapshot = Mock(side_effect=RuntimeError("broken screen"))
        terminal.send = Mock()
        terminal.pump = Mock()
        with self.assertRaisesRegex(RuntimeError, "broken screen"):
            terminal.close()
        self.assertTrue(terminal.output.closed)
        with self.assertRaises(OSError):
            os.fstat(terminal.fd)


if __name__ == "__main__":
    unittest.main()
