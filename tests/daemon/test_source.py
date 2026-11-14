"""Preserve exact original text through the real daemon without changing normalized watches."""

from support import Client, Fixture, wait_for


class SourceTests:
    def clients(self, name):
        fixture = Fixture(name, codex=True)
        self.addCleanup(fixture.close)
        first = Client(fixture, "source-reader")
        second = Client(fixture, "collapsed")
        self.addCleanup(first.close)
        self.addCleanup(second.close)
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="codex", cwd=str(fixture.root), name=None)["session"]
        first.outcome(create)
        first.call("session/subscribe", session=session, since=None)
        second.call("session/subscribe", session=session, since=None)
        return fixture, first, second, session

    def test_source_and_normalized_watches_keep_independent_cursors_and_split_terminal_controls(self):
        fixture, first, second, session = self.clients("source-watches")
        fixture.native.tool_first = "\x1b[3"
        fixture.native.tool_second = "1m🌍red\x1b[0m\n"
        send = first.operation(2)
        first.call("session/send", session=session, operation=send, text="held")
        card = first.message(lambda m: m.get("method") == "session/event" and m["params"]["event"]["kind"] == "approval_requested")["params"]["event"]["approval"]
        snapshot = second.call("session/subscribe", session=session, since=None)["snapshot"]
        entry = first.message(lambda m: m.get("method") == "session/event" and m["params"]["event"]["kind"] == "entry_added" and m["params"]["event"]["entry"]["kind"] == "tool")["params"]["event"]["entry"]["id"]
        source = first.call("entry/source/subscribe", session=session, entry=entry, cursor=None)
        normalized = first.call("entry/output/subscribe", session=session, entry=entry, cursor=None)
        self.assertEqual(source["text"], fixture.native.tool_first)
        self.assertEqual(normalized["segments"], [])
        for method, cursor in (("entry/source", normalized["next"]), ("entry/output", source["next"])):
            rejected = first.response(first.send(method, session=session, entry=entry, cursor=cursor, limit=65536))
            self.assertIn("invalid", rejected["error"]["message"])
        first.call("session/interrupt", session=session, operation=first.operation(3), turn=snapshot["info"]["current_turn"])
        first.call("session/approve", session=session, operation=first.operation(4), approval=card["id"], answer={"kind": "decision", "decision": "once"})
        first.outcome(send)
        first.call("session/send", session=session, operation=first.operation(5), text="second")
        update = first.message(lambda m: m.get("method") == "entry/source" and "red" in m["params"]["chunk"]["text"])["params"]["chunk"]
        self.assertEqual(update["from"], source["next"])
        self.assertEqual(source["text"] + update["text"], fixture.native.tool_first + fixture.native.tool_second)
        output = first.message(lambda m: m.get("method") == "entry/output" and any("red" in p["text"] for p in m["params"]["chunk"]["segments"]))["params"]["chunk"]
        self.assertEqual("".join(p["text"] for p in output["segments"]), "🌍red\n")
        first.call("entry/output/unsubscribe", session=session, entry=entry)
        resumed = first.call("entry/source/subscribe", session=session, entry=entry, cursor=update["next"])
        self.assertEqual(resumed["text"], "")
        second.call("sessions/watch")
        self.assertFalse(any(m.get("method") in ("entry/source", "entry/output") for m in second.messages))

    def test_plain_body_above_frame_limit_remains_readable_in_bounded_exact_chunks(self):
        fixture, first, second, session = self.clients("large-plain-source")
        operation = first.operation(2)
        first.call("session/send", session=session, operation=operation, text="large-plain-source")
        first.outcome(operation)
        expected = "\0🌍\x1b[31m\n" * (4096*224)
        def ready():
            snapshot = second.call("session/subscribe", session=session, since=None)["snapshot"]
            return next((entry for entry in snapshot["entries"] if entry.get("source", {}).get("bytes") == str(len(expected.encode())) and entry["done"]), None)
        entry = wait_for(ready, "large original body complete", timeout=15)
        self.assertEqual(entry["text"], "")
        seek = first.call("entry/source/seek", session=session, entry=entry["id"], revision=entry["source"]["revision"], offset=str(len(expected.encode())-11), limit=65536)
        self.assertEqual(seek["text"], "\0🌍\x1b[31m\n")
        self.assertEqual(int(seek["offset"]), len(expected.encode())-11)
        self.assertTrue(seek["done"])
        invalid = first.response(first.send("entry/source/seek", session=session, entry=entry["id"], revision=entry["source"]["revision"], offset="18446744073709551615", limit=65536))
        self.assertIn("invalid", invalid["error"]["message"])
        cursor = None
        parts = []
        while True:
            chunk = first.call("entry/source", session=session, entry=entry["id"], cursor=cursor, limit=4294967295)
            self.assertLessEqual(len(chunk["text"].encode()), 65536)
            parts.append(chunk["text"])
            cursor = chunk["next"]
            if not chunk["has_more"]:
                self.assertTrue(chunk["done"])
                break
        self.assertEqual("".join(parts), expected)
        self.assertGreater(len(expected.encode()), 8*1024*1024)
