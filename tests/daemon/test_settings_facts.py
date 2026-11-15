"""Observe accepted native settings through the real daemon's shared event stream."""

import asyncio

from support import Client, Fixture, wait_for


class SettingsFactTests:
    def test_model_catalog_pages_are_independent_and_fenced_by_native_settings(self):
        fixture = Fixture("model-pages", codex=True)
        self.addCleanup(fixture.close)
        fixture.native.models = [{
            "model":f"model-{index:03}", "displayName":f"Model {index} 🌍",
            "isDefault":index == 0, "defaultReasoningEffort":"low",
            "supportedReasoningEfforts":[{"reasoningEffort":"low"}, {"reasoningEffort":"high"}],
        } for index in range(170)]
        first, second = Client(fixture, "first"), Client(fixture, "second")
        self.addCleanup(first.close)
        self.addCleanup(second.close)
        operation = first.operation(1)
        session = first.call("session/create", operation=operation, backend="codex",
                             cwd=str(fixture.root), name=None)["session"]
        self.assertEqual(first.outcome(operation)["state"]["kind"], "accepted")
        first.call("session/subscribe", session=session, since=None)
        second.call("session/subscribe", session=session, since=None)

        def catalog():
            page = first.call("settings/read", session=session)
            return page if page["settings"]["model_count"] == 170 else None

        wait_for(catalog, "startup catalog is available through bounded typed reads")
        startup_boundary = self.startup_boundary(first, fixture, session)

        # A newer native notice fences late startup replies before comparing independent pages.
        self.announce_settings(fixture, {"model":"model-000", "effort":"high"})

        def current_catalog():
            page = catalog()
            return page if page and page["settings"]["current"]["model"] == "model-000" else None

        page = wait_for(current_catalog, "newer native choices fence startup catalog reads")
        self.assertEqual((page["offset"], page["next"], len(page["models"])), (0, 32, 32))
        self.assertEqual(page["models"][0]["display_name"], "Model 0 🌍")
        self.assertEqual(page["models"][0]["supported_efforts"], ["low", "high"])
        expected = page["settings"]["revision"]
        tail = second.call("settings/read", session=session, expected=expected, offset=128, limit=65535)
        self.assertEqual((tail["offset"], tail["next"], len(tail["models"])), (128, None, 42))
        self.assertEqual(tail["models"][-1]["model"], "model-169")
        repeated = first.call("settings/read", session=session, expected=expected)
        self.assertEqual(repeated["models"], page["models"])
        self.assertEqual(repeated["settings"], page["settings"])
        beyond = first.response(first.send("settings/read", session=session, expected=expected, offset=171))
        self.assertIn("offset", beyond["error"]["message"])

        self.announce_settings(fixture, {"model":"model-169", "effort":"high"})

        def newer():
            snapshot = first.call("session/subscribe", session=session, since=None)["snapshot"]
            return snapshot if snapshot["settings"]["current"]["model"] == "model-169" else None

        wait_for(newer, "native settings replace the catalog precondition")
        stale = second.response(second.send("settings/read", session=session, expected=expected, offset=32))
        self.assertIn("settings changed", stale["error"]["message"])
        self.assertEqual(len([r for r in fixture.native.requests if r.get("method") == "model/list"]), 1)
        self.assertFalse(any(r.get("method") == "turn/start" for r in fixture.native.requests))
        self.assertFalse(any(r.get("method") == "thread/settings/update"
                             for r in fixture.native.requests[startup_boundary:]))

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
        startup_boundary = self.startup_boundary(first, fixture, session)
        a = first.call("session/subscribe", session=session, since=None)["snapshot"]
        b = second.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertEqual(a["settings"], b["settings"])

        self.announce_settings(fixture, {"model":"native-new-model", "effort":"high", "summary":None})

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
        self.assertFalse(any(request.get("method") in ("turn/start", "turn/steer", "turn/interrupt")
                             for request in fixture.native.requests))
        self.assertFalse(any(request.get("method") in ("thread/settings/update", "turn/settings/update")
                             for request in fixture.native.requests[startup_boundary:]))

    def startup_boundary(self, client, fixture, session):
        def settled():
            snapshot = client.call("session/subscribe", session=session, since=None)["snapshot"]
            settings = snapshot["settings"]
            return (snapshot["info"]["availability"] == "ready"
                    and not settings["native_pending"] and not any(settings["pending"].values()))
        wait_for(settled, "initial native settings finish before later read-only assertions")
        return len(fixture.native.requests)


    def announce_settings(self, fixture, choices):
        async def announce():
            for link in list(fixture.native.connections):
                await fixture.native.notify(link, "thread/settings/updated", threadSettings=choices)
        asyncio.run_coroutine_threadsafe(announce(), fixture.native.loop).result(timeout=5)
