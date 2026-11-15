"""Observe accepted native settings through the real daemon's shared event stream."""

import asyncio

from support import Client, Fixture, wait_for


class SettingsFactTests:
    def test_native_settings_reach_both_clients_without_exposing_or_changing_editors(self):
        fixture = Fixture("shared-settings", codex=True)
        self.addCleanup(fixture.close)
        first, second = Client(fixture, "first"), Client(fixture, "second")
        self.addCleanup(first.close)
        self.addCleanup(second.close)
        operation = first.operation(1)
        session = first.call("session/create", operation=operation, backend="codex",
                             cwd=str(fixture.root), name=None)["session"]
        self.assertEqual(first.outcome(operation)["state"]["kind"], "accepted")
        a = first.call("session/subscribe", session=session, since=None)["snapshot"]
        b = second.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertEqual(a["settings"], b["settings"])

        async def announce():
            for link in list(fixture.native.connections):
                await fixture.native.notify(link, "thread/settings/updated", threadSettings={
                    "model":"native-new-model", "effort":"high", "summary":None})

        asyncio.run_coroutine_threadsafe(announce(), fixture.native.loop).result(timeout=5)

        def changed(message):
            event = message.get("params", {}).get("event", {})
            return (message.get("method") == "session/event"
                    and event.get("kind") == "settings_changed"
                    and event["settings"]["current"]["model"] == "native-new-model")

        left, right = first.message(changed), second.message(changed)
        self.assertEqual(left, right)
        facts = left["params"]["event"]["settings"]
        self.assertEqual(facts["current"], {"model":"native-new-model", "effort":"high", "summary":None})
        self.assertEqual(facts["pending"], {"model":None, "effort":None, "summary":None})
        self.assertNotEqual(facts["revision"], a["settings"]["revision"])
        self.assertNotIn("editor", facts)
        self.assertNotIn("draft", facts)

        async def unrelated():
            for link in list(fixture.native.connections):
                await fixture.native.send(link, {"method":"thread/settings/updated", "params":{
                    "threadId":"foreign-child", "threadSettings":{"model":"DO_NOT_ADOPT"}}})
                await fixture.native.notify(link, "item/agentMessage/delta",
                                            itemId="settings-barrier", delta="settings fact barrier")

        asyncio.run_coroutine_threadsafe(unrelated(), fixture.native.loop).result(timeout=5)

        def barrier():
            page = first.call("session/subscribe", session=session, since=None)["snapshot"]
            return page if any("settings fact barrier" in row["text"] for row in page["entries"]) else None

        page = wait_for(barrier, "ordinary native text follows the foreign settings notice")
        self.assertEqual(page["settings"], facts)
        self.assertFalse(any(request.get("method") in ("turn/start", "turn/steer", "turn/interrupt",
                                                        "thread/settings/update", "turn/settings/update")
                             for request in fixture.native.requests))
