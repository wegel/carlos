"""Keep a configured phone listener and its device grant through managed restarts."""

import json
import os
from pathlib import Path
import signal
import subprocess
from types import SimpleNamespace

from network_support import NetworkClient


class LauncherNetworkTests:
    def test_saved_phone_listener_and_device_grant_survive_managed_restart(self):
        host = self.host()
        self.write_network_settings(host, {"listen":"127.0.0.1:0"})
        first = host.terminal("first")
        first.contains("Launcher fixture ready")
        local = host.client()
        self.addCleanup(local.close)
        session = local.call("sessions/watch")["sessions"][0]["id"]
        token_file = host.host / "phone.token"
        subprocess.run([str(host.bundle / "carlosd"), "device", "add", "--socket", str(host.socket),
                        "--name", "fixture-phone", "--token-file", str(token_file)],
                       check=True, capture_output=True, timeout=8)
        token = token_file.read_text().strip()
        endpoint = self.network_endpoint(host)
        rejected = NetworkClient(endpoint, "unpaired", greet=False)
        self.addCleanup(rejected.close)
        denied = rejected.response(rejected.send("hello", **NetworkClient.greeting("unpaired", "")))
        self.assertEqual(denied["error"]["code"], -32003)
        phone = NetworkClient(endpoint, "paired", token)
        self.assertEqual(phone.call("sessions/watch")["sessions"][0]["id"], session)
        phone.close()
        first.close()
        host.capture()
        pid = local.hello["identity"]["process"]
        self.assertIn(pid, host.daemon_ids())
        local.close()
        os.kill(pid, signal.SIGTERM)
        from daemon_launcher_fixture import daemon_support
        daemon_support.wait_for(lambda:not host.socket.exists(), "first configured daemon stopped")
        second = host.terminal("second")
        second.contains("Launcher fixture ready")
        restored = NetworkClient(self.network_endpoint(host), "same-phone", token)
        self.addCleanup(restored.close)
        self.assertNotEqual(restored.hello["instance"], phone.hello["instance"])
        self.assertEqual(restored.call("sessions/watch")["sessions"][0]["id"], session)
        self.assertEqual(host.native.resumed, 2)
        self.assertFalse(any(row.get("method") == "turn/start" for row in host.native.requests))
        second.snapshot("saved-phone-listener-after-managed-restart")

    def test_invalid_saved_listener_fails_before_starting_native_work(self):
        host = self.host()
        self.write_network_settings(host, {"listne":"127.0.0.1:0"})
        host.failed(host.terminal("invalid-settings", expected=1), "daemon exited during startup")
        log = Path(host.env["XDG_STATE_HOME"]) / "carlos/daemon/daemon.log"
        self.assertIn("unknown field", log.read_text())
        self.assertFalse(host.socket.exists())
        self.assertEqual(host.native.requests, [])

    @staticmethod
    def write_network_settings(host, settings):
        path = Path(host.env["XDG_CONFIG_HOME"]) / "carlos/daemon.json"
        path.parent.mkdir(parents=True, mode=0o700)
        path.write_text(json.dumps(settings))
        path.chmod(0o600)

    @staticmethod
    def network_endpoint(host):
        log = Path(host.env["XDG_STATE_HOME"]) / "carlos/daemon/daemon.log"
        prefix = "carlosd: network listening on "
        address = [line.removeprefix(prefix) for line in log.read_text().splitlines()
                   if line.startswith(prefix)][-1]
        return SimpleNamespace(root=host.root, ws_uri=address)
