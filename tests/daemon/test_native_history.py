"""Load the newest complete native window first and preserve every older turn on demand."""

from native_history_windows import NativeHistoryWindows
from support import Client, Fixture, wait_for


class NativeHistoryTests:
    def test_codex_held_native_history_keeps_other_readers_cached_cursors_independent(self):
        fixture, peer, reader, session, opened = self.native_history_fixture("two-readers")
        before = self.native_prefix_boundary(reader, session, opened)
        other = Client(fixture, "independent-history")
        self.addCleanup(other.close)
        latest = other.call("session/subscribe", session=session, since=None)["snapshot"]
        peer.pause = True
        pending = reader.send("session/history", session=session, before=before, limit=100)
        wait_for(lambda: peer.held, "first native prefix read is held")
        cached = other.call("session/history", session=session, before=latest["entries"][0]["id"], limit=20)
        self.assertEqual(len(cached["entries"]),20)
        self.assertNotEqual(cached["entries"][0]["id"],before)
        self.assertEqual(len(peer.pages),2)
        peer.release()
        prefix = reader.response(pending)["result"]
        self.assertEqual(other.call("session/history", session=session, before=before, limit=100), prefix)
        self.assertEqual(len(peer.pages),2)
        self.assertEqual(fixture.native.prompts,0)

    def test_codex_stopping_the_owner_retires_its_original_held_history_read(self):
        fixture, peer, reader, session, opened = self.native_history_fixture("stop-held")
        before = self.native_prefix_boundary(reader, session, opened)
        peer.pause = True
        pending = reader.send("session/history", session=session, before=before, limit=100)
        wait_for(lambda: peer.held, "native prefix read is held")
        other = Client(fixture, "stop-operator")
        self.addCleanup(other.close)
        stopped = other.operation(1)
        other.call("session/stop", session=session, operation=stopped)
        self.assertEqual(other.outcome(stopped)["state"]["kind"], "accepted")
        self.assertIn("error", reader.response(pending))
        self.assertEqual(fixture.native.prompts,0)
        self.assertTrue(other.call("sessions/watch")["sessions"])

    def test_codex_busy_native_history_refuses_a_fork_without_changing_root_then_allows_explicit_retry(self):
        fixture, peer, reader, session, opened = self.native_history_fixture("fork-held")
        before = self.native_prefix_boundary(reader, session, opened)
        old = reader.call("session/subscribe", session=session, since=None)["snapshot"]["entries"]
        original = peer.handle
        async def fork(link, request):
            if request.get("method") != "thread/fork":
                return await original(link, request)
            self.assertEqual(request["params"]["beforeTurnId"], "turn-239")
            fixture.native.thread_id = "new-native-fork"
            fixture.native.forked = True
            peer.turns = []
            await fixture.native.send(link, {"id":request["id"], "result":{"thread":fixture.native.history()}})
        peer.handle = fork
        peer.pause = True
        pending = reader.send("session/history", session=session, before=before, limit=100)
        wait_for(lambda: peer.held, "original root's native prefix read is held")
        other = Client(fixture, "fork-operator")
        self.addCleanup(other.close)
        edit = other.operation(1)
        other.call("session/rewind", session=session, operation=edit, entry=old[-2]["id"], text="new native branch")
        refused = other.outcome(edit)
        self.assertEqual(refused["state"]["kind"], "uncertain")
        self.assertIn("active large history read", refused["state"]["message"])
        self.assertFalse(refused.get("rewound",False))
        self.assertEqual(fixture.native.thread_id, "native-root")
        peer.release()
        self.assertTrue(reader.response(pending)["result"]["entries"])
        retry = other.operation(2)
        other.call("session/rewind", session=session, operation=retry, entry=old[-2]["id"], text="new native branch")
        self.assertEqual(other.outcome(retry)["state"]["kind"], "accepted")
        fresh = other.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertTrue(all(row["id"] not in {entry["id"] for entry in old} for row in fresh["entries"]))
        self.assertEqual(fixture.native.prompts,1)

    def test_codex_recall_crosses_windows_without_humans_and_does_not_search_unknown_sources(self):
        from test_prompt_history import check_native_recall
        fixture, peer, client, session, opened = self.native_history_fixture("recall-sparse")
        self.assertEqual(client.outcome(opened)["state"]["kind"], "accepted")
        for turn in peer.turns[:140]:
            turn["items"] = [item for item in turn["items"] if item["type"] != "userMessage"]
        # The newest window is already loaded; remove humans only from the unread middle.
        unknown = client.call("prompt/select", session=session, source="unknown", anchor=None, direction="latest")
        self.assertIsNone(unknown["record"])
        self.assertEqual(len(peer.pages), 1, "Codex never exposes an unknown prompt source")
        peer.turns[0]["items"].insert(0, {"type":"userMessage", "id":"user-0",
            "content":[{"type":"text","text":"native user 0 🌍"}]})
        check_native_recall(self, client, session, ["native user 0 🌍"] + [f"native user {i} 🌍" for i in range(140,240)])
        self.assertEqual(len(peer.pages), 3)
        self.assertEqual(fixture.native.prompts, 0)

    def test_codex_native_recall_reads_all_humans_without_transcript_paging(self):
        from test_prompt_history import check_native_recall
        fixture, peer, client, session, opened = self.native_history_fixture("recall")
        self.assertEqual(client.outcome(opened)["state"]["kind"], "accepted")
        check_native_recall(self, client, session, [f"native user {index} 🌍" for index in range(240)])
        self.assertEqual(len(peer.pages), 3)
        self.assertEqual(fixture.native.prompts, 0)

    def native_history_fixture(self, name, empty=()):
        fixture = Fixture("native-history-" + name, codex=True)
        self.addCleanup(fixture.close)
        peer = NativeHistoryWindows(fixture.native)
        for index in empty:
            peer.turns[index]["items"] = []
        client = Client(fixture, "history-reader")
        self.addCleanup(client.close)
        opened = client.operation(1)
        session = client.call("session/open", operation=opened, backend="codex", cwd=str(fixture.root),
                              target={"kind":"resume", "address":fixture.native.thread_id})["session"]
        return fixture, peer, client, session, opened

    def test_codex_opens_the_newest_native_window_before_fetching_older_turns(self):
        fixture, peer, client, session, opened = self.native_history_fixture("newest")
        first = wait_for(lambda: peer.pages and peer.pages[0], "first public native history page")
        self.assertEqual(first["sortDirection"], "desc", "startup must not drain history from the oldest turn")
        self.assertEqual(client.outcome(opened)["state"]["kind"], "accepted")
        snapshot = client.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertEqual(len(peer.pages), 1, "no older native read before a client asks for history")
        self.assertTrue(snapshot["has_older"])
        self.assertEqual(snapshot["entries"][-1]["text"], "native answer 239 🌍")
        self.assertNotIn("PRIVATE_HISTORY", str(snapshot), "native cursors stay private")
        current_ids = {entry["text"]:entry["id"] for entry in snapshot["entries"]}
        collected, page = list(snapshot["entries"]), snapshot
        while page["has_older"]:
            page = client.call("session/history", session=session, before=collected[0]["id"], limit=100)
            self.assertTrue(page["entries"], "a successful older window advances the visible prefix")
            collected = page["entries"] + collected
        expected = [f"native {kind} {index} 🌍" for index in range(240) for kind in ("user", "answer")]
        self.assertEqual([entry["text"] for entry in collected], expected)
        self.assertEqual(len({entry["id"] for entry in collected}), 480)
        self.assertEqual(len(peer.pages), 3)
        self.assertTrue(all(page["itemsView"] == "full" and page["sortDirection"] == "desc" for page in peer.pages))
        again = client.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertEqual({entry["text"]:entry["id"] for entry in again["entries"]}, current_ids)
        self.assertEqual(fixture.native.prompts, 0)
        self.assertFalse(any(request.get("method") == "thread/read" and request["params"].get("includeTurns")
                             for request in fixture.native.requests))

    def native_prefix_boundary(self, client, session, opened):
        self.assertEqual(client.outcome(opened)["state"]["kind"], "accepted")
        snapshot = client.call("session/subscribe", session=session, since=None)["snapshot"]
        prefix = client.call("session/history", session=session, before=snapshot["entries"][0]["id"], limit=100)
        return prefix["entries"][0]["id"]

    def test_codex_skips_empty_native_windows_without_claiming_older_history_is_finished(self):
        fixture, peer, client, session, opened = self.native_history_fixture("empty", empty=range(40, 140))
        before = self.native_prefix_boundary(client, session, opened)
        older = client.call("session/history", session=session, before=before, limit=100)
        self.assertTrue(older["entries"], "native empty turns must not strand the client's older cursor")
        self.assertEqual(older["entries"][0]["text"], "native user 0 🌍")
        self.assertFalse(older["has_older"])
        self.assertEqual(len(peer.pages), 3)
        self.assertEqual(fixture.native.prompts, 0)

    def test_codex_failed_older_read_preserves_the_page_and_retries_the_original_native_cursor(self):
        fixture, peer, client, session, opened = self.native_history_fixture("retry")
        before = self.native_prefix_boundary(client, session, opened)
        peer.fail_cursor = "PRIVATE_HISTORY:desc:100"
        failed = client.response(client.send("session/history", session=session, before=before, limit=100))
        self.assertIn("native history fixture read failed", failed["error"]["message"])
        again = client.call("session/history", session=session, before=before, limit=100)
        self.assertTrue(again["entries"])
        self.assertEqual([page["cursor"] for page in peer.pages[1:]], ["PRIVATE_HISTORY:desc:100"] * 2)
        self.assertEqual(fixture.native.prompts, 0)

    def test_codex_remembers_cursors_skipped_during_the_initial_empty_window(self):
        fixture, peer, client, session, opened = self.native_history_fixture("empty-initial", empty=range(140, 240))
        before = self.native_prefix_boundary(client, session, opened)
        peer.next_override = "PRIVATE_HISTORY:desc:100"
        failed = client.response(client.send("session/history", session=session, before=before, limit=100))
        self.assertIn("cursor", failed["error"]["message"])
        peer.next_override = None
        older = client.call("session/history", session=session, before=before, limit=100)
        self.assertEqual(older["entries"][0]["text"], "native user 0 🌍")
        self.assertFalse(older["has_older"])
        self.assertEqual([page["cursor"] for page in peer.pages[-2:]], ["PRIVATE_HISTORY:desc:200"] * 2)
        self.assertEqual(fixture.native.prompts, 0)

    def test_codex_held_older_history_keeps_live_sends_approval_and_interrupt_responsive(self):
        fixture, peer, reader, session, opened = self.native_history_fixture("live")
        before = self.native_prefix_boundary(reader, session, opened)
        latest = reader.call("session/subscribe", session=session, since=None)["snapshot"]["entries"][-1]
        peer.pause = True
        pending = reader.send("session/history", session=session, before=before, limit=100)
        wait_for(lambda: peer.held, "native older page is held")
        other = Client(fixture, "live-controls")
        self.addCleanup(other.close)
        other.call("session/subscribe", session=session, since=None)
        sent = other.operation(1)
        other.call("session/send", session=session, operation=sent, text="held")
        card = other.message(lambda message: message.get("params", {}).get("event", {}).get("kind") == "approval_requested")["params"]["event"]["approval"]
        approved = other.operation(2)
        other.call("session/approve", session=session, operation=approved, approval=card["id"], answer={"kind":"decision", "decision":"once"})
        turn = other.call("session/subscribe", session=session, since=None)["snapshot"]["info"]["current_turn"]
        interrupted = other.operation(3)
        other.call("session/interrupt", session=session, operation=interrupted, turn=turn)
        for operation in (approved, interrupted, sent):
            self.assertEqual(other.outcome(operation)["state"]["kind"], "accepted")
        self.assertTrue(reader.call("sessions/watch")["sessions"])
        self.assertFalse(any(message.get("id") == pending for message in reader.messages))
        peer.release()
        page = reader.response(pending)["result"]
        self.assertTrue(page["entries"])
        source = other.call("entry/source", session=session, entry=latest["id"], cursor=None, limit=65536)
        self.assertEqual(source["text"], "native answer 239 🌍")
        self.assertEqual((fixture.native.answers, fixture.native.interrupts), (1, 1))
        self.assertEqual(len([request for request in fixture.native.requests if request.get("method") == "turn/start"]), 1)
