"""Preserve exact original text through the real daemon without changing normalized watches."""

from support import Client, Fixture, wait_for


class SourceTests:
    def test_source_roles_steers_and_diff_paths_survive_the_actual_socket(self):
        fixture, first, second, session = self.clients("source-formats")
        operation = first.operation(2)
        first.call("session/send", session=session, operation=operation, text="format-facts")
        first.outcome(operation)
        def formatted():
            snapshot = second.call("session/subscribe", session=session, since=None)["snapshot"]
            return snapshot if any(row.get("format") == "diff" for row in snapshot["entries"]) else None
        snapshot = wait_for(formatted, "native source formats published")
        steer = next(row for row in snapshot["entries"] if row["text"] == "wire second")
        self.assertEqual((steer["kind"], steer["format"]), ("user", "user_steer"))
        call = next(row for row in snapshot["entries"] if row["kind"] == "tool_call")
        self.assertTrue(first.call("entry/source", session=session, entry=call["id"], cursor=None, limit=65536)["text"])
        diff = next(row for row in snapshot["entries"] if row.get("format") == "diff")
        self.assertEqual(diff["kind"], "tool")
        self.assertEqual(diff["file_path"], "src/é.rs")
        self.assertEqual(diff["text"], "")
        self.assertEqual(first.call("entry/source", session=session, entry=diff["id"], cursor=None, limit=65536)["text"], "@@ -1 +1 @@\n-old\n+new 🌍\n")

    def test_compact_native_tool_facts_keep_outcomes_counts_and_original_source(self):
        from native_tool_facts import COMMAND, COMMAND_SOURCE, DIFF, configure
        fixture, first, second, session = self.clients("phone-tool-facts")
        configure(fixture.native)
        sent = first.operation(2)
        first.call("session/send", session=session, operation=sent, text="tool-facts")
        first.outcome(sent)

        def ready():
            rows = second.call("session/subscribe", session=session, since=None)["snapshot"]["entries"]
            command = next((row for row in rows if COMMAND in (row.get("tool") or {}).get("label", "")), None)
            diff = next((row for row in rows if row.get("file_path") == "src/é.rs"), None)
            unknown = next((row for row in rows if "observed-only" in (row.get("tool") or {}).get("label", "")), None)
            return (command, diff, unknown) if command and diff and unknown and all(row["done"] for row in (command, diff, unknown)) else None

        command, diff, unknown = wait_for(ready, "complete native command/diff facts")
        self.assertEqual(command["tool"]["kind"], "command")
        self.assertEqual(command["tool"]["status"], "ok")
        self.assertEqual(command["tool"]["summary"], "2 lines")
        self.assertEqual((diff["tool"]["kind"], diff["tool"]["added"], diff["tool"]["removed"]), ("edit", 1, 1))
        self.assertEqual(unknown["tool"]["status"], "unknown", "completion alone does not prove success")
        for row, expected in ((command, COMMAND_SOURCE), (diff, DIFF)):
            self.assertEqual(row["text"], "")
            self.assertEqual(first.call("entry/source", session=session, entry=row["id"], cursor=None, limit=65536)["text"], expected)
        self.assertFalse(any(message.get("method") in ("entry/source", "entry/output") for message in second.messages))

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

    def test_grok_tool_kinds_and_outcomes_survive_the_actual_native_process(self):
        expected_output = "done 🌍\n"
        fixture = Fixture("grok-tool-facts", mode="grok-tool-facts")
        self.addCleanup(fixture.close)
        first = Client(fixture, "reader")
        second = Client(fixture, "collapsed")
        self.addCleanup(first.close)
        self.addCleanup(second.close)
        operation = first.operation(1)
        session = first.call("session/create", operation=operation, backend="grok", cwd=str(fixture.root), name=None)["session"]
        first.outcome(operation)
        second.call("session/subscribe", session=session, since=None)
        sent = first.operation(2)
        first.call("session/send", session=session, operation=sent, text="native facts")
        first.outcome(sent)

        def ready():
            snapshot = second.call("session/subscribe", session=session, since=None)["snapshot"]
            rows = [row for row in snapshot["entries"] if row.get("tool")]
            return rows if len(rows) == 4 and snapshot["info"]["current_turn"] is None else None

        rows = wait_for(ready, "all real ACP tool updates settled")
        read, command_call, command, search_call = rows
        self.assertEqual((read["tool"]["kind"], read["tool"]["status"]), ("read", "ok"))
        for row in (command_call, command):
            self.assertEqual((row["tool"]["kind"], row["tool"]["status"]), ("command", "ok"))
        self.assertEqual((search_call["tool"]["kind"], search_call["tool"]["status"]), ("search", "failed"))
        self.assertFalse(read["tool"]["has_output"], "read-body visibility stays unchanged")
        for row in (command,):
            self.assertEqual(row["text"], "")
            self.assertEqual(row["tool"]["summary"], "1 line")
            self.assertEqual(first.call("entry/source", session=session, entry=row["id"], cursor=None, limit=65536)["text"], expected_output)
        self.assertFalse(any(message.get("method") in ("entry/source", "entry/output") for message in second.messages))

    def test_source_and_normalized_watches_keep_independent_cursors_and_split_terminal_controls(self):
        fixture, first, second, session = self.clients("source-watches")
        fixture.native.tool_first = "\x1b[3"
        fixture.native.tool_second = "1m🌍red\x1b[0m\n"
        send = first.operation(2)
        first.call("session/send", session=session, operation=send, text="held")
        card = first.message(lambda m: m.get("method") == "session/event" and m["params"]["event"]["kind"] == "approval_requested")["params"]["event"]["approval"]
        snapshot = second.call("session/subscribe", session=session, since=None)["snapshot"]
        entry = first.message(lambda m: m.get("method") == "session/event" and m["params"]["event"]["kind"] in ("entry_added", "entry_updated") and (m["params"]["event"]["entry"].get("tool") or {}).get("has_output"))["params"]["event"]["entry"]["id"]
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
