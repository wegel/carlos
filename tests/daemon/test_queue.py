"""Exercise native queue identities, bounded reads and private writes through actual socket dispatch."""

import json

from support import Client, Fixture, wait_for


def native_entry(identity, text, rich=False):
    return {"id": identity, "clientUserMessageId": "external-human", "input": [
        {"type": "text", "text": text, "text_elements": [{"future": "retained"}] if rich else []}]}


class QueueTests:
    def queue_clients(self, name, entries=None, history=False):
        fixture = Fixture(name, codex=True)
        fixture.native.queue_enabled = True
        fixture.native.queue_entries = entries or []
        fixture.native.history_user = history
        self.addCleanup(fixture.close)
        first, other = Client(fixture, "queue-owner"), Client(fixture, "queue-observer")
        self.addCleanup(first.close)
        self.addCleanup(other.close)
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="codex", cwd=str(fixture.root), name=None)["session"]
        first.outcome(create)
        first.call("session/subscribe", session=session, since=None)
        other.call("session/subscribe", session=session, since=None)
        return fixture, first, other, session, self.known_queue(first, session)

    def known_queue(self, client, session):
        def current():
            page = client.call("queue/read", session=session)
            return page if page["queue"]["known"] and not page["queue"]["reading"] else None
        return wait_for(current, "native queue finishes its bounded read")

    def queue_change(self, client, session, counter, page, change, state="accepted"):
        operation = client.operation(counter)
        params = dict(session=session, operation=operation, expected=page["queue"]["revision"], change=change)
        client.call("queue/change", **params)
        result = client.outcome(operation)
        self.assertEqual(result["state"]["kind"], state)
        return operation, params, result

    def test_queue_changes_preserve_rich_inputs_and_use_only_explicit_selected_ids(self):
        rich = native_entry("native-rich", "attachment preview", rich=True)
        fixture, first, other, session, page = self.queue_clients("queue-changes", [native_entry("native-plain", "original 🌍"), rich])
        plain_id, rich_id = [entry["id"] for entry in page["entries"]]
        self.assertEqual([entry["display_id"] for entry in page["entries"]], ["native-plain", "native-rich"])
        self.assertTrue(all("native-" not in entry["id"] for entry in page["entries"]))
        invalid = other.response(other.send("queue/change", session=session, operation=other.operation(1),
            expected=page["queue"]["revision"], change={"kind":"edit", "entry":"native-plain", "text":"never addressed by caption"}))
        self.assertIn("error", invalid)
        self.assertFalse(any(request.get("method") == "thread/queue/update" for request in fixture.native.requests))
        self.assertIsNone(page["entries"][1]["text_bytes"])
        self.queue_change(first, session, 2, page, {"kind": "edit", "entry": rich_id, "text": "discard attachments"}, "rejected")
        self.assertEqual(fixture.native.queue_entries[1], rich)
        self.queue_change(first, session, 3, page, {"kind": "move", "entry": rich_id, "direction": "up"})
        page = self.known_queue(other, session)
        self.assertEqual([entry["id"] for entry in page["entries"]], [rich_id, plain_id])
        self.assertEqual(fixture.native.queue_entries[0], rich)
        self.queue_change(first, session, 4, page, {"kind": "edit", "entry": plain_id, "text": "edited 🌍"})
        page = self.known_queue(other, session)
        self.assertEqual(page["entries"][1]["id"], plain_id)
        chunk = other.call("queue/text", session=session, expected=page["queue"]["revision"], entry=plain_id, offset="0")
        self.assertEqual(chunk["text"], "edited 🌍")
        self.assertTrue(chunk["complete"])
        self.queue_change(first, session, 5, page, {"kind": "start", "entry": rich_id})
        page = self.known_queue(other, session)
        self.assertEqual(len(page["entries"]), 1)
        self.queue_change(first, session, 6, page, {"kind": "delete", "entry": plain_id})
        page = self.known_queue(other, session)
        self.assertEqual(page["entries"], [])
        operation, params, _ = self.queue_change(first, session, 7, page, {"kind": "add", "text": "queued once"})
        self.assertEqual(first.call("queue/change", **params)["state"]["kind"], "accepted")
        writes = [r for r in fixture.native.requests if r.get("method") == "thread/queue/add"]
        self.assertEqual(len(writes), 1)
        self.assertEqual(writes[0]["params"]["clientUserMessageId"], f"carlos-{operation['client']}-{operation['id']}")
        self.assertEqual(writes[0]["params"]["input"], [{"type": "text", "text": "queued once", "text_elements": []}])

    def test_queue_pages_and_large_text_are_independent_bounded_and_revision_checked(self):
        text = "🌍\0line\n" * 100000
        entries = [native_entry(f"native-{index}", text if index == 0 else f"entry {index}") for index in range(40)]
        fixture, first, other, session, page = self.queue_clients("queue-pages", entries)
        self.assertEqual(len(page["entries"]), 32)
        self.assertTrue(page["has_more"])
        self.assertLess(len(json.dumps(page)), 100000)
        second = other.call("queue/read", session=session, expected=page["queue"]["revision"], after=page["entries"][-1]["id"], limit=1)
        self.assertEqual(second["entries"][0]["preview"], "entry 32")
        self.assertEqual(first.call("queue/read", session=session)["entries"][0]["id"], page["entries"][0]["id"])
        self.assertFalse(page["entries"][0]["preview_complete"])
        self.assertEqual(page["entries"][0]["text_bytes"], str(len(text.encode())))
        entry = page["entries"][0]["id"]
        offset, pieces = "0", []
        while True:
            chunk = first.call("queue/text", session=session, expected=page["queue"]["revision"], entry=entry, offset=offset)
            self.assertEqual(chunk["offset"], offset)
            self.assertLessEqual(len(chunk["text"].encode()), 65536)
            pieces.append(chunk["text"])
            offset = chunk["next_offset"]
            if chunk["complete"]:
                break
        self.assertEqual("".join(pieces), text)
        rounded = other.call("queue/text", session=session, expected=page["queue"]["revision"], entry=entry, offset="2", limit=4)
        self.assertEqual((rounded["offset"], rounded["text"]), ("0", "🌍"))
        self.queue_change(other, session, 1, page, {"kind": "delete", "entry": page["entries"][1]["id"]})
        self.known_queue(other, session)
        stale = first.response(first.send("queue/read", session=session, expected=page["queue"]["revision"], after=entry))
        self.assertIn("error", stale)
        stale_text = first.response(first.send("queue/text", session=session, expected=page["queue"]["revision"], entry=entry, offset="0"))
        self.assertIn("error", stale_text)
        _, _, refused = self.queue_change(first, session, 2, page, {"kind": "edit", "entry": entry, "text": "stale draft"}, "rejected")
        self.assertEqual(refused["state"]["recovery"]["text"], "stale draft")
        self.assertFalse(any(r.get("method") == "thread/queue/update" for r in fixture.native.requests))

    def test_queue_failures_and_recovered_text_stay_with_the_originating_operation(self):
        fixture, first, other, session, page = self.queue_clients("queue-private")
        _, _, result = self.queue_change(first, session, 2, page, {"kind": "add", "text": "PRIVATE_QUEUE_REJECTION"}, "rejected")
        self.assertEqual(result["state"]["recovery"]["text"], "PRIVATE_QUEUE_REJECTION")
        shared = other.call("queue/read", session=session)
        self.assertNotIn("PRIVATE_QUEUE_REJECTION", json.dumps(shared))
        other.call("sessions/watch")
        self.assertFalse(any(m.get("method") == "operation/updated" for m in other.messages))
        other.call("queue/read", session=session, refresh=True)
        page = self.known_queue(other, session)
        before = len([r for r in fixture.native.requests if r.get("method") == "thread/queue/add"])
        self.queue_change(first, session, 3, page, {"kind": "add", "text": " "}, "rejected")
        self.assertEqual(len([r for r in fixture.native.requests if r.get("method") == "thread/queue/add"]), before)
        malformed = first.response(first.send("queue/change", session=session, operation=first.operation(4), expected=page["queue"]["revision"], change={"kind": "add", "text": "unsafe", "threadId": "foreign"}))
        self.assertIn("error", malformed)

    def test_held_queue_acknowledgement_does_not_block_reads_prompts_controls_or_private_resume(self):
        fixture, first, other, session, page = self.queue_clients("queue-held")
        fixture.native.queue_hold = True
        operation = first.operation(2)
        params = dict(session=session, operation=operation, expected=page["queue"]["revision"], change={"kind": "add", "text": "held queue"})
        first.call("queue/change", **params)
        wait_for(lambda: fixture.native.queue_pending, "native queue acknowledgement held")
        first.close()
        resumed = Client(fixture, "queue-resumed", resume=first.hello)
        self.addCleanup(resumed.close)
        self.assertEqual(resumed.call("operation/read", operation=operation)["state"]["kind"], "pending")
        self.assertTrue(other.call("queue/read", session=session)["queue"]["pending"])
        send = other.operation(1)
        other.call("session/send", session=session, operation=send, text="held")
        card = other.message(lambda m: m.get("method") == "session/event" and m["params"]["event"]["kind"] == "approval_requested")["params"]["event"]["approval"]
        snapshot = other.call("session/subscribe", session=session, since=None)["snapshot"]
        interrupt = other.operation(2)
        other.call("session/interrupt", session=session, operation=interrupt, turn=snapshot["info"]["current_turn"])
        other.call("session/approve", session=session, operation=other.operation(3), approval=card["id"], answer={"kind": "decision", "decision": "once"})
        self.assertEqual(other.outcome(interrupt)["state"]["kind"], "accepted")
        self.assertEqual(other.outcome(send)["state"]["kind"], "accepted")
        fixture.native.release_queue()
        self.assertEqual(resumed.outcome(operation)["state"]["kind"], "accepted")
        self.assertEqual(resumed.call("queue/change", **params)["state"]["kind"], "accepted")
        self.assertEqual(len([r for r in fixture.native.requests if r.get("method") == "thread/queue/add"]), 1)

    def test_a_pending_queue_write_blocks_rewind_and_keeps_both_original_session_and_text(self):
        fixture, first, other, session, page = self.queue_clients("queue-fork", history=True)
        fixture.native.queue_hold = True
        operation = first.operation(2)
        first.call("queue/change", session=session, operation=operation, expected=page["queue"]["revision"], change={"kind": "add", "text": "old root draft"})
        wait_for(lambda: fixture.native.queue_pending, "old root queue acknowledgement held")
        snapshot = other.call("session/subscribe", session=session, since=None)["snapshot"]
        history = other.call("session/history", session=session, before=snapshot["entries"][0]["id"], limit=1000)
        user = next(row for row in history["entries"] if row["kind"] == "user")
        rewind = other.operation(1)
        other.call("session/rewind", session=session, operation=rewind, entry=user["id"], text="after queue fork")
        refused = other.outcome(rewind)
        self.assertEqual(refused["state"]["kind"], "rejected")
        self.assertEqual(refused["state"]["recovery"]["text"], "after queue fork")
        self.assertFalse(any(r.get("method") == "thread/fork" for r in fixture.native.requests))
        self.assertEqual(first.call("operation/read", operation=operation)["state"]["kind"], "pending")
        fixture.native.release_queue()
        self.assertEqual(first.outcome(operation)["state"]["kind"], "accepted")
        current = self.known_queue(other, session)
        self.assertEqual(current["entries"][0]["preview"], "old root draft")
        self.assertEqual(fixture.native.thread_id, "native-root")

    def test_closed_native_queue_keeps_unconfirmed_text_private_and_never_replays_it(self):
        fixture, first, other, session, page = self.queue_clients("queue-close")
        fixture.native.queue_hold = True
        operation = first.operation(2)
        first.call("queue/change", session=session, operation=operation, expected=page["queue"]["revision"], change={"kind": "add", "text": "private unconfirmed queue text"})
        wait_for(lambda: fixture.native.queue_pending, "native queue acknowledgement held")
        fixture.native.disconnect()
        result = first.outcome(operation)
        self.assertEqual(result["state"]["kind"], "uncertain")
        self.assertEqual(result["state"]["recovery"]["text"], "private unconfirmed queue text")
        other.call("sessions/watch")
        self.assertFalse(any(m.get("method") == "operation/updated" for m in other.messages))
        self.assertEqual(len([r for r in fixture.native.requests if r.get("method") == "thread/queue/add"]), 1)

    def test_other_backends_refuse_native_queue_changes_without_sending_backend_json(self):
        fixture = Fixture("queue-unsupported")
        self.addCleanup(fixture.close)
        client = Client(fixture, "queue-unsupported-client")
        self.addCleanup(client.close)
        create = client.operation(1)
        session = client.call("session/create", operation=create, backend="grok", cwd=str(fixture.root), name=None)["session"]
        client.outcome(create)
        read = client.response(client.send("queue/read", session=session))
        self.assertIn("does not support", read["error"]["message"])
        operation = client.operation(2)
        client.call("queue/change", session=session, operation=operation, expected="0" * 64, change={"kind": "add", "text": "preserve me"})
        result = client.outcome(operation)
        self.assertEqual(result["state"]["kind"], "rejected")
        self.assertEqual(result["state"]["recovery"]["text"], "preserve me")
        self.assertFalse(fixture.native_requests("thread/queue/add"))

    def test_native_queue_add_bypasses_a_held_chat_start_and_keeps_stale_writes_private(self):
        fixture, first, other, session, page = self.queue_clients("queue-after-held-start")
        sending = first.operation(2)
        first.call("session/send", session=session, operation=sending, text="held")
        wait_for(lambda: any(r.get("method") == "turn/start" for r in fixture.native.requests), "native chat start held")
        self.queue_change(other, session, 1, page, {"kind":"add", "text":"queued while native chat waits"})
        self.assertEqual(first.call("operation/read", operation=sending)["state"]["kind"], "queued")
        self.assertEqual((fixture.native.answers, fixture.native.interrupts), (0, 0))
        current = self.known_queue(other, session)
        self.assertEqual(current["entries"][0]["preview"], "queued while native chat waits")
        _, _, stale = self.queue_change(other, session, 2, page,
            {"kind":"edit", "entry":current["entries"][0]["id"], "text":"stale queue draft"}, "rejected")
        self.assertEqual(stale["state"]["recovery"]["text"], "stale queue draft")
        self.assertFalse(any(r.get("method") == "thread/queue/update" for r in fixture.native.requests))
        snapshot = other.call("session/subscribe", session=session, since=None)["snapshot"]
        card = next(card for card in snapshot["approvals"] if card["active"])
        approve, interrupt = other.operation(3), other.operation(4)
        other.call("session/approve", session=session, operation=approve, approval=card["id"],
                   answer={"kind":"decision", "decision":"once"})
        other.call("session/interrupt", session=session, operation=interrupt, turn=snapshot["info"]["current_turn"])
        self.assertEqual(other.outcome(approve)["state"]["kind"], "accepted")
        self.assertEqual(other.outcome(interrupt)["state"]["kind"], "accepted")
        self.assertEqual(first.outcome(sending)["state"]["kind"], "accepted")
        self.assertEqual(len([r for r in fixture.native.requests if r.get("method") == "thread/queue/add"]), 1)

    def test_native_queue_mutations_remain_serialized_while_an_acknowledgement_is_held(self):
        fixture, first, other, session, page = self.queue_clients("queue-serialized")
        fixture.native.queue_hold = True
        first_write = first.operation(2)
        first.call("queue/change", session=session, operation=first_write, expected=page["queue"]["revision"],
                   change={"kind":"add", "text":"first pending native input"})
        wait_for(lambda: fixture.native.queue_pending, "first native queue write held")
        current = self.known_queue(other, session)
        self.assertTrue(current["queue"]["pending"])
        _, _, rejected = self.queue_change(other, session, 1, current,
            {"kind":"add", "text":"second private draft"}, "rejected")
        self.assertEqual(rejected["state"]["recovery"]["text"], "second private draft")
        self.assertEqual(len([r for r in fixture.native.requests if r.get("method") == "thread/queue/add"]), 1)
        fixture.native.release_queue()
        self.assertEqual(first.outcome(first_write)["state"]["kind"], "accepted")

    def held_native_barrier(self, fixture, method):
        held = {}
        original = fixture.native.handle
        async def intercept(link, request):
            if request.get("method") == method:
                held.update(link=link, request=request)
            else:
                await original(link, request)
        fixture.native.handle = intercept
        return held, original

    def prove_queue_stays_behind_barrier(self, fixture, client, session):
        before = len([r for r in fixture.native.requests if r.get("method") == "thread/goal/get"])
        client.call("goal/read", session=session, refresh=True)
        wait_for(lambda: len([r for r in fixture.native.requests if r.get("method") == "thread/goal/get"]) > before,
                 "owner and native control writer advance while barrier waits")
        self.assertFalse(any(r.get("method") == "thread/queue/add" for r in fixture.native.requests))

    def test_native_queue_add_cannot_pass_a_held_rename(self):
        import asyncio
        fixture, first, other, session, page = self.queue_clients("queue-name-barrier")
        held, original = self.held_native_barrier(fixture, "thread/name/set")
        rename = first.operation(2)
        first.call("session/rename", session=session, operation=rename, name="held queue name")
        wait_for(lambda: held, "native rename held")
        queued = other.operation(1)
        other.call("queue/change", session=session, operation=queued, expected=page["queue"]["revision"],
                   change={"kind":"add", "text":"after native rename"})
        self.prove_queue_stays_behind_barrier(fixture, other, session)
        asyncio.run_coroutine_threadsafe(original(held["link"], held["request"]), fixture.native.loop).result(timeout=3)
        self.assertEqual(first.outcome(rename)["state"]["kind"], "accepted")
        self.assertEqual(other.outcome(queued)["state"]["kind"], "accepted")
        self.assertEqual(fixture.native.queue_entries[0]["input"][0]["text"], "after native rename")

    def test_native_queue_add_cannot_pass_a_held_fork_or_retarget_its_new_root(self):
        import asyncio
        fixture, first, other, session, page = self.queue_clients("queue-fork-barrier", history=True)
        snapshot = first.call("session/subscribe", session=session, since=None)["snapshot"]
        history = first.call("session/history", session=session, before=snapshot["entries"][0]["id"], limit=1000)
        user = next(entry for entry in history["entries"] if entry["kind"] == "user")
        held, original = self.held_native_barrier(fixture, "thread/fork")
        rewind = first.operation(2)
        first.call("session/rewind", session=session, operation=rewind, entry=user["id"], text="new native root")
        wait_for(lambda: held, "native fork held")
        queued = other.operation(1)
        other.call("queue/change", session=session, operation=queued, expected=page["queue"]["revision"],
                   change={"kind":"add", "text":"old-root private queue draft"})
        self.prove_queue_stays_behind_barrier(fixture, other, session)
        asyncio.run_coroutine_threadsafe(original(held["link"], held["request"]), fixture.native.loop).result(timeout=3)
        self.assertEqual(first.outcome(rewind)["state"]["kind"], "accepted")
        rejected = other.outcome(queued)
        self.assertEqual(rejected["state"]["kind"], "rejected")
        self.assertEqual(rejected["state"]["recovery"]["text"], "old-root private queue draft")
        self.assertEqual(fixture.native.thread_id, "native-fork")
        self.assertFalse(any(r.get("method") == "thread/queue/add" for r in fixture.native.requests))
