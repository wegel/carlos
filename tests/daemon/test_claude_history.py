"""Read complete newest/older Claude windows through the real compiled SDK and daemon."""

from pathlib import Path

from native_claude_history import NativeClaudeHistory
from support import Client, Fixture, wait_for


class ClaudeHistoryTests:
    def test_claude_concurrent_older_readers_keep_their_anchors_and_share_one_window(self):
        fixture, native, reader, session, snapshot = self.claude_history_fixture("concurrent", hold=True)
        before = snapshot["entries"][0]["id"]
        pending = reader.send("session/history", session=session, before=before, limit=50)
        wait_for(lambda: (fixture.root / "history-held").exists(), "original SDK read held")
        other = Client(fixture, "second-claude-history-reader")
        self.addCleanup(other.close)
        waiting = other.send("session/history", session=session, before=before, limit=50)
        other.call("session/subscribe", session=session, since=None)
        self.assertFalse(any(message.get("id") == waiting for message in other.messages),
                         "concurrent SDK read waits rather than failing busy")
        (fixture.root / "release-history").write_text("release")
        first_page = reader.response(pending)["result"]
        self.assertEqual(other.response(waiting)["result"], first_page)
        self.assertEqual(len(native.requests()), 2)
        self.assertEqual(native.path.read_text(), native.original)

    def test_claude_recall_finds_older_unknown_records_without_adopting_automatic_prompts(self):
        from test_prompt_history import check_native_recall
        sources = {index:"auto-continuation" for index in range(150)}
        sources.update({0:"unclassified",40:"unclassified"})
        _, native, client, session, _ = self.claude_history_fixture("recall-unknown", sources=sources)
        self.assertEqual(len(native.requests()), 1)
        check_native_recall(self, client, session, ["native Claude user 0 🌍","native Claude user 40 🌍"], source="unknown")
        human = client.call("prompt/select", session=session, source="human", anchor=None, direction="latest")
        self.assertIsNone(human["record"], "automatic and unclassified records cannot become human recall")
        self.assertEqual(len(native.requests()), 5)
        self.assertEqual(native.path.read_text(), native.original)

    def test_claude_native_recall_reads_all_humans_without_transcript_paging(self):
        from test_prompt_history import check_native_recall
        _, native, client, session, _ = self.claude_history_fixture("recall")
        check_native_recall(self, client, session, [f"native Claude user {index} 🌍" for index in range(150)])
        self.assertEqual(len(native.requests()), 5)
        self.assertEqual(native.path.read_text(), native.original)

    def claude_history_fixture(self, name, hold=False, **options):
        fixture = Fixture("claude-history-" + name)
        self.addCleanup(fixture.close)
        if hold:
            fixture.stop()
            fixture.env["CARLOS_REAL_CLAUDE_CONTROLLER"] = fixture.env["CARLOS_CLAUDE_CONTROLLER"]
            fixture.env["CARLOS_CLAUDE_CONTROLLER"] = str(Path(__file__).with_name("held_claude_history.py"))
            fixture.start()
        native = NativeClaudeHistory(fixture, **options)
        client = Client(fixture, "claude-history-reader")
        self.addCleanup(client.close)
        opened = client.operation(1)
        session = client.call("session/open", operation=opened, backend="claude", cwd=str(fixture.root),
                              target={"kind":"resume", "address":native.session})["session"]
        self.assertEqual(client.outcome(opened)["state"]["kind"], "accepted")
        snapshot = wait_for(lambda: value if (value := client.call("session/subscribe", session=session, since=None)["snapshot"])
                            ["info"]["availability"] == "ready" else None, "newest SDK window is adopted before Ready")
        return fixture, native, client, session, snapshot

    def check_claude_traversal(self, native, client, session, snapshot, expected_pages=5):
        newest = {row["id"] for row in snapshot["entries"]}
        collected, page = snapshot["entries"], snapshot
        while page["has_older"]:
            page = client.call("session/history", session=session, before=collected[0]["id"], limit=100)
            self.assertTrue(page["entries"] or not page["has_older"], "an empty native window must not strand the public cursor")
            collected = page["entries"] + collected
        self.assertEqual([row["text"] for row in collected if row["kind"] not in ("tool", "tool_call")], native.expected_text())
        self.assertEqual(len({row["id"] for row in collected}), len(collected))
        current = client.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertTrue(newest.issubset({row["id"] for row in current["entries"]}))
        self.assertEqual(native.path.read_text(), native.original, "Carlos must not rewrite SDK history")
        self.assertEqual(len(native.requests()), expected_pages)
        return collected

    def test_claude_opens_one_newest_sdk_window_and_keeps_all_older_rows(self):
        _, native, client, session, snapshot = self.claude_history_fixture("newest")
        self.assertTrue(native.requests()[0]["params"].get("newest"), "startup must not drain from the first record")
        self.assertEqual(len(native.requests()), 1)
        self.assertEqual(len(snapshot["entries"]), 64)
        self.assertEqual(snapshot["entries"][-1]["text"], "native Claude assistant 149 🌍")
        self.assertTrue(snapshot["has_older"])
        self.assertEqual(len(self.check_claude_traversal(native, client, session, snapshot)), 300)

    def test_claude_keeps_boundary_tool_context_and_exact_result_source(self):
        _, native, client, session, snapshot = self.claude_history_fixture("tool", boundary_tool=True)
        self.assertEqual(len(native.requests()), 2, "a whole boundary tool needs65 native records")
        rows = self.check_claude_traversal(native, client, session, snapshot, expected_pages=6)
        tool = next(row for row in rows if row["kind"] == "tool")
        self.assertEqual(tool["tool"]["kind"], "read")
        self.assertEqual(client.call("entry/source", session=session, entry=tool["id"], cursor=None, limit=65536)["text"],
                         "exact native cross-window source 🌍")

    def test_claude_child_only_newest_window_keeps_reading_until_main_history_is_ready(self):
        _, native, client, session, snapshot = self.claude_history_fixture("child-tail", hidden=range(236, 300))
        self.assertEqual(len(native.requests()), 2)
        self.assertTrue(snapshot["entries"], "Ready must not strand an empty main transcript with older data")
        self.assertEqual(snapshot["entries"][-1]["text"], "native Claude assistant 117 🌍")
        self.assertEqual(len(self.check_claude_traversal(native, client, session, snapshot)), 236)

    def test_claude_child_only_older_window_cannot_return_an_empty_successful_continuation(self):
        _, native, client, session, snapshot = self.claude_history_fixture("child-middle", hidden=range(108, 172))
        self.assertEqual(len(self.check_claude_traversal(native, client, session, snapshot)), 236)

    def test_claude_failed_older_snapshot_keeps_the_page_and_retries_the_original_scope(self):
        fixture, native, client, session, snapshot = self.claude_history_fixture("retry", hold=True)
        before = snapshot["entries"][0]["id"]
        pending = client.send("session/history", session=session, before=before, limit=100)
        wait_for(lambda: (fixture.root / "history-held").exists(), "older SDK reply held")
        (fixture.root / "fail-history").write_text("fail")
        (fixture.root / "release-history").write_text("release")
        self.assertIn("snapshot expired", client.response(pending)["error"]["message"])
        unchanged = client.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertEqual(unchanged["entries"], snapshot["entries"])
        (fixture.root / "fail-history").unlink()
        retry = client.call("session/history", session=session, before=before, limit=100)
        self.assertTrue(retry["entries"])
        self.assertEqual(native.requests()[-2:][0]["params"], native.requests()[-1]["params"])

    def test_claude_held_history_keeps_cached_reads_sends_approvals_and_interrupt_live(self):
        fixture, native, reader, session, snapshot = self.claude_history_fixture("live", hold=True)
        pending = reader.send("session/history", session=session, before=snapshot["entries"][0]["id"], limit=100)
        wait_for(lambda: (fixture.root / "history-held").exists(), "older SDK read held")
        other = Client(fixture, "live-claude-controls")
        self.addCleanup(other.close)
        other.call("session/subscribe", session=session, since=None)
        cached = other.call("session/history", session=session, before=snapshot["entries"][-1]["id"], limit=10)
        self.assertEqual(len(cached["entries"]), 10)
        sent = other.operation(1)
        other.call("session/send", session=session, operation=sent, text="native plan")
        self.assertEqual(other.outcome(sent)["state"]["kind"], "accepted")
        card = other.message(lambda message: message.get("params", {}).get("event", {}).get("kind") == "approval_requested")["params"]["event"]["approval"]
        approved = other.operation(2)
        other.call("session/approve", session=session, operation=approved, approval=card["id"], answer={"kind":"decision", "decision":"once"})
        self.assertEqual(other.outcome(approved)["state"]["kind"], "accepted")
        wait_for(lambda: other.call("session/subscribe", session=session, since=None)["snapshot"]["info"]["phase"] == "idle", "native plan settled")
        held = other.operation(3)
        other.call("session/send", session=session, operation=held, text="hold this turn")
        self.assertEqual(other.outcome(held)["state"]["kind"], "accepted")
        live = wait_for(lambda: value if (value := other.call("session/subscribe", session=session, since=None)["snapshot"])
                        ["info"]["current_turn"] is not None else None, "live native turn beside older read")
        interrupted = other.operation(4)
        other.call("session/interrupt", session=session, operation=interrupted, turn=live["info"]["current_turn"])
        self.assertEqual(other.outcome(interrupted)["state"]["kind"], "accepted")
        self.assertFalse(any(message.get("id") == pending for message in reader.messages))
        (fixture.root / "release-history").write_text("release")
        self.assertTrue(reader.response(pending)["result"]["entries"])
        self.assertEqual(len(native.requests()), 2)
