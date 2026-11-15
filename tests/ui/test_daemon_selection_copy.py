"""Check copied rows against bytes captured from the old terminal or a supplied reference binary."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import tempfile
import unittest

from wcwidth import wcswidth

from terminal import Terminal
from test_daemon_client import DaemonClient


FIRST = "Ω begin é🙂"
LAST = "finish 界🙂"
ROWS = 240
DRAFT = "private selection draft"


def source_text():
    lines = []
    for index in range(ROWS):
        token = "TOKEN_" + "x" * 63 + "; " if index % 19 == 0 else ""
        lines.append(f"row-{index:03}  界 é🙂 words, spaced   words {token}wrap before tail-{index:03}  ")
    return "\n".join(lines) + "\n"


def history():
    return {"id":"native-root", "status":{"type":"idle"}, "model":"fixture-model",
            "reasoningEffort":"low", "turns":[{"id":"selection-turn", "status":"completed", "items":[
                {"type":"reasoning", "id":"inline-first", "summary":[{"text":FIRST}]},
                {"type":"commandExecution", "id":"paged-middle", "command":"printf selection",
                 "status":"completed", "exitCode":0, "aggregatedOutput":source_text()},
                {"type":"agentMessage", "id":"inline-last", "phase":"final_answer", "text":LAST}]}]}


class DaemonSelectionCopy(unittest.TestCase):
    daemon_session = DaemonClient.daemon_session
    attach_daemon = DaemonClient.attach_daemon

    def summary_root(self):
        root = ARTIFACTS / self._testMethodName / self.label
        root.mkdir(parents=True, exist_ok=True)
        return root

    def launch(self, directory, width, resume, extra_args, extra_env=None):
        terminal = Terminal(self.binary, directory, resume=resume, width=width,
                            extra_args=extra_args, extra_env=extra_env)
        self.addCleanup(terminal.close)
        return terminal

    def prepare(self, label, binary):
        self.label, self.binary = label, binary
        fixture, _client, session = self.daemon_session("selection-" + label, history)
        terminal = self.attach_daemon(fixture, session, "terminal", width=48, extra_env={
            "SSH_TTY":"", "SSH_CONNECTION":"", "SSH_CLIENT":""})
        terminal.contains(LAST)
        terminal.paste("/tools full")
        terminal.send(b"\r")
        settings = terminal.root / "config/carlos/ui-settings.json"

        def saved_full():
            try:
                return json.loads(settings.read_text()).get("tool_output") == "full" and terminal.input_is("")
            except (FileNotFoundError, json.JSONDecodeError):
                return False

        terminal.wait(saved_full, "private full-output preference saved and composer cleared")
        terminal.wait(lambda: f"row-{ROWS - 1:03}" in terminal.text and LAST in terminal.text,
                      "full source tail is painted before selecting", timeout=20)
        self.install_clipboard(terminal.root)
        return fixture, terminal

    @staticmethod
    def install_clipboard(root):
        program = root / "bin/wl-copy"
        program.write_text("""#!/usr/bin/env python3
