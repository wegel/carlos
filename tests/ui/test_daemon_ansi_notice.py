"""Replay an ANSI native error through the actual daemon and lazy terminal cache."""

import argparse
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from terminal import Terminal


class DaemonAnsiNotice(unittest.TestCase):
    def summary_root(self):
        root = ARTIFACTS / self._testMethodName
        root.mkdir(parents=True, exist_ok=True)
        return root

    def test_ansi_native_failure_keeps_lazy_rows_and_private_input_exact(self):
        root = self.summary_root() / "notice"
        controller = Path(__file__).with_name("ansi_notice_controller.py").resolve()
        terminal = Terminal(BINARY, root, backend="claude", resume=None, width=100,
                            extra_env={"CARLOS_CLAUDE_CONTROLLER":str(controller)})
        self.addCleanup(terminal.close)
        terminal.contains("ANSI notice END")
        terminal.paste("private error draft")
        terminal.wait(lambda:terminal.input_is("private error draft"), "private draft after native failure")
        for width in (28, 72, 100):
            terminal.resize(width, 32)
            terminal.wait(lambda:"ANSI notice END" in terminal.text
                          and terminal.input_is("private error draft"),
                          "resized frame retains its notice and private composer")
            terminal.snapshot(f"ansi-error-{width}")
        self.assertEqual(terminal.daemon_requests("session/send"), [])
        self.assertEqual(terminal.daemon_requests("session/rename"), [])
        notices = []
        for row in terminal.daemon_messages("server"):
            result = row.get("result")
            if isinstance(result, dict):
                notices.extend(result.get("snapshot", {}).get("entries", []))
            params = row.get("params", {})
            event = params.get("event", {})
            notices.extend(params.get("snapshot", {}).get("entries", []))
            notices.extend(event.get("snapshot", {}).get("entries", []))
            if event.get("kind") == "entry_added":
                notices.append(event["entry"])
        self.assertTrue(any("\x1b" in entry["text"] for entry in notices), "wire retained the raw ANSI bytes")


def identity(path):
    return {"path":str(path.resolve()), "sha256":hashlib.sha256(path.read_bytes()).hexdigest()}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--daemon", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path)
    args, tests = parser.parse_known_args()
    ARTIFACTS = (args.artifacts or Path(tempfile.mkdtemp(prefix="carlos-ansi-notice-"))).resolve()
    ARTIFACTS.mkdir(parents=True, exist_ok=True)
    pair = ARTIFACTS / "executables"
    pair.mkdir()
    Terminal.daemon_binary = args.daemon.resolve()
    BINARY = pair / "carlos"
    BINARY.symlink_to(args.binary.resolve())
    (pair / "carlosd").symlink_to(args.daemon.resolve())
    (ARTIFACTS / "identities.json").write_text(json.dumps({
        "terminal":identity(BINARY), "daemon":identity(args.daemon),
        "test":identity(Path(__file__))}, indent=2) + "\n")
    unittest.main(argv=[__file__, *tests])
