"""Keep hundreds of socket-backed source rows reachable across source eviction and older pages."""


class DaemonHistory:
    def test_daemon_terminal_fetches_native_older_windows_without_resetting_its_anchor(self):
        import json
        import sys
        from test_daemon_client import daemon_support
        from native_history_windows import NativeHistoryWindows
        runner = sys.modules[type(self).__module__]
        daemon_support.BINARY = runner.BINARY.parent / "carlosd"
        daemon_support.ARTIFACTS = runner.ARTIFACTS / "daemon"
        fixture = daemon_support.Fixture("terminal-native-history", codex=True)
        self.addCleanup(fixture.close)
        peer = NativeHistoryWindows(fixture.native)
        client = daemon_support.Client(fixture, "history-owner")
        self.addCleanup(client.close)
        operation = client.operation(1)
        session = client.call("session/open", operation=operation, backend="codex", cwd=str(fixture.root),
                              target={"kind":"resume", "address":fixture.native.thread_id})["session"]
        client.outcome(operation)
        terminal = self.attach_daemon(fixture, session, "native-history")
        terminal.contains("native answer 239 🌍")
        for count, anchor in enumerate((190, 140, 90, 40), 1):
            terminal.send(b"\x1b[H")
            terminal.wait(lambda: len(terminal.requests("session/history")) == count, "one explicit older client page")
            terminal.wait(lambda: sum(isinstance(message.get("result"), dict) and "has_older" in message["result"]
                          for message in terminal.messages("server")) == count, "older page reached the client")
            terminal.resize(90 if count % 2 else 100, 40)
            terminal.contains(f"native user {anchor} 🌍")
        terminal.send(b"\x1b[H")
        terminal.contains("native user 0 🌍")
        self.assertEqual(len(peer.pages), 3)
        terminal.snapshot("oldest-native-window-with-preserved-anchor")
        terminal.send(b"\x1b[F")
        terminal.contains("native answer 239 🌍")
        terminal.wait(lambda: terminal.input_is(""), "history never adds text to the composer")
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertNotIn("PRIVATE_HISTORY", json.dumps(terminal.messages("server")))
        self.assertEqual(fixture.native.prompts, 0)
        terminal.snapshot("newest-native-window-after-complete-older-traversal")

    @staticmethod
    def many_tool_history():
        items = [{"type":"commandExecution", "id":f"tool-{index}",
                  "command":f"printf row-{index:03}",
                  "aggregatedOutput":f"source row-{index:03} unique 🌍\n",
                  "status":"completed", "exitCode":0} for index in range(200)]
        return {"id":"native-root", "status":{"type":"idle"}, "model":"fixture-model",
                "reasoningEffort":"low", "turns":[{"id":"history-turn", "status":"completed", "items":items}]}

    def test_daemon_terminal_preserves_two_hundred_sources_and_prepend_anchor_with_thirty_two_windows(self):
        import json
        import re
        fixture, client, session = self.daemon_session("terminal-many-sources", self.many_tool_history)
        terminal = self.attach_daemon(fixture, session, "many-sources")
        terminal.contains("source row-199 unique")
        terminal.paste("/tools full")
        terminal.send(b"\r")
        terminal.wait(lambda: terminal.input_is(""), "full source pinned through phone resize")
        seen = set()
        def collect():
            seen.update(int(row) for row in re.findall(r"source row-(\d+) unique", terminal.text))
        collect()
        terminal.send(b"\x1b[H")
        terminal.contains("source row-100 unique")
        terminal.wait(lambda: len(terminal.requests("session/history")) == 1, "one older page requested")
        terminal.wait(lambda: any(message.get("result",{}).get("entries",[{}])[0].get("id")
                      for message in terminal.messages("server") if isinstance(message.get("result"),dict)
                      and "has_older" in message["result"]), "older page returned")
        # Force a later layout pass after the page arrived, with a different width.
        terminal.resize(90,40)
        terminal.pump(.05)
        terminal.contains("source row-100 unique")
        collect()
        terminal.snapshot("preserved-prepend-anchor")
        terminal.send(b"\x1b[H")
        terminal.contains("source row-000 unique")
        collect()
        terminal.snapshot("oldest-source")
        for step in range(300):
            terminal.send(b"\x1b[6~")
            terminal.pump(.025)
            collect()
            # A PageDown may initially land on estimated rows; let those bodies finish before moving.
            if "loading source" in terminal.text.lower() or "loading visible" in terminal.text.lower():
                terminal.wait(lambda: "loading source" not in terminal.text.lower()
                              and "loading visible" not in terminal.text.lower(), "visible sources loaded")
                collect()
            if "source row-199 unique" in terminal.text:
                break
        self.assertEqual(seen, set(range(200)), "every original source must appear while paging")
        # Seeing the final body's text can precede reaching its trailing rows.
        # Enter follow-bottom explicitly before checking the phone resize.
        terminal.send(b"\x1b[F")
        terminal.contains("source row-199 unique")
        terminal.resize(28,32)
        terminal.contains("source row-199 unique")
        terminal.send(b"\x1b[H")
        terminal.contains("source row-000 unique")
        self.assertEqual(len(terminal.requests("session/history")),1)
        self.assertEqual(terminal.requests("session/send"),[])
        active = set()
        high_water = 0
        for request in terminal.messages("client"):
            method = request.get("method")
            if method == "entry/source/subscribe":
                active.add(request["params"]["entry"])
                high_water = max(high_water,len(active))
            elif method == "entry/source/unsubscribe":
                active.discard(request["params"]["entry"])
        self.assertLessEqual(high_water,32)
        self.assertGreater(len(terminal.requests("entry/source/unsubscribe")),0)
        self.assertEqual({r["params"]["entry"] for r in terminal.requests("entry/source/subscribe")},
                         {entry["id"] for entry in client.call("session/history",session=session,
                             before=client.call("session/subscribe",session=session,since=None)
                             ["snapshot"]["entries"][0]["id"],limit=100)["entries"]} |
                         {entry["id"] for entry in client.call("session/subscribe",session=session,since=None)
                             ["snapshot"]["entries"]})
        (terminal.root/"cache-metrics.json").write_text(json.dumps({"source_rows":len(seen),
            "source_window_high_water":high_water,"history_requests":len(terminal.requests("session/history"))},indent=2)+"\n")
        terminal.snapshot("oldest-source-phone-width")

    def test_daemon_terminal_keeps_the_accepted_prefix_while_live_source_bytes_are_delayed(self):
        import asyncio
        from test_daemon_client import daemon_support
        fixture, client, session = self.daemon_session("terminal-live-prefix")
        body = "old line 🌍\n"*6000 + "accepted prefix 🌍"
        async def running(link, request):
            await fixture.native.notify(link,"turn/started",turn={"id":"prefix-turn"})
            await fixture.native.notify(link,"item/agentMessage/delta",turnId="prefix-turn",
                                        itemId="prefix-source",delta=body)
        fixture.native.prompt = running
        operation = client.operation(2)
        client.call("session/send",session=session,operation=operation,text="live prefix")
        client.outcome(operation)
        daemon_support.wait_for(lambda:any(int(entry.get("source",{}).get("bytes",0)) == len(body.encode())
            for entry in client.call("session/subscribe",session=session,since=None)["snapshot"]["entries"]),
            "initial original source published")
        terminal = self.attach_daemon(fixture,session,"live-prefix")
        terminal.contains("accepted prefix 🌍")
        watches = len(terminal.requests("entry/source/subscribe"))
        terminal.daemon_recorder.pause_sources()
        delta = "\nnew tail 🌍"
        async def append():
            for link in list(fixture.native.connections):
                await fixture.native.notify(link,"item/agentMessage/delta",turnId="prefix-turn",
                                            itemId="prefix-source",delta=delta)
        asyncio.run_coroutine_threadsafe(append(),fixture.native.loop).result(timeout=3)
        terminal.contains("Loading visible source")
        self.assertIn("accepted prefix 🌍",terminal.text)
        self.assertNotIn("new tail 🌍",terminal.text)
        terminal.snapshot("prefix-during-delayed-source")
        terminal.paste("draft during source read")
        terminal.wait(lambda:terminal.input_is("draft during source read"),"composer stays live during read")
        terminal.daemon_recorder.resume_sources()
        terminal.contains("new tail 🌍")
        self.assertIn("accepted prefix 🌍",terminal.text)
        self.assertEqual(len(terminal.requests("entry/source/subscribe")),watches)
        self.assertEqual(terminal.requests("session/send"),[])
        terminal.snapshot("prefix-after-source-read")
