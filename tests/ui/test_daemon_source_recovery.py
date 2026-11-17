"""Exercise cache replacement and live-growth recovery through the real daemon and terminal."""

import asyncio

from test_daemon_client import daemon_support


class DaemonSourceRecovery:
    def test_daemon_terminal_keeps_indented_ascii_output_rows_exact_through_backbuffer_scroll(self):
        call = "    (root/'summary.json').write_text(json.dumps({'errors':errors,'turnStarts':len(terminal.requests('turn/start'))},indent=2))"
        output = "WRAP FIRST ROW\n" + (call + "\n") * 60 + "WRAP LAST ROW\n"
        item = {"type":"commandExecution", "id":"wrapped-code", "command":"cat recorded-code.py",
                "status":"completed", "exitCode":0, "aggregatedOutput":output}
        fixture, client, session = self.daemon_session("indented-ascii-wrap", lambda:self.source_history([item]))
        terminal = self.attach_daemon(fixture, session, "wrapped", width=100)
        terminal.contains("WRAP LAST ROW")
        terminal.paste("private draft while reading wrapped output")
        for _ in range(3):
            terminal.send(b"\x1b[5~" * 30)
            terminal.contains("WRAP FIRST ROW")
            terminal.send(b"\x1b[6~" * 30)
            terminal.contains("WRAP LAST ROW")
        self.assertTrue(terminal.input_is("private draft while reading wrapped output"))
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertTrue(terminal.requests("entry/source"))
        terminal.snapshot("indented-ascii-output-keeps-its-exact-rows")

    def test_daemon_terminal_drains_live_backlog_without_accepting_a_cancelled_watch_cursor(self):
        text = "original output\n" * 2000 + "ORIGINAL TAIL\n"
        item = {"type":"agentMessage", "id":"backlog-answer", "phase":"final_answer", "text":text}
        fixture, client, session = self.running_sources("watch-backlog", [item])
        terminal = self.attach_daemon(fixture, session, "backlog", defer_source_watch_once=True)
        recorder = terminal.daemon_recorder
        terminal.wait(recorder.source_watch_held.is_set, "first source watch pending")
        terminal.contains("ORIGINAL TAIL")
        terminal.paste("private backlog draft")
        terminal.wait(lambda:terminal.input_is("private backlog draft"), "draft ready")
        watch = terminal.requests("entry/source/subscribe")[0]
        self.source_append(fixture, "backlog-answer", "live appended output\n" * 10000 + "LIVE BACKLOG TAIL\n")
        terminal.contains("LIVE BACKLOG TAIL")
        reads = {request["id"] for request in terminal.requests("entry/source")
                 if request["params"]["cursor"] == watch["params"]["cursor"]}
        self.assertTrue(reads, "the paused watch gives way to a direct read at its accepted cursor")
        delayed = next(message["result"] for message in terminal.messages("server")
                       if message.get("id") in reads)
        self.assertTrue(delayed["has_more"], "the old reply ends before the accepted tail")
        recorder.finish_deferred_source_watch({"result":delayed})
        terminal.paste(" survives")
        terminal.wait(lambda:terminal.input_is("private backlog draft survives"), "typing after stale cursor")
        self.assertIn("LIVE BACKLOG TAIL", terminal.text)
        self.assertNotIn("F5 reconnects", terminal.text)
        self.assertNotIn("Source failed", terminal.text)
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertEqual(len(terminal.requests("hello")), 1)
        terminal.snapshot("paused-watch-late-cursor-keeps-live-backlog")

    def test_daemon_terminal_ignores_a_superseded_watch_error_after_source_replacement(self):
        def item(text):
            return {"type":"commandExecution", "id":"replaced-tool", "command":"printf output",
                    "status":"completed", "exitCode":0, "aggregatedOutput":text}
        original = "old output\n" * 5000 + "OLD OUTPUT TAIL\n"
        fixture, client, session = self.running_sources("superseded-watch", [item(original)])
        terminal = self.attach_daemon(fixture, session, "replacement", defer_source_watch_once=True)
        recorder = terminal.daemon_recorder
        terminal.wait(recorder.source_watch_held.is_set, "first source watch is still pending")
        terminal.contains("OLD OUTPUT TAIL")
        terminal.paste("private draft survives a stale watch error")
        terminal.wait(lambda:terminal.input_is("private draft survives a stale watch error"), "private draft ready")

        async def replace():
            for link in list(fixture.native.connections):
                await fixture.native.notify(link,"item/completed",turnId="history-turn",
                                            item=item("replacement output\n" * 4000 + "NEW OUTPUT TAIL\n"))
        asyncio.run_coroutine_threadsafe(replace(),fixture.native.loop).result(timeout=5)
        terminal.contains("NEW OUTPUT TAIL")
        terminal.wait(lambda:len(terminal.requests("entry/source/subscribe")) >= 2, "replacement watch issued")
        recorder.reject_deferred_source_watch()
        terminal.paste(" and still editable")
        terminal.wait(lambda:terminal.input_is("private draft survives a stale watch error and still editable"),
                      "terminal keeps handling input after the late error")
        self.assertNotIn("output watch was superseded", terminal.text)
        self.assertNotIn("F5 reconnects", terminal.text)
        self.assertNotIn("Source failed", terminal.text)
        self.assertIn("NEW OUTPUT TAIL", terminal.text)
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertEqual(len(terminal.requests("hello")), 1, "no reconnect")
        terminal.snapshot("superseded-watch-keeps-replacement-and-draft")

    @staticmethod
    def source_history(items):
        return {"id":"native-root", "status":{"type":"idle"}, "model":"fixture-model",
                "reasoningEffort":"low", "turns":[{"id":"history-turn", "status":"completed", "items":items}]}

    @staticmethod
    def source_append(fixture, identity, text, turn="history-turn"):
        async def append():
            for link in list(fixture.native.connections):
                await fixture.native.notify(link,"item/agentMessage/delta",turnId=turn,itemId=identity,delta=text)
        asyncio.run_coroutine_threadsafe(append(),fixture.native.loop).result(timeout=5)

    def running_sources(self, name, items):
        fixture, client, session = self.daemon_session(name,lambda:self.source_history([]))
        async def running(link, request):
            await fixture.native.notify(link,"turn/started",turn={"id":"history-turn"})
            for item in items:
                if item["type"] == "agentMessage":
                    for offset in range(0,len(item["text"]),65536):
                        await fixture.native.notify(link,"item/agentMessage/delta",turnId="history-turn",
                            itemId=item["id"],delta=item["text"][offset:offset+65536])
                else:
                    await fixture.native.notify(link,"item/completed",turnId="history-turn",item=item)
        fixture.native.prompt = running
        operation = client.operation(2)
        client.call("session/send",session=session,operation=operation,text="source recovery setup")
        client.outcome(operation)
        daemon_support.wait_for(lambda:len(client.call("session/subscribe",session=session,since=None)
            ["snapshot"]["entries"]) == len(items),"running source entries published")
        return fixture, client, session

    @staticmethod
    def source_chunks(terminal):
        for message in terminal.messages("server"):
            result = message.get("result")
            chunk = result if isinstance(result,dict) and "source" in result else message.get("params",{}).get("chunk",{})
            if "source" in chunk:
                yield chunk

    def test_daemon_terminal_refreshes_inline_text_after_a_gap_snapshot_with_the_same_entry_id(self):
        items = [{"type":"agentMessage", "id":"same-answer", "phase":"final_answer", "text":"before reset"}]
        fixture, client, session = self.running_sources("terminal-reset-cache",items)
        original = client.call("session/subscribe",session=session,since=None)["snapshot"]["entries"][-1]
        self.assertNotIn("source",original)
        terminal = self.attach_daemon(fixture,session,"reset-cache")
        terminal.contains("before reset")
        terminal.daemon_recorder.drop_text_once = True
        self.source_append(fixture,"same-answer"," after reset")
        terminal.wait(lambda:not terminal.daemon_recorder.drop_text_once,"one numbered append deliberately dropped")
        self.source_append(fixture,"same-answer"," after gap")
        terminal.contains("before reset after reset after gap")
        snapshots = [message["result"]["snapshot"] for message in terminal.messages("server")
                     if isinstance(message.get("result"),dict) and "snapshot" in message["result"]]
        self.assertGreaterEqual(len(snapshots),2)
        refreshed = next(entry for entry in snapshots[-1]["entries"] if entry["id"] == original["id"])
        self.assertEqual(refreshed["text"],"before reset after reset after gap")
        self.assertEqual(terminal.requests("session/send"),[])
        terminal.snapshot("same-id-snapshot-refresh")

    def test_daemon_terminal_keeps_sources_paused_after_a_rejected_session_restore(self):
        import socket
        text = "```text\n" + "source before reconnect\n"*4000 + "accepted source tail 🌍\n```\n"
        items = [{"type":"agentMessage", "id":"restore-answer", "phase":"final_answer", "text":text}]
        fixture, client, session = self.running_sources("terminal-source-restore", items)
        terminal = self.attach_daemon(fixture, session, "source-restore")
        terminal.contains("accepted source tail 🌍")
        terminal.paste("draft survives source restore failure")
        terminal.wait(lambda:terminal.input_is("draft survives source restore failure"), "private draft before reconnect")
        recorder = terminal.daemon_recorder
        recorder.pause_subscriptions(fail=True)
        reads = len(terminal.requests("entry/source/subscribe"))
        for peer in list(recorder.peers):
            try:
                peer.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
        terminal.contains("F5 reconnects")
        terminal.send(b"\x1b[15~")
        terminal.wait(recorder.subscription_held.is_set, "resumed session reply held")
        terminal.send(b"\x1b[H")
        terminal.pump(.1)
        self.assertEqual(len(terminal.requests("entry/source/subscribe")), reads)
        recorder.resume_subscriptions()
        terminal.contains("Injected resumed subscription failure")
        terminal.send(b"\x1b[F")
        terminal.contains("accepted source tail 🌍")
        terminal.pump(.1)
        self.assertEqual(len(terminal.requests("entry/source/subscribe")), reads)
        self.assertTrue(terminal.input_is("draft survives source restore failure"))
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.send(b"\x1b[15~")
        terminal.wait(lambda:len(terminal.requests("entry/source/subscribe")) == reads + 1,
                      "one source watch resumes after accepted explicit retry")
        terminal.contains("accepted source tail 🌍")
        self.assertTrue(terminal.input_is("draft survives source restore failure"))
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("source-rejected-restore-private-draft")

    def test_daemon_terminal_reloads_an_evicted_empty_source_when_it_gets_visible_text(self):
        from test_daemon_history import DaemonHistory
        blank = " \n"*32768
        items = [{"type":"agentMessage","id":"blank-answer","phase":"final_answer","text":blank}]
        items += DaemonHistory.many_tool_history()["turns"][0]["items"][:81]
        fixture, client, session = self.running_sources("terminal-empty-cache",items)
        original = client.call("session/subscribe",session=session,since=None)["snapshot"]["entries"][0]
        terminal = self.attach_daemon(fixture,session,"empty-cache")
        terminal.contains("source row-080 unique")
        terminal.send(b"\x1b[H")
        terminal.wait(lambda:any(chunk["entry"] == original["id"] and not chunk["has_more"]
                                 for chunk in self.source_chunks(terminal)),
                      "empty source fully read")
        terminal.wait(lambda:"loading" not in terminal.text.lower(),"empty source counted at zero rows")
        for step in range(200):
            terminal.send(b"\x1b[6~")
            terminal.pump(.02)
            if "loading" in terminal.text.lower():
                terminal.wait(lambda:"loading" not in terminal.text.lower(),"visible tools loaded")
            if any(request["params"]["entry"] == original["id"]
                   for request in terminal.requests("entry/source/unsubscribe")):
                break
        self.assertTrue(any(request["params"]["entry"] == original["id"]
                            for request in terminal.requests("entry/source/unsubscribe")),"empty source evicted")
        self.source_append(fixture,"blank-answer","blank source is now visible 🌍")
        daemon_support.wait_for(lambda:any(entry["id"] == original["id"] and
            int(entry.get("source",{}).get("bytes",0)) > len(blank.encode())
            for entry in client.call("session/subscribe",session=session,since=None)["snapshot"]["entries"]),
            "same blank source appended")
        terminal.wait(lambda:any(message.get("params",{}).get("event",{}).get("entry") == original["id"]
                      and message["params"]["event"].get("kind") == "source_changed"
                      for message in terminal.messages("server")), "terminal receives the changed empty source")
        # Scroll to the prefix again if the append arrived during the first Home key.
        for attempt in range(3):
            terminal.send(b"\x1b[H")
            terminal.pump(.05)
            if "blank source is now visible 🌍" in terminal.text:
                break
        terminal.contains("blank source is now visible 🌍")
        self.assertGreaterEqual(len([request for request in terminal.requests("entry/source/subscribe")
                                   if request["params"]["entry"] == original["id"]]),2)
        self.assertEqual(terminal.requests("session/send"),[])
        terminal.snapshot("empty-source-becomes-visible")

    def test_daemon_terminal_keeps_bounded_suffixes_when_logical_source_sizes_exceed_the_cache(self):
        self.check_source_growth(False)

    def test_daemon_terminal_keeps_an_unused_tool_suffix_while_a_visible_answer_grows(self):
        self.check_source_growth(True)

    def check_source_growth(self, unused_tool):
        old = "```text\n" + ("a"*78+"\n")*79639 + "\n```\n"
        live = "```text\n" + ("b"*78+"\n")*13273 + "visible-old-tail\n"
        items = [{"type":"agentMessage","id":"unused-answer","phase":"final_answer","text":old},
                 {"type":"agentMessage","id":"growing-answer","phase":"final_answer","text":live}]
        if unused_tool:
            items[0] = {"type":"commandExecution", "id":"unused-tool", "command":"cat unused.log",
                        "aggregatedOutput":old, "status":"completed", "exitCode":0}
        suffix = "tool" if unused_tool else "answer"
        fixture, client, session = self.running_sources("terminal-growing-cache-"+suffix,items)
        entries = client.call("session/subscribe",session=session,since=None)["snapshot"]["entries"]
        unused, growing = entries[-2:]
        self.assertEqual(unused.get("tool") is not None,unused_tool)
        terminal = self.attach_daemon(fixture,session,"growing-cache-"+suffix)
        terminal.contains("visible-old-tail")
        terminal.wait(lambda:any(chunk["entry"] == unused["id"] and not chunk["has_more"]
                                 for chunk in self.source_chunks(terminal)),
                      "unused six-MiB source loaded")
        terminal.paste("draft during growth")
        terminal.wait(lambda:terminal.input_is("draft during growth"),"private draft before live growth")
        for index in range(33):
            self.source_append(fixture,"growing-answer",("g"*78+"\n")*829)
        self.source_append(fixture,"growing-answer","\nvisible-grown-tail 🌍")
        terminal.contains("visible-grown-tail 🌍")
        self.assertTrue(terminal.input_is("draft during growth"))
        # Both pure-cache suffixes remain bounded even though their original bodies total >8MiB.
        self.assertFalse(any(request["params"]["entry"] == unused["id"]
                             for request in terminal.requests("entry/source/unsubscribe")))
        subscriptions = [request for request in terminal.requests("entry/source/subscribe")
                         if request["params"]["entry"] == growing["id"]]
        pauses = [request for request in terminal.requests("entry/source/unsubscribe")
                  if request["params"]["entry"] == growing["id"]]
        self.assertEqual(len(subscriptions), len(pauses)+1, "each paused watch resumes once")
        cursors = [request["params"]["cursor"] for request in subscriptions]
        self.assertEqual(len(cursors), len(set(cursors)), "backlog recovery advances the accepted cursor")
        self.assertFalse(any("error" in message for message in terminal.messages("server")))
        self.assertEqual(terminal.requests("session/send"),[])
        terminal.snapshot("visible-source-survives-growth")