import os,sys
from pathlib import Path
root=Path(os.environ['CARLOS_TEST_ROOT'])
target=root/'copy.tmp'
target.write_bytes(sys.stdin.buffer.read())
target.replace(root/'copied.bin')
""")
        program.chmod(0o755)

    @staticmethod
    def point(terminal, text, end=False):
        row = next(index for index, line in enumerate(terminal.screen.display) if text in line)
        line = terminal.screen.display[row]
        start = line.index(text)
        column = wcswidth(line[:start + len(text)]) if end else wcswidth(line[:start]) + 1
        return column, row + 1

    @staticmethod
    def mouse(terminal, button, point, release=False):
        column, row = point
        terminal.send(f"\x1b[<{button};{column};{row}{'m' if release else 'M'}".encode())

    def start_selection(self, terminal, point, reverse):
        self.mouse(terminal, 0, point)
        column, row = point
        moved = (column - 3 if reverse else column + 3, row)
        self.mouse(terminal, 32, moved)
        selected = min(point[0], moved[0]) - 1
        terminal.wait(lambda: terminal.screen.buffer[row - 1][selected].bg == "6c7086",
                      "horizontal drag chooses text selection before wheeling")

    @staticmethod
    def visible_rows(terminal):
        return {int(value) for value in re.findall(r"row-(\d{3})", terminal.text)}

    @staticmethod
    def frame_finished(terminal):
        # The renderer restores the composer cursor after flushing the complete frame.
        cursor = terminal.screen.cursor
        return (not cursor.hidden and cursor.y == terminal.screen.lines - 1
                and cursor.x == terminal.content_x + wcswidth(DRAFT))

    def wheel_to(self, terminal, target, reverse):
        windows = []
        for _ in range(400):
            if target in terminal.text:
                break
            previous = self.visible_rows(terminal)
            self.mouse(terminal, 64 if reverse else 65, (2, 2))
            terminal.wait(lambda: self.frame_finished(terminal)
                          and (target in terminal.text or self.visible_rows(terminal) != previous),
                          "wheel finishes painting another numbered source viewport")
            current = self.visible_rows(terminal)
            if current and sorted(current) not in windows:
                windows.append(sorted(current))
        self.assertIn(target, terminal.text)
        disjoint, covered = [], set()
        for window in windows:
            if not covered.intersection(window):
                disjoint.append(window)
                covered.update(window)
        self.assertGreaterEqual(len(disjoint), 3, "selection must cross three disjoint painted viewports")
        self.assertGreater(max(max(window) for window in windows) - min(min(window) for window in windows), 80)
        return windows

    def copied(self, terminal, name):
        target = terminal.root / "copied.bin"
        terminal.wait(target.exists, name, timeout=20)
        data = target.read_bytes()
        self.assertTrue(data)
        target.rename(terminal.root / (name + ".bin"))
        return data

    def select_and_copy(self, label, binary, reverse):
        fixture, terminal = self.prepare(label, binary)
        terminal.send(b"\x1b[F" if reverse else b"\x1b[H")
        anchor, focus = (LAST, "Ω begin") if reverse else ("Ω begin", LAST)
        edge = f"row-{ROWS - 1:03}" if reverse else "row-000"
        terminal.wait(lambda: anchor in terminal.text and focus not in terminal.text
                      and edge in terminal.text,
                      "Home or End paints the complete source edge before selecting", timeout=20)
        self.assertNotIn(focus, terminal.text)
        terminal.paste(DRAFT)
        terminal.wait(lambda: terminal.input_is(DRAFT), "private draft before selecting")
        self.start_selection(terminal, self.point(terminal, anchor, end=reverse), reverse)
        terminal.snapshot("selection-anchor")
        windows = self.wheel_to(terminal, focus, reverse)
        self.assertNotIn(anchor, terminal.text, "the selected anchor must leave the painted viewport")
        endpoint = self.point(terminal, focus, end=not reverse)
        self.mouse(terminal, 32, endpoint)
        self.mouse(terminal, 0, endpoint, release=True)
        automatic = self.copied(terminal, "mouse-copy")
        terminal.send(b"\x19")
        keyboard = self.copied(terminal, "keyboard-copy")
        self.assertEqual(keyboard, automatic, "CtrlY retains the completed selection")
        terminal.wait(lambda: terminal.input_is(DRAFT), "selection copy preserves the draft")
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertEqual(terminal.requests("session/interrupt"), [])
        self.assertEqual([request for request in fixture.native.requests if request.get("method") == "turn/start"], [])
        terminal.snapshot("selection-copied-after-scrolling")
        (terminal.root / "selection.json").write_text(json.dumps({
            "reverse": reverse, "viewports": windows, "bytes": len(keyboard),
            "sha256": hashlib.sha256(keyboard).hexdigest()}, indent=2) + "\n")
        return keyboard

    def compare(self, reverse):
        expected = (self.select_and_copy("reference", REFERENCE, reverse) if REFERENCE else
                    (Path(__file__).parent / "fixtures/daemon-selection-copy.txt").read_bytes())
        text = expected.decode()
        self.assertTrue(text.startswith(FIRST))
        self.assertTrue(text.endswith(LAST))
        for index in range(ROWS):
            self.assertIn(f"row-{index:03}", text, "old oracle must include every numbered logical line")
        self.assertIn("é🙂", text)
        actual = self.select_and_copy("candidate", BINARY, reverse)
        (self.summary_root() / "comparison.json").write_text(json.dumps({
            "expected_bytes": len(expected), "actual_bytes": len(actual),
            "expected_sha256": hashlib.sha256(expected).hexdigest(),
            "actual_sha256": hashlib.sha256(actual).hexdigest()}, indent=2) + "\n")
        self.assertEqual(actual, expected, "copy must preserve exact old whitespace and soft-wrap joins")

    def test_forward_selection_copies_inline_and_evicted_unicode_source_rows(self):
        self.compare(False)

    def test_reversed_selection_copies_the_same_complete_rows(self):
        self.compare(True)


def identity(path):
    return {"path": str(path.resolve()), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--reference-binary", type=Path)
    parser.add_argument("--daemon", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path)
    parser.add_argument("case", choices=("both", "forward", "reversed"), nargs="?", default="both")
    options = parser.parse_args()
    ARTIFACTS = (options.artifacts or Path(tempfile.mkdtemp(prefix="carlos-selection-ui-"))).resolve()
    ARTIFACTS.mkdir(parents=True, exist_ok=True)
    pair = ARTIFACTS / "executables"
    pair.mkdir()
    BINARY = pair / "carlos"
    BINARY.symlink_to(options.binary.resolve())
    (pair / "carlosd").symlink_to(options.daemon.resolve())
    REFERENCE = options.reference_binary.resolve() if options.reference_binary else None
    body = source_text().encode()
    assert 4096 < len(body) < 1024 * 1024
    (ARTIFACTS / "identities.json").write_text(json.dumps({
        "candidate": identity(BINARY), "reference": identity(REFERENCE) if REFERENCE else None,
        "daemon": identity(options.daemon), "test": identity(Path(__file__)),
        "source_bytes": len(body), "source_sha256": hashlib.sha256(body).hexdigest()}, indent=2) + "\n")
    names = ("test_forward_selection_copies_inline_and_evicted_unicode_source_rows",
             "test_reversed_selection_copies_the_same_complete_rows")
    suite = unittest.TestSuite(DaemonSelectionCopy(name) for name in names
                              if options.case == "both" or options.case in name)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    raise SystemExit(not result.wasSuccessful())
