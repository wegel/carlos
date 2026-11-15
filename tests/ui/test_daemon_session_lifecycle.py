"""Drive explicit lifecycle commands through real terminals and the capacity-full operator CLI."""

import sqlite3
import subprocess
import sys

from test_daemon_client import daemon_support


class SessionLifecycle:
    def test_stop_and_reopen_keep_two_composers_and_wait_for_an_explicit_send(self):
        fixture, observer, session = self.daemon_session("terminal-session-stop")
        fixture.native.saved_rows = []
        first = self.attach_daemon(fixture, session, "lifecycle-first")
        second = self.attach_daemon(fixture, session, "lifecycle-second")
        first.contains("history-output 9999")
        second.contains("history-output 9999")
        second.paste("other terminal keeps its draft 🌍")
        database = sqlite3.connect(fixture.root / "state/sessions.sqlite3")
        self.addCleanup(database.close)
        database.execute("BEGIN IMMEDIATE")
        first.paste("/session stop")
        first.send(b"\r")
        first.wait(lambda: len(first.requests("session/stop")) == 1, "typed stop reached the daemon")
        first.paste("newer draft while stop waits 🦊")
        first.wait(lambda: first.input_is("newer draft while stop waits 🦊"), "newer draft remains editable")
        database.rollback()
        first.contains("Session stopped")
        second.contains("Codex offline")
        self.assertTrue(first.input_is("newer draft while stop waits 🦊"))
        self.assertTrue(second.input_is("other terminal keeps its draft 🌍"))
        self.assertEqual(first.requests("session/send"), [])
        self.assertEqual(second.requests("session/send"), [])
        fixture.native.saved_hold_resume = True
        first.replace_input("/session reopen")
        first.send(b"\r")
        first.wait(lambda: (fixture.root / "saved-resume-held").exists(), "explicit native restore waits")
        first.paste("newer draft while reopen waits 🐙")
        fixture.native.release_saved("resume")
        first.contains("Session reopened")
        self.assertTrue(first.input_is("newer draft while reopen waits 🐙"))
        self.assertTrue(second.input_is("other terminal keeps its draft 🌍"))
        self.assertEqual(len(first.requests("session/reopen")), 1)
        self.assertEqual(first.requests("session/send"), [])
        self.assertEqual(fixture.native.started, 1)
        self.assertEqual(fixture.native.resumed, 1)
        first.send(b"\r")
        first.contains("Answer: newer draft while reopen waits 🐙")
        second.contains("Answer: newer draft while reopen waits 🐙")
        self.assertEqual(len(first.requests("session/send")), 1)
        self.assertEqual(second.requests("session/send"), [])
        first.snapshot("explicit-stop-reopen-newer-draft")
        second.snapshot("same-conversation-independent-composer")

    def test_cli_stop_can_free_a_full_owner_slot_without_opening_a_native_session(self):
        runner = sys.modules[type(self).__module__]
        daemon_support.BINARY = runner.BINARY.parent / "carlosd"
        daemon_support.ARTIFACTS = runner.ARTIFACTS / "daemon"
        fixture = daemon_support.Fixture("cli-full-owner-slots")
        self.addCleanup(fixture.close)
        client = daemon_support.Client(fixture, "capacity-owner")
        self.addCleanup(client.close)
        sessions = []
        for counter in range(1, 17):
            cwd = fixture.root / str(counter)
            cwd.mkdir()
            operation = client.operation(counter)
            session = client.call("session/create", backend="grok", cwd=str(cwd), name=None, operation=operation)["session"]
            self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
            sessions.append(session)
        pid = fixture.pids()[0]["pid"]
        stopped = subprocess.run([str(runner.BINARY), "session", "stop", sessions[0], "--daemon-socket", str(fixture.socket)],
                                 env=fixture.env, capture_output=True, text=True, timeout=8)
        (self.summary_root() / "cli-stop.txt").write_text(stopped.stdout + stopped.stderr)
        self.assertEqual(stopped.returncode, 0, stopped.stderr)
        self.assertIn(sessions[0], stopped.stdout)
        self.assertIn("Session stopped", stopped.stdout)
        self.assertFalse(daemon_support.alive(pid))
        self.assertEqual(len(fixture.pids()), 16, "CLI stop cannot create an owner to reach its command")
        operation = client.operation(17)
        added = client.call("session/create", backend="grok", cwd=str(fixture.root), name=None, operation=operation)
        self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
        self.assertNotEqual(added["session"], sessions[0])
        stop_other = client.operation(18)
        client.call("session/stop", session=added["session"], operation=stop_other)
        client.outcome(stop_other)
        (fixture.root / "release-history").write_text("go")
        reopened = subprocess.run([str(runner.BINARY), "--daemon-socket", str(fixture.socket), "session", "reopen", sessions[0]],
                                  env=fixture.env, capture_output=True, text=True, timeout=8)
        (self.summary_root() / "cli-reopen.txt").write_text(reopened.stdout + reopened.stderr)
        self.assertEqual(reopened.returncode, 0, reopened.stderr)
        self.assertIn(sessions[0], reopened.stdout)
        self.assertEqual(fixture.native_requests("session/load")[-1]["params"]["sessionId"], "grok-saved")

    def test_cli_rejects_a_missing_logical_session_without_creating_native_work(self):
        runner = sys.modules[type(self).__module__]
        daemon_support.BINARY = runner.BINARY.parent / "carlosd"
        daemon_support.ARTIFACTS = runner.ARTIFACTS / "daemon"
        fixture = daemon_support.Fixture("cli-missing-session")
        self.addCleanup(fixture.close)
        result = subprocess.run([str(runner.BINARY), "session", "stop", "a" * 32, "--daemon-socket", str(fixture.socket)],
                                env=fixture.env, capture_output=True, text=True, timeout=8)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Unknown session", result.stderr)
        self.assertEqual(fixture.pids(), [])

    def test_cli_managed_route_uses_the_existing_daemon_without_a_native_launch(self):
        runner = sys.modules[type(self).__module__]
        daemon_support.BINARY = runner.BINARY.parent / "carlosd"
        daemon_support.ARTIFACTS = runner.ARTIFACTS / "daemon"
        fixture = daemon_support.Fixture("cli-managed-route")
        self.addCleanup(fixture.close)
        fixture.stop()
        runtime = fixture.root / "operator-runtime"
        runtime.mkdir(mode=0o700)
        (runtime / "carlos").mkdir(mode=0o700)
        fixture.env["XDG_RUNTIME_DIR"] = str(runtime)
        fixture.socket = runtime / "carlos/daemon.sock"
        fixture.start()
        client = daemon_support.Client(fixture, "managed-owner")
        self.addCleanup(client.close)
        create = client.operation(1)
        session = client.call("session/create", backend="grok", cwd=str(fixture.root), name=None, operation=create)["session"]
        client.outcome(create)
        daemon_pid = fixture.process.pid
        native_pid = fixture.pids()[0]["pid"]
        result = subprocess.run([str(runner.BINARY), "session", "stop", session], env=fixture.env,
                                capture_output=True, text=True, timeout=8)
        (self.summary_root() / "cli-managed.txt").write_text(result.stdout + result.stderr)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(fixture.process.pid, daemon_pid)
        self.assertIsNone(fixture.process.poll())
        self.assertEqual(len(fixture.pids()), 1)
        self.assertFalse(daemon_support.alive(native_pid))
