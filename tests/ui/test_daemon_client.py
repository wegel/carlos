"""Drive the actual terminal through the actual daemon and deterministic native adapter."""

from pathlib import Path
import sys

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tests/daemon"))
import support as daemon_support
from daemon_recorder import DaemonRecorder


class DaemonClient:
    def daemon_session(self, name, history=None, configure=None):
        runner = sys.modules[type(self).__module__]
        daemon_support.BINARY = runner.BINARY.parent / "carlosd"
        daemon_support.ARTIFACTS = runner.ARTIFACTS / "daemon"
        fixture = daemon_support.Fixture(name, codex=True)
        self.addCleanup(fixture.close)
        if history is not None:
            fixture.native.history = history
        if configure is not None:
            configure(fixture)
        client = daemon_support.Client(fixture, "terminal-test-owner")
        self.addCleanup(client.close)
        create = client.operation(1)
        session = client.call("session/create", operation=create, backend="codex",
                              cwd=str(fixture.root), name="Shared terminal")['session']
        client.outcome(create)
        daemon_support.wait_for(lambda: client.call("session/subscribe", session=session, since=None)
                                ['snapshot']['info']['availability'] == "ready", "native session ready")
        return fixture, client, session

    def attach_daemon(self, fixture, session, name, width=100, source_error_once=False, extra_env=None, goal_error_once=False, source_read_error_after=0, extra_args=None, defer_source_watch_once=False, defer_subscription_once=False, defer_source_read_once=False):
        directory = self.summary_root() / name
        directory.mkdir(parents=True, exist_ok=True)
        recorder = DaemonRecorder(fixture.root / (name+".sock"), fixture.socket, directory, source_error_once=source_error_once, goal_error_once=goal_error_once, source_read_error_after=source_read_error_after, defer_source_watch_once=defer_source_watch_once, defer_subscription_once=defer_subscription_once, defer_source_read_once=defer_source_read_once)
        self.addCleanup(recorder.close)
        terminal = self.launch(directory=directory, width=width, resume=session,
                               extra_args=["--daemon-socket", str(recorder.path), *(extra_args or [])], extra_env=extra_env)
        terminal.daemon_recorder = recorder
        return terminal

    def test_daemon_replaced_subscription_error_keeps_current_facts_and_private_draft(self):
        fixture, client, session = self.daemon_session("superseded-session")
        terminal = self.attach_daemon(fixture, session, "newest", defer_subscription_once=True)
        recorder = terminal.daemon_recorder
        terminal.wait(recorder.subscription_held.is_set, "original subscription deliberately pending")
        terminal.paste("private draft while subscription changes")
        terminal.wait(lambda: terminal.input_is("private draft while subscription changes"), "private draft ready")
        terminal.send(b"\x1b[15~")
        terminal.contains("history-output 9999")
        terminal.wait(lambda: len(terminal.requests("session/subscribe")) == 2, "explicit replacement subscription")
        recorder.reject_deferred_subscription()
        terminal.paste(" survives")
        terminal.wait(lambda: terminal.input_is("private draft while subscription changes survives"), "typing after retired failure")
        self.assertNotIn("subscription was superseded", terminal.text)
        self.assertNotIn("F5 reconnects", terminal.text)
        self.assertIn("history-output 9999", terminal.text)
        self.assertEqual(len(terminal.requests("hello")), 1)
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertEqual(fixture.native.prompts, 0)
        terminal.snapshot("newest-subscription-keeps-facts-and-draft")

    def test_daemon_terminals_keep_drafts_private_while_native_control_releases_a_held_send(self):
        fixture, client, session = self.daemon_session("terminal-control")
        first = self.attach_daemon(fixture, session, "first")
        second = self.attach_daemon(fixture, session, "second")
        first.contains("history-output 9999")
        second.contains("history-output 9999")
        second.paste("second window draft 🌍")
        second.wait(lambda: second.input_is("second window draft 🌍"), "private second draft")
        first.paste("held")
        first.send(b"\r")
        first.contains("Approve")
        second.contains("Approve")
        first.send(b"y")
        first.wait(lambda: fixture.native.answers == 1, "native approval through daemon")
        first.send(b"\x1b")
        first.wait(lambda: fixture.native.interrupts == 1, "native interrupt through daemon")
        second.wait(lambda: second.input_is("second window draft 🌍"), "other composer survives shared controls")
        starts = [request for request in fixture.native.requests if request.get("method") == "turn/start"]
        self.assertEqual([request["params"]["input"][0]["text"] for request in starts], ["held"])
        self.assertEqual(fixture.native.answers, 1)
        self.assertEqual(fixture.native.interrupts, 1)
        self.assertEqual(first.requests("turn/start"), [], "terminal cannot contact a native adapter directly")
        self.assertEqual(second.requests("turn/start"), [])
        self.assertEqual(len(first.requests("session/send")), 1)
        self.assertEqual(first.requests("session/send")[0]["params"]["text"], "held")
        self.assertEqual(second.requests("session/send"), [])
        self.assertEqual(len(first.requests("session/approve")), 1)
        self.assertEqual(len(first.requests("session/interrupt")), 1)
        self.assertTrue(first.requests("entry/source/subscribe"))
        self.assertTrue(second.requests("entry/source/subscribe"))
        first.snapshot("socket-first")
        second.snapshot("socket-second")

    def test_daemon_restart_keeps_an_unsent_terminal_draft_and_never_replays_a_send(self):
        fixture, client, session = self.daemon_session("terminal-restart")
        terminal = self.attach_daemon(fixture, session, "reconnect")
        terminal.contains("history-output 9999")
        terminal.paste("keep this draft 🌍")
        terminal.wait(lambda: terminal.input_is("keep this draft 🌍"), "draft before disconnect")
        fixture.stop()
        terminal.contains("F5 reconnects")
        terminal.wait(lambda: terminal.input_is("keep this draft 🌍"), "offline draft retained")
        import asyncio
        import threading
        released = threading.Event()
        resume_seen = threading.Event()
        self.addCleanup(released.set)
        handle = fixture.native.handle
        async def held_resume(link, request):
            if request.get("method") == "thread/resume":
                resume_seen.set()
                while not released.is_set():
                    await asyncio.sleep(.005)
            await handle(link, request)
        fixture.native.handle = held_resume
        fixture.start()
        terminal.send(b"\x1b[15~")
        terminal.wait(lambda: len(terminal.requests("hello")) == 2, "second socket greeting")
        terminal.wait(resume_seen.is_set, "native resume deliberately waits")
        terminal.send(b"\r")
        terminal.contains("Waiting for the session to become ready")
        self.assertTrue(terminal.input_is("keep this draft 🌍"))
        self.assertEqual(terminal.requests("session/send"), [], "early Enter allocates no send")
        released.set()
        terminal.wait(lambda: len(terminal.requests("session/subscribe")) >= 2, "same logical session resubscribed")
        def replacement_ready():
            messages = terminal.messages("server")
            hello = terminal.requests("hello")[-1]["id"]
            start = next((index + 1 for index, message in enumerate(messages)
                          if message.get("id") == hello), len(messages))
            for message in messages[start:]:
                params = message.get("params", {})
                result = message.get("result")
                info = result.get("snapshot", {}).get("info") if isinstance(result, dict) else None
                info = info or params.get("snapshot", {}).get("info") or params.get("event", {}).get("info")
                info = info or params.get("event", {}).get("snapshot", {}).get("info")
                if info and info["id"] == session and info["availability"] == "ready":
                    return True
            return False
        terminal.wait(replacement_ready, "fresh ready facts from the replacement daemon")
        terminal.contains("Idle")
        self.assertTrue(terminal.input_is("keep this draft 🌍"))
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.send(b"\r")
        terminal.contains("Answer: keep this draft 🌍")
        self.assertEqual(len(terminal.requests("session/send")), 1)
        self.assertEqual(terminal.requests("session/send")[0]["params"]["operation"]["id"], "1")
        self.assertEqual(terminal.requests("session/send")[0]["params"]["text"], "keep this draft 🌍")
        terminal.snapshot("socket-reconnected")

    def test_daemon_terminal_releases_handled_private_operations_after_their_shared_echo(self):
        fixture, client, session = self.daemon_session("terminal-operations")
        async def echoed_prompt(link, request):
            text = request["params"]["input"][0]["text"]
            token = request["params"]["clientUserMessageId"]
            turn = "native-" + token
            await fixture.native.notify(link, "turn/started", turn={"id":turn})
            await fixture.native.notify(link, "item/completed", turnId=turn, item={
                "type":"userMessage","id":token,"clientId":token,
                "content":[{"type":"text","text":text}]})
            await fixture.native.notify(link, "item/agentMessage/delta", turnId=turn,
                                        itemId="answer-"+token, delta="Answer: "+text)
            await fixture.native.notify(link, "turn/completed", turn={"id":turn,"status":"completed"})
        fixture.native.prompt = echoed_prompt
        terminal = self.attach_daemon(fixture, session, "operations")
        terminal.contains("history-output 9999")
        for index in range(130):
            text = f"prompt-{index}-end"
            terminal.paste(text)
            terminal.send(b"\r")
            terminal.contains("Answer: " + text)
        requests = terminal.requests("session/send")
        self.assertEqual(len(requests), 130)
        identities = [request["params"]["operation"]["id"] for request in requests]
        self.assertEqual(len(set(identities)), 130)
        terminal.wait(lambda: terminal.input_is(""), "composer after more than 128 handled operations")
        terminal.snapshot("handled-private-results")

    def test_daemon_terminal_reads_beyond_the_cache_and_keeps_live_appends(self):
        import asyncio
        fixture, client, session = self.daemon_session("terminal-source-pressure")
        tail = "\n\npressure tail"

        async def large_source(link, request):
            await fixture.native.notify(link, "turn/started", turn={"id":"pressure-turn"})
            for _ in range(145):
                await fixture.native.notify(link, "item/agentMessage/delta", turnId="pressure-turn",
                                            itemId="pressure-source", delta="x"*65536)
            await fixture.native.notify(link, "item/agentMessage/delta", turnId="pressure-turn",
                                        itemId="pressure-source", delta=tail)
        fixture.native.prompt = large_source
        send = client.operation(2)
        client.call("session/send", session=session, operation=send, text="source pressure")
        client.outcome(send)
        expected = 145*65536+len(tail)
        daemon_support.wait_for(lambda: any(int(entry.get("source",{}).get("bytes",0)) == expected
                                for entry in client.call("session/subscribe", session=session, since=None)
                                ["snapshot"]["entries"]), "large source published before attaching")
        terminal = self.attach_daemon(fixture, session, "pressure")
        terminal.wait(lambda: "pressure tail" in terminal.text, "body above the pure cache limit", timeout=30)
        terminal.wait(lambda: bool(terminal.requests("entry/source/subscribe")), "live watch starts at the accepted tail")
        count = len(terminal.requests("entry/source/subscribe"))
        for index in range(3):
            addition = f"\n\nappended-{index}"
            async def append():
                for link in list(fixture.native.connections):
                    await fixture.native.notify(link, "item/agentMessage/delta", turnId="pressure-turn",
                                                itemId="pressure-source", delta=addition)
            asyncio.run_coroutine_threadsafe(append(), fixture.native.loop).result(timeout=3)
            terminal.contains(f"appended-{index}")
        self.assertEqual(len(terminal.requests("entry/source/subscribe")), count)
        self.assertGreater(len(terminal.requests("entry/source")), 128)
        self.assertNotIn("exceeds the body cache", terminal.text)
        terminal.paste("private draft after large source")
        terminal.wait(lambda: terminal.input_is("private draft after large source"), "large source leaves composer usable")
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertEqual(terminal.requests("session/interrupt"), [])
        terminal.snapshot("above-cache-source-and-appends")

    def ansi_session(self, name):
        fixture, client, session = self.daemon_session(name)
        body = "\x1b[31m" + "red 🌍 still red\n"*24000 + "tail stays red"
        async def ansi_source(link, request):
            await fixture.native.notify(link, "turn/started", turn={"id":"ansi-turn"})
            await fixture.native.notify(link, "item/completed", turnId="ansi-turn", item={
                "type":"commandExecution","id":"ansi-source","command":"printf probe",
                "status":"completed","exitCode":0,"aggregatedOutput":body})
            await fixture.native.notify(link, "turn/completed", turn={"id":"ansi-turn","status":"completed"})
        fixture.native.prompt = ansi_source
        send = client.operation(2)
        client.call("session/send", session=session, operation=send, text="ansi source")
        client.outcome(send)
        daemon_support.wait_for(lambda: any("printf probe" in (entry.get("tool") or {}).get("label", "")
                                for entry in client.call("session/subscribe", session=session, since=None)
                                ["snapshot"]["entries"]), "large native ANSI source published")
        return fixture, client, session, body

    def clipboard_root(self, name):
        root = self.summary_root()/name
        (root/"bin").mkdir(parents=True, exist_ok=True)
        program = root/"bin/wl-copy"
        program.write_text("#!/usr/bin/env python3\nimport os,sys\nfrom pathlib import Path\nPath(os.environ['CARLOS_TEST_ROOT']).joinpath('copied.txt').write_bytes(sys.stdin.buffer.read())\n")
        program.chmod(0o755)
        return root

    def test_daemon_terminal_borrows_a_large_ansi_body_without_losing_its_color_prefix(self):
        import json
        import time
        fixture, client, session, body = self.ansi_session("terminal-ansi-source")
        started = time.monotonic()
        terminal = self.attach_daemon(fixture, session, "ansi")
        terminal.contains("tail stays red")
        elapsed = (time.monotonic()-started)*1000
        cell = self.source_cell(terminal, "tail stays red", "tail")
        self.assertEqual(cell.fg, "cd0000")
        chunks = []
        for message in terminal.messages("server"):
            chunk = message.get("result",{})
            chunk = chunk if isinstance(chunk, dict) else {}
            if "source" not in chunk:
                chunk = message.get("params",{}).get("chunk",{})
            if chunk.get("source",{}).get("bytes") == str(len(("$ printf probe\n"+body+"\nexit code: 0").encode())):
                chunks.append(chunk)
        text = "".join({int(chunk["offset"]):chunk["text"] for chunk in chunks}[offset]
                       for offset in sorted({int(chunk["offset"]) for chunk in chunks}))
        self.assertEqual(text, "$ printf probe\n"+body+"\nexit code: 0")
        self.assertGreater(len(text.encode()), 256*1024)
        self.assertTrue(all(len(chunk["text"].encode()) <= 65536 for chunk in chunks))
        metrics = {"warm_attach_ms":elapsed,"source_bytes":len(text.encode()),"source_chunks":len(chunks),
                   "tail_color":cell.fg}
        for name, pid in (("terminal",terminal.pid),("daemon",fixture.process.pid)):
            status = Path(f"/proc/{pid}/status").read_text().splitlines()
            metrics[name+"_rss_kib"] = int(next(line for line in status if line.startswith("VmRSS:")).split()[1])
        (terminal.root / "metrics.json").write_text(json.dumps(metrics,indent=2)+"\n")
        terminal.snapshot("large-source-color")

    def test_daemon_terminal_does_not_reopen_a_source_after_a_failed_rpc_reply(self):
        import asyncio
        fixture, client, session = self.daemon_session("terminal-read-error", history=lambda: {
            "id":"native-root", "status":{"type":"idle"}, "model":"fixture-model",
            "reasoningEffort":"low", "turns":[]})
        async def live_source(link, request):
            await fixture.native.notify(link, "turn/started", turn={"id":"read-error-turn"})
            await fixture.native.notify(link, "item/agentMessage/delta", turnId="read-error-turn",
                                        itemId="read-error-source", delta="x"*65536)
        fixture.native.prompt = live_source
        send = client.operation(2)
        client.call("session/send", session=session, operation=send, text="read error source")
        client.outcome(send)
        daemon_support.wait_for(lambda: any(int(entry.get("source",{}).get("bytes",0)) == 65536
                                for entry in client.call("session/subscribe", session=session, since=None)
                                ["snapshot"]["entries"]), "running source published")
        terminal = self.attach_daemon(fixture, session, "read-error", source_error_once=True)
        terminal.contains("Injected daemon source read failure")
        terminal.wait(lambda: bool(terminal.requests("entry/source/unsubscribe")), "failed RPC cancels source watch")
        count = len(terminal.requests("entry/source/subscribe"))
        for index in range(3):
            async def append():
                for link in list(fixture.native.connections):
                    await fixture.native.notify(link, "item/agentMessage/delta", turnId="read-error-turn",
                                                itemId="read-error-source", delta=f"appended-{index}")
            asyncio.run_coroutine_threadsafe(append(), fixture.native.loop).result(timeout=3)
            terminal.wait(lambda: any(message.get("params",{}).get("event",{}).get("source",{}).get("bytes") ==
                          str(65536+len("appended-0")*(index+1)) for message in terminal.messages("server")),
                          "append to the same failed source revision")
        self.assertEqual(len(terminal.requests("entry/source/subscribe")), count)
        terminal.paste("draft after failed read")
        terminal.wait(lambda: terminal.input_is("draft after failed read"), "failed RPC leaves composer usable")
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("failed-rpc-stays-closed")
        failed_entry = terminal.requests("entry/source/subscribe")[0]["params"]["entry"]
        before = len([request for request in terminal.requests("entry/source/subscribe")
                      if request["params"]["entry"] == failed_entry])
        terminal.send(b"\x1b[17~")
        terminal.wait(lambda: len([request for request in terminal.requests("entry/source/subscribe")
                      if request["params"]["entry"] == failed_entry]) == before+1, "F6 explicitly retries failed read")
        terminal.wait(lambda: "Injected daemon source read failure" not in terminal.text, "retry clears failed-read hint")
        self.assertTrue(terminal.input_is("draft after failed read"))
        self.assertEqual(terminal.requests("session/send"), [])

    def test_daemon_terminal_folds_resizes_scrolls_and_copies_unicode_without_a_send(self):
        import json
        from wcwidth import wcswidth
        fixture, client, session, body = self.ansi_session("terminal-local-display")
        root = self.clipboard_root("display")
        terminal = self.attach_daemon(fixture, session, "display", extra_env={
            "SSH_TTY":"","SSH_CONNECTION":"","SSH_CLIENT":""})
        terminal.contains("tail stays red")
        terminal.paste("/tools compact")
        terminal.send(b"\r")
        terminal.wait(lambda: "tail stays red" not in terminal.text and terminal.input_is(""), "local fold")
        self.assertEqual(json.loads((root/"config/carlos/ui-settings.json").read_text())["tool_output"], "compact")
        terminal.paste("/tools full")
        terminal.send(b"\r")
        terminal.contains("tail stays red")
        terminal.resize(28,32)
        terminal.contains("tail stays red")
        self.assertEqual(self.source_cell(terminal,"tail stays red","tail").fg,"cd0000")
        selected = "red 🌍 still red"
        row = next(index for index,line in enumerate(terminal.screen.display) if selected in line)
        column = terminal.screen.display[row].index(selected)
        terminal.send(f"\x1b[<0;{column+1};{row+1}M".encode())
        terminal.send(f"\x1b[<32;{column+wcswidth(selected)};{row+1}M".encode())
        terminal.send(f"\x1b[<0;{column+wcswidth(selected)};{row+1}m".encode())
        terminal.wait(lambda: (root/"copied.txt").exists(), "selection copied from borrowed source")
        self.assertEqual((root/"copied.txt").read_text(),selected)
        terminal.send(b"\x0c")
        terminal.send(b"\x1b[5~")
        terminal.wait(lambda: "tail stays red" not in terminal.text,"PageUp scrolls transcript")
        terminal.send(b"\x1b[6~")
        terminal.contains("tail stays red")
        terminal.send(b"\x1b[<64;2;2M")
        terminal.wait(lambda: "tail stays red" not in terminal.text,"wheel scrolls transcript")
        terminal.send(b"\x1b[<65;2;2M")
        terminal.contains("tail stays red")
        fixture.stop()
        terminal.contains("F5 reconnects")
        terminal.paste("/tools compact")
        terminal.send(b"\r")
        terminal.wait(lambda: terminal.input_is("") and "tail stays red" not in terminal.text,"offline fold")
        self.assertEqual(terminal.requests("session/send"),[])
        self.assertEqual(len([request for request in fixture.native.requests if request.get("method")=="turn/start"]),1)
        terminal.snapshot("local-phone-controls")

    def test_daemon_terminal_copies_complete_markdown_source_after_phone_resize(self):
        fixture, client, session = self.daemon_session("terminal-markdown-copy")
        body = "[ref]: https://example.test\n\n```text\n" + "**literal 🌍** [ref]\n"*24000 + "```\n\nTail [link][ref]\n"
        async def markdown_source(link, request):
            await fixture.native.notify(link,"turn/started",turn={"id":"markdown-copy"})
            await fixture.native.notify(link,"item/completed",turnId="markdown-copy",item={
                "type":"agentMessage","id":"markdown-source","phase":"final_answer","text":body})
            await fixture.native.notify(link,"turn/completed",turn={"id":"markdown-copy","status":"completed"})
        fixture.native.prompt=markdown_source
        send=client.operation(2)
        client.call("session/send",session=session,operation=send,text="copy original Markdown")
        client.outcome(send)
        daemon_support.wait_for(lambda:any(int(entry.get("source",{}).get("bytes",0))==len(body.encode())
            for entry in client.call("session/subscribe",session=session,since=None)["snapshot"]["entries"]),"Markdown published")
        root=self.clipboard_root("markdown")
        terminal=self.attach_daemon(fixture,session,"markdown",extra_env={"SSH_TTY":"","SSH_CONNECTION":"","SSH_CLIENT":""})
        terminal.contains("Tail link")
        terminal.resize(28,32)
        terminal.contains("Tail link")
        terminal.send(b"\x19")
        terminal.wait(lambda:(root/"copied.txt").exists(),"complete original Markdown copied")
        self.assertEqual((root/"copied.txt").read_bytes(),body.encode())
        self.assertGreater(len(body.encode()),256*1024)
        self.assertEqual(terminal.requests("session/send"),[])
        terminal.snapshot("markdown-phone-copy")
