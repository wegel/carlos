"""Reject unauthenticated work, unsafe browser upgrades and oversized WebSocket input."""

import json
import socket
import subprocess
import time

from websockets.exceptions import ConnectionClosed, InvalidHandshake
from websockets.sync.client import connect

import support
from network_support import NetworkClient, grant
from support import Fixture


class NetworkLimitTests:
    def test_default_daemon_has_no_network_listener_and_rejects_unspecified_or_multicast_bindings(self):
        fixture = Fixture("network-default")
        self.addCleanup(fixture.close)
        self.assertNotIn("network listening", (fixture.root / "daemon.log").read_text())
        for index, address in enumerate(("0.0.0.0:0", "[::]:0", "224.0.0.1:0")):
            root = fixture.root / str(index)
            root.mkdir(mode=0o700)
            result = subprocess.run([str(support.BINARY), "--socket", str(root / "socket"),
                                     "--state-dir", str(root / "state"), "--listen", address],
                                    env=fixture.env, capture_output=True, text=True, timeout=3)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("explicit unicast", result.stderr)
        self.assertEqual(fixture.pids(), [])

    def test_missing_wrong_and_unprovisioned_tokens_and_protocol_mismatch_start_no_native_work(self):
        fixture, local = self.network_fixture("bad-auth")
        token = grant(fixture, "phone")
        identity, secret = token.split(".")
        wrong = "0" * 64 if secret != "0" * 64 else "1" * 64
        for index, supplied in enumerate(("", identity + "." + wrong, "f" * 32 + "." + secret)):
            client = self.network_client(fixture, f"bad-{index}", greet=False)
            denied = client.response(client.send("hello", **NetworkClient.greeting("bad", supplied)))
            self.assertEqual(denied["error"]["code"], -32003)
            client.close()
        mismatch = self.network_client(fixture, "protocol", greet=False)
        params = NetworkClient.greeting("bad-protocol", token)
        params["protocol"] = 999
        denied = mismatch.response(mismatch.send("hello", **params))
        self.assertEqual(denied["error"]["code"], -32001)
        mismatch.close()
        good = self.network_client(fixture, "good", token)
        repeated = good.response(good.send("hello", **NetworkClient.greeting("again", token)))
        self.assertEqual(repeated["error"]["code"], -32002)
        self.assertEqual(local.call("sessions/watch"), {"sessions": []})
        self.assertEqual(fixture.pids(), [])

    def test_invalid_utf8_text_is_rejected_before_any_backend_dispatch(self):
        fixture, local = self.network_fixture("invalid-utf8")
        token = grant(fixture, "phone")
        for name, authenticated in [("before-auth", False), ("after-auth", True)]:
            client = self.network_client(fixture, name, token, greet=authenticated)
            with self.assertRaises(ConnectionClosed):
                client.socket.send(b"\xff", text=True)
                client.socket.recv(timeout=2)
            client.close()
        self.assertEqual(local.call("sessions/watch"), {"sessions": []})
        self.assertEqual(fixture.pids(), [])

    def test_network_token_is_unusable_until_its_held_catalogue_grant_becomes_durable(self):
        import sqlite3
        from support import wait_for

        fixture, local = self.network_fixture("grant-durability")
        database = sqlite3.connect(fixture.root / "state/sessions.sqlite3")
        self.addCleanup(database.close)
        database.execute("BEGIN IMMEDIATE")
        token_file = fixture.root / "pending.token"
        process = subprocess.Popen([str(support.BINARY), "device", "add", "--socket", str(fixture.socket),
                                    "--name", "pending", "--token-file", str(token_file)], env=fixture.env,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        def close_operator():
            if process.poll() is None:
                process.kill()
            process.communicate(timeout=3)
        self.addCleanup(close_operator)
        wait_for(lambda: token_file.exists() and token_file.stat().st_size == 98, "private token is written")
        token = token_file.read_text().strip()
        early = self.network_client(fixture, "early", greet=False)
        denied = early.response(early.send("hello", **NetworkClient.greeting("early", token)))
        self.assertEqual(denied["error"]["code"], -32003)
        early.close()
        self.assertEqual(local.call("device/list"), {"devices": []})
        database.rollback()
        stdout, stderr = process.communicate(timeout=3)
        self.assertEqual(process.returncode, 0, stderr)
        self.assertNotIn(token.split(".")[1], stdout + stderr)
        ready = self.network_client(fixture, "durable", token)
        self.assertTrue(ready.hello["client"])
        self.assertEqual(fixture.pids(), [])

    def test_origin_is_exact_and_url_credentials_subprotocols_and_large_headers_are_refused(self):
        origin = "https://phone.example"
        fixture, _ = self.network_fixture("origins", origin=origin)
        token = grant(fixture, "phone")
        good = self.network_client(fixture, "browser", token, origin=origin)
        self.assertTrue(good.hello["client"])
        attempts = [(fixture.ws_uri, {"origin": "https://elsewhere.example"}),
                    (fixture.ws_uri, {"origin": "null"}),
                    (fixture.ws_uri, {"origin": origin, "additional_headers": {"Origin": origin}}),
                    (fixture.ws_uri + "?token=" + token, {}),
                    (fixture.ws_uri, {"subprotocols": [token]}),
                    (fixture.ws_uri, {"additional_headers": {"X-Fill": "x" * 9000}})]
        for uri, options in attempts:
            with self.assertRaises((InvalidHandshake, ConnectionClosed, OSError)):
                with connect(uri, compression=None, proxy=None, ping_interval=None, open_timeout=2, **options):
                    self.fail("unsafe WebSocket upgrade succeeded")
        with connect(fixture.ws_uri, origin=origin, compression="deflate", proxy=None, ping_interval=None) as offered:
            self.assertIsNone(offered.response.headers.get("Sec-WebSocket-Extensions"))
            offered.send(json.dumps({"jsonrpc": "2.0", "id": 1, "method": "hello", "params": NetworkClient.greeting("offer", token)}))
            self.assertIn("result", json.loads(offered.recv(timeout=2)))
        self.assertNotIn(token, (fixture.root / "daemon.log").read_text())
        self.assertEqual(fixture.pids(), [])

    def test_first_hello_frames_are_bounded_and_binary_unknown_and_malformed_input_never_launch(self):
        fixture, local = self.network_fixture("frames")
        token = grant(fixture, "phone")
        payloads = ["{", b"{}", json.dumps({"jsonrpc": "2.0", "id": 1, "method": "sessions/watch", "params": {}}),
                    ["x" * 2048, "y" * 2049]]
        for index, payload in enumerate(payloads):
            client = self.network_client(fixture, f"frame-{index}", greet=False)
            with self.assertRaises(ConnectionClosed):
                client.socket.send(payload)
                client.socket.recv(timeout=2)
            client.close()
        good = self.network_client(fixture, "valid-fragments", greet=False)
        message = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "hello", "params": NetworkClient.greeting("fragments", token)})
        good.socket.send([message[:17], message[17:]])
        good.hello = good.response(1)["result"]
        good.serial = 1
        pong = good.socket.ping(b"bounded-ping")
        self.assertTrue(pong.wait(timeout=2))
        good.socket.send('{"jsonrpc":"2.0","id":2,"method":"not-a-method","params":{}}')
        self.assertEqual(good.response(2)["error"]["code"], -32601)
        good.socket.send("{")
        self.assertEqual(good.receive()["error"]["code"], -32700)
        oversized = self.network_client(fixture, "oversized", token)
        with self.assertRaises(ConnectionClosed):
            oversized.socket.send(["x" * (2 * 1024 * 1024), "x" * (2 * 1024 * 1024 + 1)])
            oversized.socket.recv(timeout=2)
        self.assertEqual(local.call("sessions/watch"), {"sessions": []})
        self.assertEqual(fixture.pids(), [])

    def test_unfinished_upgrades_keep_fixed_deadlines_and_cannot_exhaust_local_operator_access(self):
        fixture, local = self.network_fixture("handshake-reserve")
        token = grant(fixture, "phone")
        host, port = fixture.ws_uri.removeprefix("ws://").split(":")
        sockets = []
        for _ in range(4):
            stream = socket.create_connection((host, int(port)), timeout=2)
            stream.sendall(b"GET / HTTP/1.1\r\nX-Held: ")
            sockets.append(stream)
            self.addCleanup(stream.close)
        began = time.monotonic()
        self.assertEqual(local.call("device/list")["devices"][0]["id"], token.split(".")[0])
        extra = socket.create_connection((host, int(port)), timeout=2)
        self.addCleanup(extra.close)
        self.assertEqual(extra.recv(1), b"", "fifth unauthenticated connection is refused")
        sockets[0].settimeout(6)
        self.assertEqual(sockets[0].recv(1), b"")
        self.assertLess(time.monotonic() - began, 6)
        for stream in sockets:
            stream.close()
        good = self.network_client(fixture, "after-deadline", token)
        self.assertTrue(good.hello["client"])
        self.assertEqual(fixture.pids(), [])

    def test_full_network_peer_capacity_preserves_local_operator_greeting_and_revocation(self):
        fixture, local = self.network_fixture("peer-reserve")
        token = grant(fixture, "phone")
        clients = [self.network_client(fixture, f"network-{index}", token) for index in range(16)]
        self.assertEqual(len({client.hello["client"] for client in clients}), 16)
        with self.assertRaises((InvalidHandshake, ConnectionClosed, OSError)):
            self.network_client(fixture, "network-17", token)
        new_local = support.Client(fixture, "new-local-operator")
        self.addCleanup(new_local.close)
        operation = new_local.operation(1)
        new_local.call("device/revoke", operation=operation, device=token.split(".")[0])
        self.assertEqual(new_local.outcome(operation)["state"]["kind"], "accepted")
        self.assertEqual(local.call("device/list"), {"devices": []})
        self.assertEqual(fixture.pids(), [])
