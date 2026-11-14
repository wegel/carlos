"""Prove reconnect privacy, durable-create acknowledgement and crash-safe automatic-work choices."""

import json
import sqlite3

from support import Client, Fixture, wait_for


class RecoveryTests:
    def test_an_expired_shared_prefix_gets_a_fresh_snapshot_without_a_sequence_gap(self):
        fixture = Fixture("expired-prefix", codex=True)
        self.addCleanup(fixture.close)
        first = Client(fixture, "disconnecting")
        second = Client(fixture, "writer")
        self.addCleanup(first.close)
        self.addCleanup(second.close)
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="codex", cwd=str(fixture.root), name=None)["session"]
        first.outcome(create)
        snapshot = first.call("session/subscribe", session=session, since=None)["snapshot"]
        first.close()
        operation = second.operation(1)
        second.call("session/send", session=session, operation=operation, text="many-rows")
        self.assertEqual(second.outcome(operation)["state"]["kind"], "accepted")
        fresh = second.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertGreater(int(fresh["seq"]) - int(snapshot["seq"]), 1024)
        resumed = Client(fixture, "expired-resume", resume=first.hello)
        self.addCleanup(resumed.close)
        replacement = resumed.call("session/subscribe", session=session, since=snapshot["seq"])
        self.assertEqual(replacement["kind"], "snapshot")
        self.assertEqual(replacement["snapshot"], fresh)

    def test_sqlite_wait_does_not_block_a_second_clients_reads_or_native_send(self):
        fixture = Fixture("sqlite-wait")
        self.addCleanup(fixture.close)
        first = Client(fixture, "creator")
        second = Client(fixture, "observer")
        self.addCleanup(first.close)
        self.addCleanup(second.close)
        database = sqlite3.connect(fixture.root / "state/sessions.sqlite3")
        self.addCleanup(database.close)
        database.execute("BEGIN IMMEDIATE")
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="grok", cwd=str(fixture.root), name=None)["session"]
        snapshot = second.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertIn(snapshot["info"]["availability"], ("starting", "ready"))
        send = second.operation(1)
        second.call("session/send", session=session, operation=send, text="while disk waits")
        self.assertEqual(second.outcome(send)["state"]["kind"], "accepted")
        self.assertEqual(first.call("operation/read", operation=create)["state"]["kind"], "pending")
        database.rollback()
        self.assertEqual(first.outcome(create)["state"]["kind"], "accepted")
        self.assertEqual(fixture.saved()[0][2], "grok-saved")

    def test_lost_pending_send_resumes_privately_and_old_missing_counters_never_dispatch(self):
        fixture = Fixture("pending-reconnect", codex=True)
        self.addCleanup(fixture.close)
        first = Client(fixture, "sender")
        self.addCleanup(first.close)
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="codex", cwd=str(fixture.root), name=None)["session"]
        first.outcome(create)
        first.call("session/subscribe", session=session, since=None)
        held = first.operation(2)
        params = dict(session=session, operation=held, text="held")
        first.call("session/send", **params)
        card = first.message(lambda m: m.get("method") == "session/event" and m["params"]["event"]["kind"] == "approval_requested")["params"]["event"]["approval"]
        first.close()
        resumed = Client(fixture, "resumed", resume=first.hello)
        other = Client(fixture, "other")
        self.addCleanup(resumed.close)
        self.addCleanup(other.close)
        self.assertTrue(resumed.hello["resumed"])
        self.assertIn(resumed.call("session/send", **params)["state"]["kind"], ("pending", "queued"))
        changed = resumed.response(resumed.send("session/send", **{**params, "text": "different"}))
        self.assertEqual(changed["error"]["code"], -32005)
        private = other.response(other.send("operation/read", operation=held))
        self.assertEqual(private["error"]["code"], -32003)
        snapshot = other.call("session/subscribe", session=session, since=None)["snapshot"]
        turn = snapshot["info"]["current_turn"]
        other.call("session/interrupt", session=session, turn=turn, operation=other.operation(10))
        resumed.call("session/approve", session=session, approval=card["id"], operation=resumed.operation(3), answer={"kind": "decision", "decision": "once"})
        self.assertEqual(resumed.outcome(held)["state"]["kind"], "accepted")
        expired = other.response(other.send("session/setRalph", session=session, on=False, operation=other.operation(1)))
        self.assertEqual(expired["error"]["code"], -32006)
        self.assertEqual(sum(request.get("method") == "turn/start" for request in fixture.native.requests), 1)
        # Keep the original cursor and identity: current-instance reconnect can replay it.
        replay = resumed.call("session/subscribe", session=session, since=snapshot["seq"])
        self.assertEqual(replay["kind"], "replay")
        sequences = [int(event["seq"]) for event in replay["events"]]
        self.assertEqual(sequences, list(range(int(snapshot["seq"])+1, int(replay["seq"])+1)))

    def test_restart_keeps_ralph_off_and_publishes_its_notice_after_history(self):
        fixture = Fixture("ralph-restart", codex=True)
        self.addCleanup(fixture.close)
        (fixture.root / ".agents").mkdir()
        (fixture.root / ".agents/ralph-prompt.md").write_text("held")
        first = Client(fixture, "ralph-owner")
        self.addCleanup(first.close)
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="codex", cwd=str(fixture.root), name=None)["session"]
        first.outcome(create)
        enable = first.operation(2)
        first.call("session/setRalph", session=session, operation=enable, on=True)
        self.assertEqual(first.outcome(enable)["state"]["kind"], "accepted")
        first.call("session/send", session=session, operation=first.operation(3), text="held")
        wait_for(lambda: any(row[0] == session and row[3] for row in fixture.saved()), "Ralph-on metadata saved")
        wait_for(lambda: any(request.get("method") == "turn/start" for request in fixture.native.requests), "one held human prompt while Ralph is on")
        fixture.stop()
        starts = sum(request.get("method") == "turn/start" for request in fixture.native.requests)
        fixture.start()
        restored = Client(fixture, "after-restart")
        self.addCleanup(restored.close)
        self.assertFalse(restored.call("sessions/watch")["sessions"][0]["ralph"])
        snapshot = restored.call("session/subscribe", session=session, since=None)["snapshot"]
        predicate = lambda entry: "Ralph was on" in entry.get("text", "")
        if not any(predicate(entry) for entry in snapshot["entries"]):
            restored.message(lambda message: message.get("method") == "session/event" and "Ralph was on" in json.dumps(message["params"]["event"]))
        self.assertEqual(sum(request.get("method") == "turn/start" for request in fixture.native.requests), starts)
