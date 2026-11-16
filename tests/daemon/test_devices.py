"""Exercise durable local device controls on the actual daemon without launching operator sessions."""

import sqlite3

from support import Client, Fixture


class DeviceTests:
    def device_fixture(self, name):
        fixture = Fixture("devices-" + name)
        self.addCleanup(fixture.close)
        client = Client(fixture, "operator")
        self.addCleanup(client.close)
        return fixture, client

    def provision(self, client, counter, index=1):
        return dict(operation=client.operation(counter),
                    device={"id": f"{index:032x}", "name": f"Phone {index} 🌍"}, verifier="b" * 64)

    def test_device_metadata_is_durable_private_and_starts_no_native_owner(self):
        fixture, first = self.device_fixture("restart")
        params = self.provision(first, 1)
        first.call("device/add", **params)
        accepted = first.outcome(params["operation"])
        self.assertEqual(accepted["state"]["kind"], "accepted")
        self.assertNotIn("session", accepted)
        self.assertEqual(first.call("device/list"), {"devices": [params["device"]]})
        second = Client(fixture, "other")
        self.addCleanup(second.close)
        denied = second.response(second.send("operation/read", operation=params["operation"]))
        self.assertEqual(denied["error"]["code"], -32003)
        first.close()
        resumed = Client(fixture, "resumed", resume=first.hello)
        self.addCleanup(resumed.close)
        self.assertTrue(resumed.hello["resumed"])
        self.assertEqual(resumed.call("device/add", **params), accepted)
        fixture.stop()
        fixture.start()
        restarted = Client(fixture, "restart")
        self.addCleanup(restarted.close)
        self.assertEqual(restarted.call("device/list"), {"devices": [params["device"]]})
        self.assertEqual(fixture.pids(), [])

    def test_old_provision_retry_after_revoke_does_not_restore_access_and_changed_work_is_rejected(self):
        fixture, client = self.device_fixture("old-retry")
        params = self.provision(client, 1)
        client.call("device/add", **params)
        accepted = client.outcome(params["operation"])
        changed = dict(params, verifier="c" * 64)
        conflict = client.response(client.send("device/add", **changed))
        self.assertEqual(conflict["error"]["code"], -32005)
        revoke = client.operation(2)
        client.call("device/revoke", operation=revoke, device=params["device"]["id"])
        self.assertEqual(client.outcome(revoke)["state"]["kind"], "accepted")
        self.assertEqual(client.call("device/add", **params), accepted)
        self.assertEqual(client.call("device/list"), {"devices": []})
        self.assertEqual(fixture.pids(), [])

    def test_device_add_waits_for_disk_while_native_send_and_private_reads_stay_usable(self):
        fixture, client = self.device_fixture("held-disk")
        create = client.operation(1)
        session = client.call("session/create", operation=create, backend="grok", cwd=str(fixture.root), name=None)["session"]
        self.assertEqual(client.outcome(create)["state"]["kind"], "accepted")
        database = sqlite3.connect(fixture.root / "state/sessions.sqlite3")
        self.addCleanup(database.close)
        database.execute("BEGIN IMMEDIATE")
        params = self.provision(client, 2)
        client.call("device/add", **params)
        self.assertEqual(client.call("operation/read", operation=params["operation"])["state"]["kind"], "pending")
        self.assertEqual(client.call("device/list"), {"devices": []})
        other = Client(fixture, "live-native")
        self.addCleanup(other.close)
        send = other.operation(1)
        other.call("session/send", session=session, operation=send, text="still usable")
        self.assertEqual(other.outcome(send)["state"]["kind"], "accepted")
        database.rollback()
        self.assertEqual(client.outcome(params["operation"])["state"]["kind"], "accepted")
        self.assertEqual(client.call("device/list"), {"devices": [params["device"]]})
        self.assertEqual(len(fixture.pids()), 1)

    def test_failed_device_writes_stay_uncertain_and_revoke_blocks_until_durable_retry(self):
        fixture, client = self.device_fixture("save-failure")
        first = self.provision(client, 1)
        client.call("device/add", **first)
        self.assertEqual(client.outcome(first["operation"])["state"]["kind"], "accepted")
        database = sqlite3.connect(fixture.root / "state/sessions.sqlite3")
        self.addCleanup(database.close)
        database.execute("CREATE TRIGGER refuse_device_add BEFORE INSERT ON devices BEGIN SELECT RAISE(FAIL,'fixture refused device'); END")
        database.commit()
        second = self.provision(client, 2, 2)
        client.call("device/add", **second)
        self.assertEqual(client.outcome(second["operation"])["state"]["kind"], "uncertain")
        self.assertEqual(client.call("device/list"), {"devices": [first["device"]]})
        database.execute("DROP TRIGGER refuse_device_add")
        database.execute("CREATE TRIGGER refuse_device_revoke BEFORE DELETE ON devices BEGIN SELECT RAISE(FAIL,'fixture refused revocation'); END")
        database.commit()
        revoke = client.operation(3)
        client.call("device/revoke", operation=revoke, device=first["device"]["id"])
        result = client.outcome(revoke)
        self.assertEqual(result["state"]["kind"], "uncertain")
        self.assertIn("blocked", result["state"]["message"])
        self.assertEqual(client.call("device/list"), {"devices": []})
        self.assertEqual(database.execute("SELECT count(*) FROM devices").fetchone()[0], 1)
        database.execute("DROP TRIGGER refuse_device_revoke")
        database.commit()
        retry = client.operation(4)
        client.call("device/revoke", operation=retry, device=first["device"]["id"])
        self.assertEqual(client.outcome(retry)["state"]["kind"], "accepted")
        self.assertEqual(database.execute("SELECT count(*) FROM devices").fetchone()[0], 0)
