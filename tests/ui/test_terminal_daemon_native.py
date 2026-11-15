"""Reap actual daemon/native adapter processes after the test's client has detached."""

import argparse
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest

from terminal import Terminal
from terminal_daemon_processes import capture_children, remaining
from test_terminal_daemon import CLIENT

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "daemon"))
from support import Client, wait_for


class NativeDaemonOwnership(unittest.TestCase):
    def test_detach_preserves_all_native_adapters_until_private_fixture_shutdown_reaps_them(self):
        self.root = Path(tempfile.mkdtemp(prefix="carlos-ui-daemon-native-", dir=ARTIFACTS))
        program = self.root / "fixture-client"
        program.write_text(CLIENT)
        program.chmod(0o755)
        previous = Terminal.daemon_binary
        self.addCleanup(setattr, Terminal, "daemon_binary", previous)
        Terminal.daemon_binary = DAEMON
        terminal = Terminal(program, self.root, backend="grok", resume=None)
        self.addCleanup(terminal.close)
        owner = terminal.owned_daemon
        fixture = SimpleNamespace(root=self.root, socket=owner.socket, memory=lambda _:None)
        client = Client(fixture, "native-ownership")
        self.addCleanup(client.close)
        for counter, backend in enumerate(("codex", "claude", "grok"), 1):
            operation = client.operation(counter)
            session = client.call("session/create", operation=operation, backend=backend,
                                  cwd=str(self.root), name="Owned " + backend)["session"]
            self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
            wait_for(lambda:client.call("session/subscribe", session=session, since=None)
                     ["snapshot"]["info"]["availability"] == "ready", backend + " native ready")
        children = capture_children(owner.process.pid)
        self.assertGreaterEqual(len(children), 4, "Codex, Grok, Claude helper and Claude native child")
        terminal.close_process()
        self.assertIsNone(owner.process.poll(), "terminal detach leaves the daemon alive")
        self.assertEqual(remaining(children), children, "terminal detach leaves every native process alive")
        client.close()
        terminal.close()
        self.assertEqual(owner.process.returncode, 0)
        self.assertEqual(remaining(children), {}, "private fixture shutdown reaps actual native descendants")
        self.assertTrue(terminal.requests("thread/start"), "original native RPC log remains readable")
        self.assertTrue((self.root/"claude-controller.jsonl").exists())
        (self.root/"owned-pids.txt").write_text("\n".join(map(str, sorted(children)))+"\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--daemon-binary", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path, required=True)
    options, arguments = parser.parse_known_args()
    DAEMON = options.daemon_binary.resolve()
    ARTIFACTS = options.artifacts.resolve()
    ARTIFACTS.mkdir(parents=True, exist_ok=True)
    unittest.main(argv=[__file__, *arguments])
