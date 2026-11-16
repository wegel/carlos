"""Exercise durable local device controls on the actual daemon without launching operator sessions."""

import sqlite3
import os
import stat
import subprocess

import support
from support import Client, Fixture, alive


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

    def operator_cli(self, fixture, *args):
        return subprocess.run([str(support.BINARY), "device", *map(str, args), "--socket", str(fixture.socket)],
                              env=fixture.env, cwd=fixture.root, capture_output=True, text=True, timeout=10)

    def test_device_cli_creates_distinct_private_tokens_without_printing_or_storing_their_secrets(self):
        fixture, client = self.device_fixture("cli-token")
        identifiers = []
        for number in range(2):
            path = fixture.root / f"phone-{number}.token"
            result = self.operator_cli(fixture, "add", "--name", f"Phone {number} 🌍", "--token-file", path)
            self.assertEqual(result.returncode, 0, result.stderr)
            raw = path.read_text()
            identifier, secret = raw.strip().split(".")
            self.assertEqual((len(identifier), len(secret)), (32, 64))
            self.assertEqual(len(bytes.fromhex(secret)), 32)
            self.assertEqual(len(raw), 98)
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
            self.assertEqual(path.stat().st_uid, os.geteuid())
            self.assertIn(identifier, result.stdout)
            self.assertNotIn(secret, result.stdout + result.stderr + (fixture.root / "daemon.log").read_text())
            identifiers.append(identifier)
        self.assertNotEqual(*identifiers)
        rows = client.call("device/list")["devices"]
        self.assertEqual({row["id"] for row in rows}, set(identifiers))
        listed = self.operator_cli(fixture, "list")
        self.assertEqual(listed.returncode, 0, listed.stderr)
        self.assertTrue(all(row["id"] + "\t" + row["name"] in listed.stdout for row in rows))
        database = sqlite3.connect(fixture.root / "state/sessions.sqlite3")
        self.addCleanup(database.close)
        for number, identifier in enumerate(identifiers):
            secret = (fixture.root / f"phone-{number}.token").read_text().strip().split(".")[1]
            verifier = database.execute("SELECT verifier FROM devices WHERE id=?", (identifier,)).fetchone()[0]
            self.assertEqual(len(verifier), 64)
            self.assertNotEqual(verifier, secret)
        revoked = self.operator_cli(fixture, "revoke", identifiers[0])
        self.assertEqual(revoked.returncode, 0, revoked.stderr)
        self.assertEqual([row["id"] for row in client.call("device/list")["devices"]], identifiers[1:])
        self.assertEqual(fixture.pids(), [])

    def test_device_cli_refuses_existing_files_symlinks_and_invalid_names_without_provisioning(self):
        fixture, client = self.device_fixture("cli-exclusive")
        existing = fixture.root / "existing.token"
        existing.write_text("keep this exact file")
        link = fixture.root / "link.token"
        link.symlink_to(existing)
        for path in [existing, link]:
            result = self.operator_cli(fixture, "add", "--name", "Phone", "--token-file", path)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("create a new private token file", result.stderr)
            self.assertEqual(existing.read_text(), "keep this exact file")
        invalid = fixture.root / "invalid.token"
        result = self.operator_cli(fixture, "add", "--name", "Phone\nforged", "--token-file", invalid)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(invalid.exists())
        self.assertTrue(link.is_symlink())
        self.assertEqual(client.call("device/list"), {"devices": []})
        self.assertEqual(fixture.pids(), [])

    def test_uncertain_cli_grant_keeps_its_token_and_never_overwrites_or_retries_it(self):
        fixture, client = self.device_fixture("cli-uncertain")
        database = sqlite3.connect(fixture.root / "state/sessions.sqlite3")
        self.addCleanup(database.close)
        database.execute("CREATE TRIGGER refuse_cli_device BEFORE INSERT ON devices BEGIN SELECT RAISE(FAIL,'fixture refused CLI grant'); END")
        database.commit()
        path = fixture.root / "unconfirmed.token"
        result = self.operator_cli(fixture, "add", "--name", "Phone", "--token-file", path)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("uncertain", result.stderr)
        before = path.read_bytes()
        secret = before.decode().strip().split(".")[1]
        self.assertNotIn(secret, result.stdout + result.stderr)
        self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
        self.assertEqual(client.call("device/list"), {"devices": []})
        database.execute("DROP TRIGGER refuse_cli_device")
        database.commit()
        retried = self.operator_cli(fixture, "add", "--name", "Phone", "--token-file", path)
        self.assertNotEqual(retried.returncode, 0)
        self.assertEqual(path.read_bytes(), before)
        self.assertEqual(client.call("device/list"), {"devices": []})

    def test_device_cli_remains_usable_at_full_native_owner_capacity(self):
        fixture, client = self.device_fixture("cli-owner-capacity")
        for number in range(16):
            cwd = fixture.root / str(number)
            cwd.mkdir()
            operation = client.operation(number + 1)
            client.call("session/create", operation=operation, backend="grok", cwd=str(cwd), name=None)
            self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
        self.assertEqual(len(fixture.pids()), 16)
        path = fixture.root / "capacity.token"
        result = self.operator_cli(fixture, "add", "--name", "Phone", "--token-file", path)
        self.assertEqual(result.returncode, 0, result.stderr)
        identifier = path.read_text().split(".")[0]
        self.assertEqual(self.operator_cli(fixture, "list").returncode, 0)
        revoked = self.operator_cli(fixture, "revoke", identifier)
        self.assertEqual(revoked.returncode, 0, revoked.stderr)
        self.assertEqual(len(fixture.pids()), 16)
        self.assertTrue(all(alive(record["pid"]) for record in fixture.pids()))

    def test_pending_device_add_recovers_after_disconnect_without_a_second_grant(self):
        fixture, first = self.device_fixture("lost-reply")
        database = sqlite3.connect(fixture.root / "state/sessions.sqlite3")
        self.addCleanup(database.close)
        database.execute("BEGIN IMMEDIATE")
        params = self.provision(first, 1)
        first.call("device/add", **params)
        self.assertEqual(first.call("operation/read", operation=params["operation"])["state"]["kind"], "pending")
        first.close()
        database.rollback()
        resumed = Client(fixture, "recover-device", resume=first.hello)
        self.addCleanup(resumed.close)
        self.assertTrue(resumed.hello["resumed"])
        accepted = resumed.outcome(params["operation"])
        self.assertEqual(accepted["state"]["kind"], "accepted")
        self.assertEqual(resumed.call("device/add", **params), accepted)
        self.assertEqual(resumed.call("device/list"), {"devices": [params["device"]]})
        self.assertEqual(database.execute("SELECT count(*) FROM devices").fetchone()[0], 1)
        self.assertEqual(fixture.pids(), [])

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
