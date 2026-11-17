"""Exercise managed startup, detach and explicit failures through real terminal processes."""

import argparse
import json
from pathlib import Path
import subprocess
import time
import unittest

from daemon_launcher_fixture import GreetingProxy, Host, daemon_support, subreaper
from test_daemon_launcher_network import LauncherNetworkTests


class LauncherTests(LauncherNetworkTests, unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.previous_subreaper = subreaper(True)

    @classmethod
    def tearDownClass(cls):
        subreaper(cls.previous_subreaper)

    def host(self, companion=True):
        host = Host(BINARY, DAEMON, ARTIFACTS, self._testMethodName, companion)
        self.addCleanup(host.close)
        return host

    def test_concurrent_starts_share_one_owner_and_terminal_exit_only_detaches(self):
        host = self.host()
        first, second = host.terminal("first"), host.terminal("second")
        for terminal in (first, second):
            terminal.contains("Launcher fixture ready")
        self.assertEqual(len(host.daemon_ids()), 1)
        client = host.client()
        self.addCleanup(client.close)
        original = client.hello["instance"]
        sessions = client.call("sessions/watch")["sessions"]
        self.assertEqual(len(sessions), 1)
        self.assertIn(sessions[0]["cwd"], {str(first.root), str(second.root)})
        self.assertEqual(host.native.resumed, 1)
        for terminal, draft in ((first, "first private 🌍"), (second, "second private 日本語")):
            terminal.paste(draft)
            terminal.wait(lambda:terminal.input_is(draft), "private managed-device draft")
        self.assertNotIn("second private", first.text)
        self.assertNotIn("first private", second.text)
        first.close()
        second.close()
        self.assertEqual(client.call("sessions/watch")["sessions"][0]["availability"], "ready")
        attached = host.terminal("reattach", resume=sessions[0]["id"],
                                 arguments=["--daemon-socket", str(host.socket)])
        attached.contains("Launcher fixture ready")
        other = host.client()
        self.addCleanup(other.close)
        self.assertEqual(other.hello["instance"], original)
        self.assertEqual(host.native.resumed, 1)
        self.assertFalse(any(row.get("method") == "turn/start" for row in host.native.requests))
        (host.root / "identity.json").write_text(json.dumps(client.hello, indent=2) + "\n")

    def test_new_native_session_uses_the_terminal_cwd_instead_of_the_daemon_state_directory(self):
        host = self.host()
        terminal = host.terminal("new", resume=None)
        terminal.contains("Launcher fixture ready")
        starts = [row for row in host.native.requests if row.get("method") == "thread/start"]
        self.assertEqual(len(starts), 1)
        self.assertEqual(starts[0]["params"]["cwd"], str(terminal.root))
        client = host.client()
        self.addCleanup(client.close)
        self.assertEqual(client.call("sessions/watch")["sessions"][0]["cwd"], str(terminal.root))

    def test_missing_companion_returns_a_specific_error_without_starting_native_work(self):
        host = self.host(companion=False)
        host.failed(host.terminal("missing", expected=1), "matching daemon binary is missing")
        self.assertFalse(host.socket.exists())
        self.assertEqual(host.native.requests, [])

    def test_incompatible_companion_returns_before_daemon_or_native_start(self):
        host = self.host()
        identity = {**host.identity(), "source":"0" * 32}
        host.fake_companion("import json\nprint(json.dumps(" + repr(identity) + "))\n")
        host.failed(host.terminal("mismatch", expected=1), "companion carlosd build differs")
        self.assertFalse(host.socket.exists())
        self.assertEqual(host.native.requests, [])

    def test_explicit_native_endpoint_conflict_preserves_the_running_owner(self):
        host = self.host()
        first = host.terminal("original")
        first.contains("Launcher fixture ready")
        client = host.client()
        self.addCleanup(client.close)
        instance = client.hello["instance"]
        second = host.terminal("conflict", expected=1,
            arguments=["--daemon", "--codex-socket", str(host.host / "different-native.sock")])
        host.failed(second, "different native Codex endpoint")
        self.assertEqual(len(host.daemon_ids()), 1)
        self.assertEqual(host.native.resumed, 1)
        self.assertEqual(client.call("sessions/watch")["sessions"][0]["availability"], "ready")
        self.assertEqual(instance, client.hello["instance"])

    def test_daemon_start_deadline_kills_only_the_new_stuck_child(self):
        host = self.host()
        identity = host.identity()
        host.fake_companion("import json,sys,time\nif sys.argv[1:] == ['--build-info']:\n"
            " print(json.dumps(" + repr(identity) + "))\nelse:\n time.sleep(60)\n")
        started = time.monotonic()
        host.failed(host.terminal("stuck", expected=1), "did not start within 5 seconds")
        self.assertLess(time.monotonic() - started, 6.5)
        self.assertEqual(host.daemon_ids(), set())
        self.assertEqual(host.native.requests, [])

    def test_explicit_daemon_socket_never_looks_for_or_spawns_a_companion(self):
        host = self.host(companion=False)
        daemon = self.explicit_daemon(host, host.host / "explicit.sock")
        terminal = host.terminal("explicit", arguments=["--daemon-socket", str(host.host / "explicit.sock")])
        terminal.contains("Launcher fixture ready")
        terminal.close()
        self.assertIsNone(daemon.poll())
        self.assertFalse(host.socket.exists())
        self.assertEqual(host.native.resumed, 1)

    def test_incompatible_running_greeting_does_not_stop_the_existing_daemon(self):
        host = self.host(companion=False)
        upstream = host.host / "upstream.sock"
        daemon = self.explicit_daemon(host, upstream)
        client = host.client(upstream)
        self.addCleanup(client.close)
        proxy = GreetingProxy(host.socket, upstream, host.root / "incompatible-greeting.json")
        self.addCleanup(proxy.close)
        terminal = host.terminal("incompatible-live", expected=1)
        host.failed(terminal, "running Carlos daemon build differs")
        self.assertIn(str(daemon.pid), terminal.text)
        self.assertIsNone(daemon.poll())
        other = host.client(upstream)
        self.addCleanup(other.close)
        self.assertEqual(client.hello["instance"], other.hello["instance"])
        self.assertEqual(host.native.requests, [])

    def test_explicit_attach_accepts_the_daemons_same_native_socket_symlink(self):
        host = self.host(companion=False)
        alias = host.host / "native-alias.sock"
        alias.symlink_to(host.native.path)
        self.explicit_daemon(host, host.host / "explicit.sock", alias)
        terminal = host.terminal("explicit-alias", arguments=[
            "--daemon-socket", str(host.host / "explicit.sock"), "--codex-socket", str(alias)])
        terminal.contains("Launcher fixture ready")
        self.assertEqual(host.native.resumed, 1)
        self.assertFalse(host.socket.exists())

    def explicit_daemon(self, host, socket, native=None):
        log = (host.root / "explicit-daemon.log").open("wb")
        daemon = subprocess.Popen([str(DAEMON), "--socket", str(socket),
            "--state-dir", str(host.host / "explicit-state"), "--codex-socket", str(native or host.native.path)],
            stdout=log, stderr=log)
        self.addCleanup(log.close)
        self.addCleanup(lambda:self.stop(daemon))
        daemon_support.wait_for(socket.exists, "explicit daemon listens")
        return daemon

    @staticmethod
    def stop(process):
        if process.poll() is None:
            process.terminate()
        process.wait(timeout=5)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--daemon", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path, required=True)
    options, remaining = parser.parse_known_args()
    BINARY, DAEMON, ARTIFACTS = options.binary.resolve(), options.daemon.resolve(), options.artifacts.resolve()
    unittest.main(argv=[__file__, *remaining])
