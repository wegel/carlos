"""Keep native recovery distinct from transport reconnect and protect drafts during Codex rebuild."""

from test_daemon_client import daemon_support


class DaemonStartup:
    def test_daemon_native_f5_rebuilds_once_for_two_terminals_and_keeps_drafts_private(self):
        fixture, _, session = self.daemon_session("native-f5")
        fixture.native.saved_rows = []
        first = self.attach_daemon(fixture, session, "first")
        other = self.attach_daemon(fixture, session, "other")
        first.contains("history-output 9999")
        other.contains("history-output 9999")
        first.paste("draft after native reconnect 🦊")
        other.paste("other window keeps this 🌍")
        fixture.native.disconnect()
        first.contains("Codex offline")
        other.contains("Codex offline")
        fixture.native.saved_hold_resume = True
        first.send(b"\x1b[15~")
        first.wait(lambda: (fixture.root / "saved-resume-held").exists(), "exact native resume held")
        first.contains("Restoring Codex")
        other.contains("Restoring Codex")
        first.send(b"\r")
        first.contains("Waiting for the session to become ready")
        self.assertTrue(first.input_is("draft after native reconnect 🦊"))
        self.assertEqual(first.requests("session/send"), [])
        self.assertEqual(len(first.requests("session/reconnect")), 1)
        self.assertEqual(other.requests("session/reconnect"), [])
        fixture.native.release_saved("resume")
        first.contains("Codex reconnected")
        other.contains("Codex reconnected")
        first.wait(lambda: first.input_is("draft after native reconnect 🦊"), "first private composer after native recovery")
        other.wait(lambda: other.input_is("other window keeps this 🌍"), "other private composer after native recovery")
        first.send(b"\r")
        first.wait(lambda: len(first.requests("session/send")) == 1, "one explicit send after recovery")
        daemon_support.wait_for(lambda: fixture.native.prompts == 1, "one native prompt after recovery")
        self.assertEqual(fixture.native.resumed, 1)
        self.assertEqual(fixture.native.started, 1)
        self.assertEqual(len(first.requests("hello")), 1, "native F5 does not reconnect the device socket")
        self.assertEqual(len(other.requests("hello")), 1)
        self.assertEqual(other.requests("session/send"), [])
        first.snapshot("one-native-rebuild-first")
        other.snapshot("one-native-rebuild-other")
