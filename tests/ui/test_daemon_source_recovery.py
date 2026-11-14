"""Exercise cache replacement and live-growth recovery through the real daemon and terminal."""

import asyncio

from test_daemon_client import daemon_support


class DaemonSourceRecovery:
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

    def test_daemon_terminal_evicts_unused_source_before_a_visible_watch_grows_over_the_shared_budget(self):
        self.check_source_growth(False)

    def test_daemon_terminal_counts_an_unused_tool_source_when_a_visible_answer_grows(self):
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
        self.assertTrue(any(request["params"]["entry"] == unused["id"]
                            for request in terminal.requests("entry/source/unsubscribe")))
        self.assertEqual(len([request for request in terminal.requests("entry/source/subscribe")
                             if request["params"]["entry"] == growing["id"]]),1)
        self.assertFalse(any("error" in message for message in terminal.messages("server")))
        self.assertEqual(terminal.requests("session/send"),[])
        terminal.snapshot("visible-source-survives-growth")
