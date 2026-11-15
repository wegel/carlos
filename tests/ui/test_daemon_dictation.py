"""Exercise device dictation controls through the actual socket terminal without a microphone."""

import argparse
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from terminal import Terminal
from test_daemon_client import DaemonClient, daemon_support


class DaemonDictation(unittest.TestCase):
    daemon_session = DaemonClient.daemon_session
    attach_daemon = DaemonClient.attach_daemon

    def summary_root(self):
        root = ARTIFACTS / self._testMethodName
        root.mkdir(parents=True, exist_ok=True)
        return root

    def launch(self, directory, width, resume, extra_args, extra_env=None):
        terminal = Terminal(BINARY, directory, resume=resume, width=width,
                            extra_args=[*extra_args, "--dictation-profile", "fr"], extra_env=extra_env)
        self.addCleanup(terminal.close)
        return terminal

    def configured_session(self, name):
        empty = lambda: {"id":"native-root", "status":{"type":"idle"}, "model":"fixture-model",
                         "reasoningEffort":"low", "turns":[]}
        fixture, client, session = self.daemon_session(name, empty)
        path = self.summary_root() / "dictation/config/carlos/dictation.toml"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text('default_profile = "en"\n[profiles.en]\nname = "English"\nmodel = "missing-en.bin"\nlanguage = "en"\n[profiles.fr]\nname = "French"\nmodel = "missing-fr.bin"\nlanguage = "fr"\n')
        return fixture, client, session

    def assert_local_only(self, terminal):
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertEqual(terminal.requests("session/interrupt"), [])
        self.assertEqual(terminal.requests("turn/start"), [])
        self.assertEqual(terminal.requests("turn/interrupt"), [])

    def test_profiles_endpoint_and_missing_model_keep_the_draft_private(self):
        fixture, _client, session = self.configured_session("local-dictation")
        terminal = self.attach_daemon(fixture, session, "dictation")
        terminal.wait(lambda: terminal.placeholder_is("Shared terminal"), "socket composer ready")
        terminal.paste("private dictation draft")
        terminal.wait(lambda: terminal.input_is("private dictation draft"), "private draft before local keys")
        terminal.send(b"\x04")
        if WITHOUT_FEATURE:
            terminal.contains("dictation unavailable: rebuild with --features dictation")
        else:
            terminal.contains("dictation model unavailable: missing-fr.bin")
            terminal.send(b"\x1b[18~")
            terminal.contains("dictation profile: English")
            terminal.send(b"\x04")
            terminal.contains("dictation model unavailable: missing-en.bin")
            terminal.send(b"\x1b[20~")
            terminal.contains("dictation endpoint: manual (Enter to stop)")
            terminal.send(b"\x1b[20~")
            terminal.contains("dictation endpoint: auto")
        self.assertTrue(terminal.input_is("private dictation draft"))
        self.assert_local_only(terminal)
        self.assertEqual([request for request in fixture.native.requests if request.get("method") == "turn/start"], [])
        terminal.snapshot("device-dictation-keys-private")

    def test_busy_turn_guard_precedes_model_and_audio_access(self):
        fixture, client, session = self.configured_session("busy-dictation")

        async def running(link, request):
            await fixture.native.notify(link, "turn/started", turn={"id":"dictation-busy-turn"})
            await fixture.native.notify(link, "item/agentMessage/delta", turnId="dictation-busy-turn",
                                        itemId="busy-note", delta="dictation busy fixture")
        fixture.native.prompt = running
        operation = client.operation(2)
        client.call("session/send", session=session, operation=operation, text="start busy dictation fixture")
        client.outcome(operation)
        daemon_support.wait_for(lambda: client.call("session/subscribe", session=session, since=None)
                                ["snapshot"]["info"]["current_turn"] is not None, "busy turn published")
        terminal = self.attach_daemon(fixture, session, "dictation")
        terminal.contains("dictation busy fixture")
        terminal.paste("private while busy")
        terminal.wait(lambda: terminal.input_is("private while busy"), "private draft during busy turn")
        terminal.send(b"\x04")
        terminal.contains("dictation unavailable while turn is active")
        if WITHOUT_FEATURE:
            terminal.send(b"\x1b[18~")
            terminal.contains("dictation unavailable: rebuild with --features dictation")
            terminal.send(b"\x04")
            terminal.contains("dictation unavailable while turn is active")
            terminal.send(b"\x1b[20~")
            terminal.contains("dictation unavailable: rebuild with --features dictation")
        self.assertTrue(terminal.input_is("private while busy"))
        self.assert_local_only(terminal)
        starts = [request for request in fixture.native.requests if request.get("method") == "turn/start"]
        self.assertEqual(len(starts), 1, "local dictation keys cannot start another native turn")
        terminal.snapshot("dictation-refuses-busy-turn")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--daemon", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path)
    parser.add_argument("--without-feature", action="store_true")
    options = parser.parse_args()
    WITHOUT_FEATURE = options.without_feature
    ARTIFACTS = (options.artifacts or Path(tempfile.mkdtemp(prefix="carlos-dictation-ui-"))).resolve()
    ARTIFACTS.mkdir(parents=True, exist_ok=True)
    pair = ARTIFACTS / "executables"
    pair.mkdir()
    BINARY = pair / "carlos"
    BINARY.symlink_to(options.binary.resolve())
    (pair / "carlosd").symlink_to(options.daemon.resolve())
    def identity(path):
        return {"path": str(path.resolve()), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
    (ARTIFACTS / "identities.json").write_text(json.dumps({
        "terminal": identity(BINARY), "daemon": identity(options.daemon),
        "test": identity(Path(__file__)), "without_feature": WITHOUT_FEATURE}, indent=2) + "\n")
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(DaemonDictation)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    raise SystemExit(not result.wasSuccessful())
