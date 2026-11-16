"""Keep original tool bytes, reconnect cursors and native controls independent of network readers."""

import json
import socket
import time

from websockets.exceptions import ConnectionClosed, InvalidHandshake

from network_support import grant
from support import wait_for


class NetworkSourceTests:
    def test_network_clients_race_one_approval_without_duplicate_native_answers_or_private_results(self):
        fixture, local, phone, _, session = self.network_codex("approval-race")
        other = self.network_client(fixture, "other-approver", grant(fixture, "other-device"))
        operation, card, _, snapshot = self.network_held_tool(fixture, phone, session)
        local.call("session/subscribe", session=session, since=None)
        interrupt = local.operation(1)
        local.call("session/interrupt", session=session, operation=interrupt, turn=snapshot["info"]["current_turn"])
        self.assertEqual(local.outcome(interrupt)["state"]["kind"], "accepted")
        first, second = phone.operation(3), other.operation(1)
        one = phone.send("session/approve", session=session, operation=first, approval=card["id"], answer={"kind": "decision", "decision": "once"})
        two = other.send("session/approve", session=session, operation=second, approval=card["id"], answer={"kind": "decision", "decision": "once"})
        phone.response(one)
        other.response(two)
        states = sorted([phone.outcome(first)["state"]["kind"], other.outcome(second)["state"]["kind"]])
        self.assertEqual(states, ["accepted", "rejected"])
        self.assertEqual(phone.outcome(operation)["state"]["kind"], "accepted")
        self.assertEqual(fixture.native.answers, 1)
        foreign = other.response(other.send("operation/read", operation=first))
        self.assertEqual(foreign["error"]["code"], -32003)

    def network_codex(self, name):
        fixture, local = self.network_fixture(name, codex=True)
        token = grant(fixture, "phone")
        phone = self.network_client(fixture, "phone", token)
        operation = phone.operation(1)
        session = phone.call("session/create", operation=operation, backend="codex", cwd=str(fixture.root), name=None)["session"]
        self.assertEqual(phone.outcome(operation)["state"]["kind"], "accepted")
        phone.call("session/subscribe", session=session, since=None)
        return fixture, local, phone, token, session

    def network_held_tool(self, fixture, phone, session):
        operation = phone.operation(2)
        phone.call("session/send", session=session, operation=operation, text="held")
        card = phone.message(lambda m: m.get("method") == "session/event" and m["params"]["event"]["kind"] == "approval_requested")["params"]["event"]["approval"]
        entry = phone.message(lambda m: m.get("method") == "session/event" and m["params"]["event"]["kind"] in ("entry_added", "entry_updated") and (m["params"]["event"]["entry"].get("tool") or {}).get("has_output"))["params"]["event"]["entry"]["id"]
        snapshot = phone.call("session/subscribe", session=session, since=None)["snapshot"]
        return operation, card, entry, snapshot

    def test_network_running_source_watch_resumes_exact_split_controls_after_private_reconnect(self):
        fixture, local, phone, token, session = self.network_codex("source-reconnect")
        fixture.native.tool_first = "\x1b[3"
        fixture.native.tool_second = "1m🌍red\x1b[0m\n"
        operation, card, entry, snapshot = self.network_held_tool(fixture, phone, session)
        first = phone.call("entry/source/subscribe", session=session, entry=entry, cursor=None)
        self.assertEqual(first["text"], fixture.native.tool_first)
        phone.close()
        resumed = self.network_client(fixture, "reconnected-source", token, resume=phone.hello)
        self.assertTrue(resumed.hello["resumed"])
        resumed.call("session/subscribe", session=session, since=snapshot["seq"])
        continuation = resumed.call("entry/source/subscribe", session=session, entry=entry, cursor=first["next"])
        self.assertEqual(continuation["text"], "")
        local.call("session/subscribe", session=session, since=None)
        local.call("session/interrupt", session=session, operation=local.operation(1), turn=snapshot["info"]["current_turn"])
        local.call("session/approve", session=session, operation=local.operation(2), approval=card["id"], answer={"kind": "decision", "decision": "once"})
        self.assertEqual(resumed.outcome(operation)["state"]["kind"], "accepted")
        resumed.call("session/send", session=session, operation=resumed.operation(3), text="second")
        chunk = resumed.message(lambda m: m.get("method") == "entry/source" and "red" in m["params"]["chunk"]["text"])["params"]["chunk"]
        self.assertEqual(chunk["from"], first["next"])
        self.assertEqual(first["text"] + chunk["text"], fixture.native.tool_first + fixture.native.tool_second)
        self.assertEqual(fixture.native.interrupts, 1)
        self.assertEqual(fixture.native.answers, 1)

    def test_source_above_frame_limit_crosses_websocket_in_exact_bounded_chunks(self):
        fixture, _, phone, _, session = self.network_codex("large-source")
        operation = phone.operation(2)
        phone.call("session/send", session=session, operation=operation, text="large-plain-source")
        phone.outcome(operation)
        expected = "\0🌍\x1b[31m\n" * (4096 * 224)
        def ready():
            snapshot = phone.call("session/subscribe", session=session, since=None)["snapshot"]
            return next((row for row in snapshot["entries"] if row.get("source", {}).get("bytes") == str(len(expected.encode())) and row["done"]), None)
        entry = wait_for(ready, "large original source is ready", timeout=15)
        cursor, pieces = None, []
        while True:
            chunk = phone.call("entry/source", session=session, entry=entry["id"], cursor=cursor, limit=4294967295)
            self.assertLessEqual(len(chunk["text"].encode()), 65536)
            pieces.append(chunk["text"])
            cursor = chunk["next"]
            if not chunk["has_more"]:
                self.assertTrue(chunk["done"])
                break
        self.assertGreater(len(expected.encode()), 8 * 1024 * 1024)
        self.assertEqual("".join(pieces), expected)

    def test_slow_network_source_reader_releases_its_slot_without_stalling_another_clients_controls(self):
        fixture, local, phone, token, session = self.network_codex("slow-reader")
        fixture.native.tool_first = "x" * (128 * 1024)
        operation, card, entry, snapshot = self.network_held_tool(fixture, phone, session)
        local.call("session/subscribe", session=session, since=None)
        slow = self.network_client(fixture, "slow", token, max_queue=1)
        slow.socket.socket.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 1024)
        idle = [self.network_client(fixture, f"idle-{index}", token) for index in range(14)]
        self.assertEqual(len(idle), 14)
        for _ in range(100):
            slow.send("entry/source", session=session, entry=entry, cursor=None, limit=65536)
        began = time.monotonic()
        interrupt, answer = local.operation(1), local.operation(2)
        local.call("session/interrupt", session=session, operation=interrupt, turn=snapshot["info"]["current_turn"])
        local.call("session/approve", session=session, operation=answer, approval=card["id"], answer={"kind": "decision", "decision": "once"})
        self.assertEqual(local.outcome(interrupt)["state"]["kind"], "accepted")
        self.assertEqual(local.outcome(answer)["state"]["kind"], "accepted")
        elapsed = time.monotonic() - began
        self.assertLess(elapsed, .4, "native controls must not wait for the slow peer's500ms write deadline")
        def replacement():
            try:
                return self.network_client(fixture, "replacement", token)
            except (InvalidHandshake, ConnectionClosed, OSError):
                return None
        fresh = wait_for(replacement, "slow reader's network slot is released", timeout=3)
        self.assertTrue(fresh.hello["client"])
        self.assertEqual(phone.outcome(operation)["state"]["kind"], "accepted")
        (fixture.root / "slow-network.json").write_text(json.dumps({"controlSeconds": elapsed}))
        slow.close()

    def test_pending_send_survives_network_disconnect_and_resumes_without_a_second_native_write(self):
        fixture, local, phone, token, session = self.network_codex("lost-send-reply")
        operation, card, _, snapshot = self.network_held_tool(fixture, phone, session)
        phone.close()
        resumed = self.network_client(fixture, "resumed-send", token, resume=phone.hello)
        self.assertTrue(resumed.hello["resumed"])
        local.call("session/subscribe", session=session, since=None)
        local.call("session/interrupt", session=session, operation=local.operation(1), turn=snapshot["info"]["current_turn"])
        local.call("session/approve", session=session, operation=local.operation(2), approval=card["id"], answer={"kind": "decision", "decision": "once"})
        accepted = resumed.outcome(operation)
        self.assertEqual(accepted["state"]["kind"], "accepted")
        self.assertEqual(resumed.call("session/send", session=session, operation=operation, text="held"), accepted)
        self.assertEqual(len([request for request in fixture.native.requests if request.get("method") == "turn/start"]), 1)
