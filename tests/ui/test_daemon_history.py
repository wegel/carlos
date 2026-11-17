"""Keep hundreds of socket-backed source rows reachable across source eviction and older pages."""


class DaemonHistory:
    def test_daemon_terminal_opens_supported_rows_before_an_unrendered_native_tail(self):
        import sys
        from test_daemon_client import daemon_support
        from test_native_history import NativeHistoryTests
        runner = sys.modules[type(self).__module__]
        daemon_support.BINARY = runner.BINARY.parent / "carlosd"
        daemon_support.ARTIFACTS = runner.ARTIFACTS / "daemon"
        fixture, peer, client, session, opened = NativeHistoryTests.native_history_fixture(
            self, "terminal-unrendered-tail", unrendered=range(14,24), turns=24)
        self.assertEqual(client.outcome(opened)["state"]["kind"], "accepted")
        terminal = self.attach_daemon(fixture, session, "unrendered-tail")
        terminal.contains("native answer 13 🌍")
        self.assertEqual(len(peer.pages), 11, "startup crosses every unrendered turn before Ready")
        def oldest_visible():
            if "native user 0 🌍" in terminal.text:
                return True
            terminal.send(b"\x1b[H")
            return False
        terminal.wait(oldest_visible, "every supported older native row remains reachable")
        self.assertEqual(len(peer.pages), 24)
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertEqual(fixture.native.prompts, 0)
        terminal.snapshot("unrendered-native-tail-keeps-oldest-supported-history")

    def test_daemon_terminal_shares_a_held_older_read_and_crosses_unrendered_native_items(self):
        import sys
        from test_daemon_client import daemon_support
        from native_history_windows import NativeHistoryWindows
        runner = sys.modules[type(self).__module__]
        daemon_support.BINARY = runner.BINARY.parent / "carlosd"
        daemon_support.ARTIFACTS = runner.ARTIFACTS / "daemon"
        fixture = daemon_support.Fixture("terminal-concurrent-native-history", codex=True)
        self.addCleanup(fixture.close)
        peer = NativeHistoryWindows(fixture.native, turns=24)
        for turn in peer.turns[4:14]:
            turn["items"] = [{"type":"futureOpaqueItem", "id":"opaque-" + turn["id"]}]
        client = daemon_support.Client(fixture, "other-history-reader")
        self.addCleanup(client.close)
        opened = client.operation(1)
        session = client.call("session/open", operation=opened, backend="codex", cwd=str(fixture.root),
                              target={"kind":"resume", "address":fixture.native.thread_id})["session"]
        self.assertEqual(client.outcome(opened)["state"]["kind"], "accepted")
        snapshot = client.call("session/subscribe", session=session, since=None)["snapshot"]
        peer.pause = True
        pending = client.send("session/history", session=session, before=snapshot["entries"][0]["id"], limit=100)
        daemon_support.wait_for(lambda: peer.held, "other device holds the native read")
        terminal = self.attach_daemon(fixture, session, "concurrent-native-history")
        terminal.contains("native answer 23 🌍")
        terminal.send(b"\x1b[H")
        terminal.wait(lambda: len(terminal.requests("session/history")) == 1, "terminal joins the older read")
        terminal.resize(90, 40)
        terminal.paste("private draft during shared read 🌍")
        terminal.wait(lambda: terminal.input_is("private draft during shared read 🌍"), "private draft while shared read waits")
        self.assertEqual(len(peer.pages), 2, "one shared native fetch remains held")
        peer.release()
        self.assertTrue(client.response(pending)["result"]["entries"])
        def oldest_visible():
            if "native user 0 🌍" in terminal.text:
                return True
            terminal.send(b"\x1b[5~")
            return False
        terminal.wait(oldest_visible, "terminal crosses opaque native items without a stale page")
        self.assertTrue(terminal.input_is("private draft during shared read 🌍"))
        self.assertNotIn("queue is full", terminal.text)
        self.assertEqual(len(peer.pages), 24)
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertEqual(fixture.native.prompts, 0)
        terminal.snapshot("shared-native-history-reaches-oldest-with-private-draft")

    def test_daemon_terminal_recalls_older_native_claude_prompts_without_scrolling_history(self):
        _, native, _, _, _, terminal = self.claude_history_terminal("terminal-claude-recall")
        terminal.paste("private draft before native recall 🌍")
        terminal.send(b"\x1b[A" * 34)
        terminal.wait(lambda: terminal.input_is("native Claude user 116 🌍"), "rapid Up keys cross the native window boundary")
        self.assertEqual(len(native.requests()), 2)
        terminal.send(b"\x1b[B" * 34)
        terminal.wait(lambda: terminal.input_is("private draft before native recall 🌍"), "Down restores the exact private draft")
        self.assertEqual(terminal.requests("session/history"), [])
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertEqual(terminal.requests("session/rewind"), [])
        self.assertEqual(native.path.read_text(), native.original)
        terminal.snapshot("native-prompt-recall-keeps-private-draft")

    def claude_history_terminal(self, name, hold=False):
        import sys
        from test_daemon_client import daemon_support
        from test_claude_history import ClaudeHistoryTests
        runner = sys.modules[type(self).__module__]
        daemon_support.BINARY = runner.BINARY.parent / "carlosd"
        daemon_support.ARTIFACTS = runner.ARTIFACTS / "daemon"
        fixture, native, client, session, snapshot = ClaudeHistoryTests.claude_history_fixture(self, name, hold=hold)
        terminal = self.attach_daemon(fixture, session, name, width=100)
        terminal.contains("native Claude assistant 149 🌍")
        return fixture, native, client, session, snapshot, terminal

    def test_daemon_terminal_pages_complete_native_claude_history_with_stable_anchors(self):
        _, native, _, _, _, terminal = self.claude_history_terminal("terminal-claude-history")
        self.assertEqual(len(native.requests()), 1)
        for count, anchor in enumerate((118, 86, 54, 22), 1):
            terminal.send(b"\x1b[H")
            terminal.wait(lambda: len(terminal.requests("session/history")) == count, "one explicit SDK prefix read")
            request_id = terminal.requests("session/history")[-1]["id"]
            terminal.wait(lambda: any(message.get("id") == request_id for message in terminal.messages("server")), "SDK prefix replied")
            terminal.resize(90 if count % 2 else 100, 40)
            terminal.wait(lambda: terminal.input_is(""), "terminal applies the resized viewport")
            terminal.contains(f"native Claude user {anchor} 🌍")
        terminal.send(b"\x1b[H")
        terminal.contains("native Claude user 0 🌍")
        terminal.snapshot("oldest-native-claude-window")
        terminal.send(b"\x1b[F")
        terminal.contains("native Claude assistant 149 🌍")
        self.assertEqual(len(native.requests()), 5)
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertEqual(native.path.read_text(), native.original)

    def test_daemon_terminal_keeps_a_private_draft_and_anchor_during_held_native_claude_history(self):
        from test_daemon_client import daemon_support
        fixture, native, _, session, _, terminal = self.claude_history_terminal("terminal-claude-held", hold=True)
        terminal.send(b"\x1b[H")
        daemon_support.wait_for(lambda: (fixture.root / "history-held").exists(), "SDK reply held")
        terminal.paste("private draft while native history waits 🌍")
        terminal.wait(lambda: terminal.input_is("private draft while native history waits 🌍"), "private draft stays editable")
        other = daemon_support.Client(fixture, "other-native-client")
        self.addCleanup(other.close)
        sent = other.operation(1)
        other.call("session/send", session=session, operation=sent, text="hold this turn")
        self.assertEqual(other.outcome(sent)["state"]["kind"], "accepted")
        live = daemon_support.wait_for(lambda: value if (value := other.call("session/subscribe", session=session, since=None)["snapshot"])
                                      ["info"]["current_turn"] is not None else None, "native streaming beside history")
        interrupted = other.operation(2)
        other.call("session/interrupt", session=session, operation=interrupted, turn=live["info"]["current_turn"])
        self.assertEqual(other.outcome(interrupted)["state"]["kind"], "accepted")
        (fixture.root / "release-history").write_text("release")
        request_id = terminal.requests("session/history")[0]["id"]
        terminal.wait(lambda: any(message.get("id") == request_id for message in terminal.messages("server")), "original older read completed")
        terminal.resize(90, 40)
        terminal.wait(lambda: terminal.input_is("private draft while native history waits 🌍"), "resized composer redraws the same draft")
        terminal.contains("native Claude user 118 🌍")
        self.assertTrue(terminal.input_is("private draft while native history waits 🌍"))
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertTrue(native.path.read_text().startswith(native.original))
        terminal.snapshot("private-draft-and-anchor-after-older-native-read")

    def test_daemon_terminal_expands_native_file_changes_and_complete_mcp_results(self):
        from native_items import PATCH, MCP
        history = {"id":"native-root", "status":{"type":"idle"}, "turns":[
            {"id":"native-items", "status":"completed", "items":[PATCH, MCP]},
        ]}
        fixture, client, session = self.daemon_session("terminal-native-items", lambda: history)
        terminal = self.attach_daemon(fixture, session, "native-items", width=100)
        terminal.contains("Lookup")
        self.tools_command(terminal, "/tools full", "full")
        terminal.contains('"answer": 42')
        terminal.contains("native MCP response 🌍")
        terminal.snapshot("native-mcp-result-expanded")
        terminal.send(b"\x1b[H")
        terminal.contains("+new 🌍")
        terminal.contains("+second 🌍")
        terminal.snapshot("native-file-changes-expanded")
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertEqual(fixture.native.prompts, 0)
        self.assertTrue(terminal.requests("entry/source/subscribe"))
        snapshot = client.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertEqual(len([row for row in snapshot["entries"] if row.get("tool")]), 3)

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
        peer = NativeHistoryWindows(fixture.native, turns=12)
        client = daemon_support.Client(fixture, "history-owner")
        self.addCleanup(client.close)
        operation = client.operation(1)
        session = client.call("session/open", operation=operation, backend="codex", cwd=str(fixture.root),
                              target={"kind":"resume", "address":fixture.native.thread_id})["session"]
        client.outcome(operation)
        terminal = self.attach_daemon(fixture, session, "native-history")
        terminal.contains("native answer 11 🌍")
        for count, anchor in enumerate(range(10, -1, -1), 1):
            terminal.send(b"\x1b[H")
            terminal.wait(lambda: len(terminal.requests("session/history")) == count, "one explicit older client page")
            terminal.wait(lambda: sum(isinstance(message.get("result"), dict) and "has_older" in message["result"]
                          for message in terminal.messages("server")) == count, "older page reached the client")
            terminal.resize(90 if count % 2 else 100, 40)
            if f"native user {anchor} 🌍" not in terminal.text:
                terminal.send(b"\x1b[H")
            terminal.contains(f"native user {anchor} 🌍")
        terminal.send(b"\x1b[H")
        terminal.contains("native user 0 🌍")
        self.assertEqual(len(peer.pages), 12)
        terminal.snapshot("oldest-native-window-with-preserved-anchor")
        terminal.send(b"\x1b[F")
        terminal.contains("native answer 11 🌍")
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
        footer = terminal.screen.display[-2]
        watches = len(terminal.requests("entry/source/subscribe"))
        terminal.daemon_recorder.pause_sources()
        delta = "\nnew tail 🌍"
        async def append():
            for link in list(fixture.native.connections):
                await fixture.native.notify(link,"item/agentMessage/delta",turnId="prefix-turn",
                                            itemId="prefix-source",delta=delta)
        asyncio.run_coroutine_threadsafe(append(),fixture.native.loop).result(timeout=3)
        terminal.wait(lambda: terminal.daemon_recorder.held_sources, "new tail source is held")
        terminal.wait(lambda: terminal.screen.display[-2] == footer, "source reads keep the normal footer")
        self.assertNotIn("Loading visible source", terminal.screen.display[-2])
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
