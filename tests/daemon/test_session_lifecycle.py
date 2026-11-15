"""Stop real native owners without losing logical identity, private results, or other owners."""

import json
import sqlite3

from support import Client, Fixture, alive, wait_for


class SessionLifecycleTests:
    def lifecycle_fixture(self, name, **options):
        fixture = Fixture("lifecycle-" + name, **options)
        self.addCleanup(fixture.close)
        client = Client(fixture, "operator")
        self.addCleanup(client.close)
        return fixture, client

    def create_owner(self, fixture, client, counter, cwd=None):
        operation = client.operation(counter)
        session = client.call("session/create", operation=operation, backend="grok",
                              cwd=str(cwd or fixture.root), name="Saved conversation")["session"]
        self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
        return session

    def lifecycle(self, client, session, action, counter):
        operation = client.operation(counter)
        client.call("session/" + action, session=session, operation=operation)
        return client.outcome(operation)

    def test_stop_releases_a_full_native_owner_slot_and_reopen_keeps_history(self):
        fixture, client = self.lifecycle_fixture("capacity")
        sessions = []
        for number in range(16):
            cwd = fixture.root / str(number)
            cwd.mkdir()
            sessions.append(self.create_owner(fixture, client, number + 1, cwd))
        pids = [record["pid"] for record in fixture.pids()]
        self.assertEqual(len(pids), 16)
        refused = client.call("session/create", operation=client.operation(17), backend="grok",
                              cwd=str(fixture.root), name=None)
        self.assertEqual(refused["state"]["kind"], "rejected")
        self.assertEqual(len(fixture.pids()), 16)
        stopped = self.lifecycle(client, sessions[0], "stop", 18)
        self.assertEqual(stopped["state"]["kind"], "accepted")
        self.assertFalse(alive(pids[0]), "accepted stop must reap its native process")
        self.assertTrue(all(alive(pid) for pid in pids[1:]))
        seventeenth = self.create_owner(fixture, client, 19)
        self.assertEqual(len(fixture.pids()), 17)
        self.assertEqual(self.lifecycle(client, seventeenth, "stop", 20)["state"]["kind"], "accepted")
        (fixture.root / "release-history").write_text("go")
        self.assertEqual(self.lifecycle(client, sessions[0], "reopen", 21)["state"]["kind"], "accepted")
        restored = client.call("session/subscribe", session=sessions[0], since=None)["snapshot"]
        self.assertEqual(restored["info"]["id"], sessions[0])
        self.assertEqual(restored["info"]["name"], "Saved conversation")
        self.assertTrue(any("History complete" in row.get("text", "") or row.get("source") for row in restored["entries"]))
        self.assertEqual(fixture.native_requests("session/load")[-1]["params"]["sessionId"], "grok-saved")
        # An old stop retry returns its old result and cannot stop the replacement.
        retried = client.call("session/stop", session=sessions[0], operation=client.operation(18))
        self.assertEqual(retried, stopped)
        self.assertEqual(client.call("session/subscribe", session=sessions[0], since=None)["snapshot"]["info"]["availability"], "ready")
        self.assertTrue(all(alive(pid) for pid in pids[1:]))

    def test_stopped_subscription_and_daemon_restart_never_reopen_implicitly(self):
        fixture, client = self.lifecycle_fixture("restart")
        session = self.create_owner(fixture, client, 1)
        self.assertEqual(self.lifecycle(client, session, "stop", 2)["state"]["kind"], "accepted")
        reply = client.response(client.send("session/subscribe", session=session, since=None))
        self.assertIn("explicitly reopen", reply["error"]["message"])
        fixture.stop()
        fixture.start()
        resumed = Client(fixture, "after-restart")
        self.addCleanup(resumed.close)
        saved = resumed.call("sessions/watch")["sessions"][0]
        self.assertEqual((saved["id"], saved["availability"], saved["ralph"]), (session, "closed", False))
        reply = resumed.response(resumed.send("session/subscribe", session=session, since=None))
        self.assertIn("explicitly reopen", reply["error"]["message"])
        self.assertEqual(len(fixture.pids()), 1)

    def test_stop_joins_native_before_disk_ack_and_reopen_cannot_overtake_it(self):
        fixture, client = self.lifecycle_fixture("disk-order")
        session = self.create_owner(fixture, client, 1)
        database = sqlite3.connect(fixture.root / "state/sessions.sqlite3")
        self.addCleanup(database.close)
        database.execute("BEGIN IMMEDIATE")
        stop = client.operation(2)
        client.call("session/stop", session=session, operation=stop)
        wait_for(lambda: not alive(fixture.pids()[0]["pid"]), "native exits while disk waits")
        self.assertEqual(client.call("operation/read", operation=stop)["state"]["kind"], "pending")
        refused = self.lifecycle(client, session, "reopen", 3)
        self.assertEqual(refused["state"]["kind"], "rejected")
        self.assertIn("still stopping", refused["state"]["message"])
        database.rollback()
        self.assertEqual(client.outcome(stop)["state"]["kind"], "accepted")
        row = database.execute("SELECT stopped,ralph,native_thread FROM sessions WHERE id=?", (session,)).fetchone()
        self.assertEqual(row, (1, 0, "grok-saved"))

    def test_private_held_and_queued_text_finish_without_an_automatic_resend(self):
        fixture, first = self.lifecycle_fixture("private-drafts", mode="grok-held-prompt")
        second = Client(fixture, "other-editor")
        self.addCleanup(second.close)
        session = self.create_owner(fixture, first, 1)
        original = first.operation(2)
        first.call("session/send", session=session, operation=original, text="first")
        wait_for(lambda: fixture.native_requests("session/prompt"), "native first prompt")
        wait_for(lambda: first.call("session/subscribe", session=session, since=None)["snapshot"]["info"]["current_turn"], "native turn is visible")
        own_queued = first.operation(3)
        first.call("session/send", session=session, operation=own_queued, text="first private 🦊", policy="queue")
        queued = second.operation(1)
        second.call("session/send", session=session, operation=queued, text="second private 🐙", policy="queue")
        self.assertEqual(self.lifecycle(second, session, "stop", 2)["state"]["kind"], "accepted")
        for client, operation, text in [(first, own_queued, "first private 🦊"), (second, queued, "second private 🐙")]:
            result = client.outcome(operation)
            self.assertIn(result["state"]["kind"], ("uncertain", "rejected"))
            self.assertEqual(result["state"]["recovery"], {"text": text, "operation": operation})
        self.assertEqual(first.outcome(original)["state"]["kind"], "accepted")
        self.assertEqual(len(fixture.native_requests("session/prompt")), 1)
        foreign = first.response(first.send("operation/read", operation=queued))
        self.assertEqual(foreign["error"]["code"], -32003)

    def test_failed_stop_save_reports_uncertainty_after_reaping_the_native_owner(self):
        fixture, client = self.lifecycle_fixture("save-failure")
        session = self.create_owner(fixture, client, 1)
        database = sqlite3.connect(fixture.root / "state/sessions.sqlite3")
        self.addCleanup(database.close)
        database.execute("CREATE TRIGGER refuse_stop BEFORE UPDATE ON sessions WHEN NEW.stopped=1 BEGIN SELECT RAISE(FAIL,'fixture refused stopped metadata'); END")
        database.commit()
        result = self.lifecycle(client, session, "stop", 2)
        self.assertEqual(result["state"]["kind"], "uncertain")
        self.assertIn("catalogue save", result["state"]["message"])
        self.assertFalse(alive(fixture.pids()[0]["pid"]))
        self.assertEqual(database.execute("SELECT stopped FROM sessions WHERE id=?", (session,)).fetchone(), (0,))
        reply = client.response(client.send("session/subscribe", session=session, since=None))
        self.assertIn("explicitly reopen", reply["error"]["message"])
        self.assertEqual(self.lifecycle(client, session, "reopen", 3)["state"]["kind"], "rejected")

    def test_stop_cancels_a_captured_native_launch_without_stranding_create(self):
        fixture, client = self.lifecycle_fixture("launch-cancel", mode="grok-held-initialize")
        create = client.operation(1)
        session = client.call("session/create", operation=create, backend="grok", cwd=str(fixture.root), name=None)["session"]
        wait_for(lambda: (fixture.root / "initialize-held").exists(), "real initialize is waiting")
        self.assertEqual(self.lifecycle(client, session, "stop", 2)["state"]["kind"], "accepted")
        self.assertFalse(alive(fixture.pids()[0]["pid"]))
        self.assertEqual(client.outcome(create)["state"]["kind"], "uncertain")
        self.assertEqual(self.lifecycle(client, session, "reopen", 3)["state"]["kind"], "rejected")
        self.assertEqual(len(fixture.pids()), 1)

    def test_two_reopens_share_one_restore_and_advance_the_existing_subscribers_cursor(self):
        fixture, first = self.lifecycle_fixture("stream-reopen")
        second = Client(fixture, "second-opener")
        self.addCleanup(second.close)
        session = self.create_owner(fixture, first, 1)
        initial = first.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertEqual(self.lifecycle(first, session, "stop", 2)["state"]["kind"], "accepted")
        closed = first.message(lambda m: m.get("method") == "session/snapshot" and m["params"]["snapshot"]["info"]["availability"] == "closed")["params"]["snapshot"]
        self.assertGreater(int(closed["seq"]), int(initial["seq"]))
        a, b = first.operation(3), second.operation(1)
        first.call("session/reopen", session=session, operation=a)
        second.call("session/reopen", session=session, operation=b)
        wait_for(lambda: fixture.native_requests("session/load"), "one native restore waits for history")
        (fixture.root / "release-history").write_text("go")
        self.assertEqual(first.outcome(a)["state"]["kind"], "accepted")
        self.assertEqual(second.outcome(b)["state"]["kind"], "accepted")
        self.assertEqual(len(fixture.native_requests("session/load")), 1)
        reset = first.message(lambda m: m.get("method") == "session/event" and m["params"]["event"]["kind"] == "reset" and int(m["params"]["seq"]) > int(closed["seq"]))["params"]
        self.assertEqual(int(reset["seq"]), int(closed["seq"]) + 1)
        self.assertEqual(reset["event"]["snapshot"]["info"]["id"], session)

    def test_attached_stop_interrupts_only_its_turn_and_keeps_the_external_server_usable(self):
        fixture, client = self.lifecycle_fixture("attached", codex=True)
        from websockets.sync.client import unix_connect
        bystander = self.enterContext(unix_connect(str(fixture.native.path)))
        create = client.operation(1)
        session = client.call("session/create", operation=create, backend="codex", cwd=str(fixture.root), name=None)["session"]
        self.assertEqual(client.outcome(create)["state"]["kind"], "accepted")
        client.call("session/subscribe", session=session, since=None)
        held = client.operation(2)
        client.call("session/send", session=session, operation=held, text="held")
        client.message(lambda m: m.get("method") == "session/event" and m["params"]["event"]["kind"] == "approval_requested")
        self.assertEqual(self.lifecycle(client, session, "stop", 3)["state"]["kind"], "accepted")
        closed = client.message(lambda m: m.get("method") == "session/snapshot" and m["params"]["snapshot"]["info"]["availability"] == "closed")["params"]["snapshot"]
        self.assertEqual(closed["approvals"], [])
        self.assertIsNone(closed["info"]["current_turn"])
        requests = [r for r in fixture.native.requests if r.get("method") == "turn/interrupt"]
        self.assertEqual([r["params"] for r in requests], [{"threadId": "native-root", "turnId": "live"}])
        self.assertEqual(fixture.native.answers, 0, "stop cannot invent consent")
        recovery = client.outcome(held)["state"]
        self.assertEqual(recovery["kind"], "uncertain")
        self.assertEqual(recovery["recovery"], {"operation": held, "text": "held"})
        bystander.send(json.dumps({"id": "other-client", "method": "model/list", "params": {}}))
        self.assertEqual(json.loads(bystander.recv(timeout=2))["id"], "other-client")

    def test_unacknowledged_attached_interrupt_reports_uncertainty_and_still_releases_owner(self):
        fixture, client = self.lifecycle_fixture("attached-lost-ack", codex=True)
        fixture.native.hold_interrupts = True
        create = client.operation(1)
        session = client.call("session/create", operation=create, backend="codex", cwd=str(fixture.root), name=None)["session"]
        client.outcome(create)
        client.call("session/subscribe", session=session, since=None)
        client.call("session/send", session=session, operation=client.operation(2), text="held")
        client.message(lambda m: m.get("method") == "session/event" and m["params"]["event"]["kind"] == "approval_requested")
        result = self.lifecycle(client, session, "stop", 3)
        self.assertEqual(result["state"]["kind"], "uncertain")
        self.assertIn("Attached Codex stop was not confirmed", result["state"]["message"])
        self.assertEqual(fixture.native.interrupts, 1)
        self.assertEqual(fixture.native.answers, 0)
        self.assertEqual(client.call("sessions/watch")["sessions"][0]["availability"], "closed")
        self.assertEqual(self.lifecycle(client, session, "reopen", 4)["state"]["kind"], "accepted")
        self.assertEqual(fixture.native.resumed, 1)

    def test_stop_result_survives_disconnect_while_its_durable_acknowledgement_waits(self):
        fixture, first = self.lifecycle_fixture("lost-stop-reply")
        session = self.create_owner(fixture, first, 1)
        database = sqlite3.connect(fixture.root / "state/sessions.sqlite3")
        self.addCleanup(database.close)
        database.execute("BEGIN IMMEDIATE")
        operation = first.operation(2)
        first.call("session/stop", session=session, operation=operation)
        wait_for(lambda: not alive(fixture.pids()[0]["pid"]), "captured owner joined before disconnect")
        first.close()
        resumed = Client(fixture, "resume-stop-result", resume=first.hello)
        self.addCleanup(resumed.close)
        self.assertTrue(resumed.hello["resumed"])
        self.assertEqual(resumed.call("operation/read", operation=operation)["state"]["kind"], "pending")
        database.rollback()
        self.assertEqual(resumed.outcome(operation)["state"]["kind"], "accepted")
        self.assertEqual(resumed.call("session/stop", session=session, operation=operation)["state"]["kind"], "accepted")
        self.assertEqual(len(fixture.pids()), 1)

    def test_stop_settles_a_held_native_queue_write_and_keeps_its_text_private(self):
        fixture, first, other, session, page = self.queue_clients("lifecycle-held-queue")
        fixture.native.queue_hold = True
        operation = first.operation(2)
        first.call("queue/change", session=session, operation=operation, expected=page["queue"]["revision"],
                   change={"kind": "add", "text": "held native queue 🦊"})
        wait_for(lambda: fixture.native.queue_pending, "native queue acknowledgement waits")
        self.assertEqual(self.lifecycle(other, session, "stop", 1)["state"]["kind"], "accepted")
        result = first.outcome(operation)["state"]
        self.assertEqual(result["kind"], "uncertain")
        self.assertEqual(result["recovery"], {"operation": operation, "text": "held native queue 🦊"})
        denied = other.response(other.send("operation/read", operation=operation))
        self.assertEqual(denied["error"]["code"], -32003)
        self.assertEqual(sum(r.get("method") == "thread/queue/add" for r in fixture.native.requests), 1)

    def test_stop_settles_held_settings_and_never_saves_unconfirmed_defaults(self):
        fixture, first, other, session, facts = self.settings_clients("lifecycle-held-settings")
        fixture.native.settings_hold.add("thread/settings/update")
        params = self.settings_params(first, session, facts)
        first.call("settings/set", **params)
        wait_for(lambda: fixture.native.settings_pending, "native settings acknowledgement waits")
        self.assertEqual(self.lifecycle(other, session, "stop", 1)["state"]["kind"], "accepted")
        result = first.outcome(params["operation"])["state"]
        self.assertEqual(result["kind"], "uncertain")
        self.assertIsNone(result["recovery"])
        self.assertFalse(self.settings_path(fixture).exists())
        self.assertEqual(len(self.settings_writes(fixture)), 1)
