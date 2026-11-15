"""Read exact immutable prompt records and distinguish an adopted fork from a failed fork."""

import argparse
import asyncio
from pathlib import Path
import unittest

import support
from support import Client, Fixture, wait_for


class PromptHistoryTests:
    def prompt_fixture(self, name):
        fixture = Fixture(name, codex=True)
        self.addCleanup(fixture.close)
        fixture.native.history_user = True
        first, other = Client(fixture, "prompt-reader"), Client(fixture, "independent-reader")
        self.addCleanup(first.close)
        self.addCleanup(other.close)
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="codex", cwd=str(fixture.root), name=None)["session"]
        first.outcome(create)
        wait_for(lambda:first.call("session/subscribe", session=session, since=None)["snapshot"]["info"]["availability"] == "ready", "native history adopted")
        return fixture, first, other, session

    @staticmethod
    def prompt_selection(client, session, anchor=None, direction="latest", source="human"):
        return client.call("prompt/select", session=session, source=source, anchor=anchor, direction=direction)

    def test_prompt_cursors_read_older_records_without_transcript_pages_and_keep_readers_independent(self):
        fixture, first, other, session = self.prompt_fixture("prompt-cursors")
        snapshot = first.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertFalse(any(entry["kind"] == "user" for entry in snapshot["entries"]), "original prompt is outside the newest page")
        original = self.prompt_selection(first, session)["record"]
        self.assertNotIn("native-user", str(original))
        text = "🌍日本語\n" * 12000
        send = first.operation(2)
        first.call("session/send", session=session, operation=send, text=text)
        self.assertEqual(first.outcome(send)["state"]["kind"], "accepted")
        latest = wait_for(lambda:(page if page["record"]["id"] != original["id"] else None)
                          if (page := self.prompt_selection(other, session)) else None, "second original human record")
        self.assertTrue(latest["has_older"])
        older = self.prompt_selection(first, session, latest["record"]["id"], "older")
        self.assertEqual(older["record"]["id"], original["id"])
        self.assertEqual(self.prompt_selection(other, session)["record"]["id"], latest["record"]["id"])
        offset, copied, pieces = "0", "", 0
        while True:
            chunk = other.call("prompt/text", session=session, prompt=latest["record"]["id"], offset=offset, limit=65536)
            self.assertEqual(chunk["offset"], offset)
            self.assertLessEqual(len(chunk["text"].encode()), 65536)
            copied += chunk["text"]
            pieces += 1
            if chunk["next"] is None:
                break
            offset = chunk["next"]
        self.assertGreater(pieces, 1)
        self.assertEqual(copied, text)
        empty = self.prompt_selection(first, session, latest["record"]["id"], "newer")
        self.assertIsNone(empty["record"])
        invalid = first.response(first.send("prompt/text", session=session, prompt=latest["record"]["id"], offset="1", limit=4))
        self.assertIn("error", invalid)
        unknown = self.prompt_selection(first, session, source="unknown")
        self.assertIsNone(unknown["record"])

    def test_retired_prompt_address_cannot_read_or_edit_the_replacement_branch(self):
        fixture, first, other, session = self.prompt_fixture("prompt-retired")
        record = self.prompt_selection(first, session)["record"]
        edit = first.operation(2)
        first.call("session/rewind", session=session, operation=edit, entry=record["id"], text="new branch")
        result = first.outcome(edit)
        self.assertEqual(result["state"]["kind"], "accepted")
        self.assertTrue(result["rewound"])
        stale = other.response(other.send("prompt/text", session=session, prompt=record["id"], offset="0", limit=4))
        self.assertIn("error", stale)
        retired = other.operation(1)
        other.call("session/rewind", session=session, operation=retired, entry=record["id"], text="never fork")
        self.assertEqual(other.outcome(retired)["state"]["kind"], "rejected")
        self.assertEqual(sum(request.get("method") == "thread/fork" for request in fixture.native.requests), 1)

    def test_private_failure_reports_whether_this_exact_operation_adopted_a_fork(self):
        for failure in ("thread/fork", "turn/start"):
            with self.subTest(failure=failure):
                fixture, first, other, session = self.prompt_fixture("prompt-progress-" + failure.replace("/", "-"))
                handle = fixture.native.handle
                async def reject(link, request, handle=handle, failure=failure, native=fixture.native):
                    if request.get("method") == failure:
                        await native.send(link, {"id":request["id"], "error":{"code":-32602,"message":"explicit fixture refusal"}})
                    else:
                        await handle(link, request)
                fixture.native.handle = reject
                record = self.prompt_selection(first, session)["record"]
                edit = first.operation(2)
                first.call("session/rewind", session=session, operation=edit, entry=record["id"], text="private edited draft")
                result = first.outcome(edit)
                self.assertIn(result["state"]["kind"], ("rejected", "uncertain"))
                self.assertEqual(result.get("rewound", False), failure == "turn/start")
                resumed = Client(fixture, "same-editor", resume=first.hello)
                self.addCleanup(resumed.close)
                self.assertEqual(resumed.call("operation/read", operation=edit), result)
                forbidden = other.response(other.send("operation/read", operation=edit))
                self.assertEqual(forbidden["error"]["code"], -32003)

    def test_a_rejected_steer_does_not_leave_an_unbound_prompt_in_recall(self):
        fixture, first, other, session = self.prompt_fixture("prompt-rejected-steer")
        original = self.prompt_selection(first, session)["record"]
        handle = fixture.native.handle
        async def reject(link, request):
            if request.get("method") == "turn/steer":
                await fixture.native.send(link, {"id":request["id"], "error":{"code":-32602,"message":"explicit steer refusal"}})
            else:
                await handle(link, request)
        fixture.native.handle = reject
        async def started():
            for link in list(fixture.native.connections):
                await fixture.native.notify(link, "turn/started", turn={"id":"held-for-steer"})
        asyncio.run_coroutine_threadsafe(started(), fixture.native.loop).result(timeout=3)
        wait_for(lambda:first.call("session/subscribe", session=session, since=None)["snapshot"]["info"]["current_turn"], "native turn running")
        steer = first.operation(2)
        first.call("session/send", session=session, operation=steer, text="never accepted human", policy="steer")
        self.assertIn(first.outcome(steer)["state"]["kind"], ("rejected", "uncertain"))
        self.assertEqual(self.prompt_selection(other, session)["record"]["id"], original["id"])


if __name__ == "__main__":
    class StandalonePromptHistoryTests(PromptHistoryTests, unittest.TestCase):
        pass

    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path, required=True)
    args, tests = parser.parse_known_args()
    support.BINARY, support.ARTIFACTS = args.binary.resolve(), args.artifacts.resolve()
    unittest.main(argv=[__file__, *tests])
