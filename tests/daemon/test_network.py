"""Authenticate real network peers against the same native session owners and private retry store."""

import time

from websockets.exceptions import ConnectionClosed

from network_support import NetworkClient, grant
from support import Client, Fixture, alive


class NetworkTests:
    def network_fixture(self, name, **options):
        fixture = Fixture("network-" + name, listen="127.0.0.1:0", **options)
        self.addCleanup(fixture.close)
        local = Client(fixture, "local-operator")
        self.addCleanup(local.close)
        return fixture, local

    def network_client(self, fixture, name, token=None, **options):
        client = NetworkClient(fixture, name, token, **options)
        self.addCleanup(client.close)
        return client

    def network_create(self, fixture, client, counter=1):
        operation = client.operation(counter)
        session = client.call("session/create", operation=operation, backend="grok",
                              cwd=str(fixture.root), name="Network conversation")["session"]
        self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
        return session

    def test_authenticated_websocket_and_unix_clients_share_one_native_owner_and_private_results(self):
        fixture, local = self.network_fixture("shared")
        token = grant(fixture, "phone")
        first = self.network_client(fixture, "phone-one", token)
        second = self.network_client(fixture, "phone-two", token)
        session = self.network_create(fixture, first)
        for client in (local, first, second):
            snapshot = client.call("session/subscribe", session=session, since=None)["snapshot"]
            self.assertEqual(snapshot["info"]["id"], session)
            self.assertEqual(snapshot["info"]["availability"], "ready")
        operation = first.operation(2)
        first.call("session/send", session=session, operation=operation, text="network prompt 🦊")
        self.assertEqual(first.outcome(operation)["state"]["kind"], "accepted")
        denied = second.response(second.send("operation/read", operation=operation))
        self.assertEqual(denied["error"]["code"], -32003)
        self.assertEqual(len(fixture.native_requests("session/new")), 1)
        self.assertEqual(len(fixture.pids()), 1)
        self.assertNotIn(token, (fixture.root / "daemon.log").read_text())

    def test_network_devices_cannot_call_operator_methods_or_borrow_other_device_or_unix_leases(self):
        fixture, local = self.network_fixture("scoped")
        first_token, second_token = grant(fixture, "first"), grant(fixture, "second")
        first = self.network_client(fixture, "first-client", first_token)
        session = self.network_create(fixture, first)
        attacker = self.network_client(fixture, "borrow", greet=False)
        response = attacker.response(attacker.send("hello", **NetworkClient.greeting("borrow", second_token, first.hello)))
        self.assertEqual(response["error"]["code"], -32003)
        second = self.network_client(fixture, "second-client", second_token)
        calls = [("device/list", {}),
                 ("device/revoke", {"operation": second.operation(1), "device": first_token.split(".")[0]}),
                 ("device/add", {"operation": second.operation(2), "device": {"id": "f" * 32, "name": "forged"}, "verifier": "a" * 64})]
        for method, params in calls:
            denied = second.response(second.send(method, **params))
            self.assertEqual(denied["error"]["code"], -32003)
        unix = Client(fixture, "unix-borrow", greet=False)
        self.addCleanup(unix.close)
        denied = unix.response(unix.send("hello", protocol=1, client="unix-borrow", resume=first.hello["client"],
                                          resume_token=first.hello["resume_token"], instance=first.hello["instance"]))
        self.assertEqual(denied["error"]["code"], -32003)
        first.close()
        resumed = self.network_client(fixture, "same-device", first_token, resume=first.hello)
        self.assertTrue(resumed.hello["resumed"])
        self.assertEqual(resumed.call("session/create", operation=resumed.operation(1), backend="grok",
                                      cwd=str(fixture.root), name="Network conversation")["session"], session)
        self.assertEqual(len(local.call("device/list")["devices"]), 2)
        self.assertEqual(len(fixture.pids()), 1)

    def test_revocation_closes_only_its_device_and_keeps_the_native_owner_and_other_clients_alive(self):
        fixture, local = self.network_fixture("revoke")
        first_token, other_token = grant(fixture, "revoked"), grant(fixture, "bystander")
        first = self.network_client(fixture, "first-socket", first_token)
        second = self.network_client(fixture, "second-socket", first_token)
        other = self.network_client(fixture, "other-device", other_token)
        session = self.network_create(fixture, first)
        operation = local.operation(1)
        local.call("device/revoke", operation=operation, device=first_token.split(".")[0])
        self.assertEqual(local.outcome(operation)["state"]["kind"], "accepted")
        for client in (first, second):
            deadline = time.monotonic() + 3
            with self.assertRaises(ConnectionClosed):
                while time.monotonic() < deadline:
                    client.receive(timeout=1)
        rejected = self.network_client(fixture, "revoked-token", greet=False)
        denied = rejected.response(rejected.send("hello", **NetworkClient.greeting("revoked", first_token, first.hello)))
        self.assertEqual(denied["error"]["code"], -32003)
        self.assertEqual(other.call("session/subscribe", session=session, since=None)["snapshot"]["info"]["availability"], "ready")
        send = other.operation(1)
        other.call("session/send", session=session, operation=send, text="bystander remains usable")
        self.assertEqual(other.outcome(send)["state"]["kind"], "accepted")
        self.assertTrue(alive(fixture.pids()[0]["pid"]))
