"""Exercise local prompt recall, explicit branches, rename and help through the actual daemon."""

import asyncio
import json
import threading



class DaemonEdits:
    def test_daemon_empty_prompt_history_preserves_the_private_composer(self):
        fixture, client, session = self.daemon_session("empty-prompt-history")
        terminal = self.attach_daemon(fixture, session, "empty-prompts")
        terminal.contains("history-output 9999")
        terminal.paste("private draft without any earlier prompt 🌍")
        terminal.send(b"\x1b[A")
        terminal.wait(lambda: bool(terminal.requests("prompt/select")), "explicit history read")
        request = terminal.requests("prompt/select")[-1]
        terminal.wait(lambda: any(message.get("id") == request["id"] and
                      isinstance(message.get("result"), dict) and
                      message["result"].get("record", "missing") is None
                      for message in terminal.messages("server")), "empty history reply")
        terminal.send(b"\x1b[B")
        terminal.wait(lambda: terminal.input_is("private draft without any earlier prompt 🌍"),
                      "empty history leaves the original composer intact")
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertEqual(terminal.requests("session/rewind"), [])

    def test_daemon_fork_and_history_reset_keep_the_local_theme(self):
        fixture, client, session = self.daemon_session("flow-rewind", configure=lambda f: setattr(f.native, "history_user", True))
        directory = self.summary_root() / "flow-editor"
        settings = directory / "config/carlos/ui-settings.json"
        settings.parent.mkdir(parents=True)
        original = json.dumps({"themes":{"desktop":"flow", "phone":"flow"}})
        settings.write_text(original)
        terminal = self.attach_daemon(fixture, session, "flow-editor")
        terminal.contains("history-output 9999")
        terminal.rewind("original prompt")
        terminal.replace_input("edited flow branch")
        terminal.send(b"\r")
        terminal.contains("Answer: edited flow branch")
        terminal.contains("─ ✓ answer")
        row = next(y for y, line in enumerate(terminal.screen.display) if "Answer: edited flow branch" in line)
        self.assertEqual(terminal.screen.buffer[row][0].data, " ")
        self.assertEqual(settings.read_text(), original)
        self.assertEqual(len(terminal.requests("session/rewind")), 1)
        self.assertEqual(sum(r.get("method") == "thread/fork" for r in fixture.native.requests), 1)

    def test_daemon_mobile_coordinate_pastes_scroll_without_entering_the_composer(self):
        fixture, client, session = self.daemon_session("mobile-coordinate-paste")
        terminal = self.attach_daemon(fixture, session, "mobile-coordinates")
        terminal.contains("history-output 9999")
        terminal.paste("66;52")
        terminal.wait(lambda: any('66;52' in line for line in (terminal.root / "keys.log").read_text().splitlines()),
                      "terminal received the first mobile coordinate paste")
        terminal.paste("64;47")
        terminal.wait(lambda: "history-output 9994" in terminal.text and "history-output 9999" not in terminal.text,
                      "mobile coordinate paste moves the actual visible transcript upward")
        terminal.wait(lambda: terminal.input_is(""), "coordinate pastes never enter the private composer")
        self.assertNotIn("history-output 9999", terminal.text)
        terminal.paste("private draft")
        terminal.paste("ordinary 66;52 text")
        terminal.wait(lambda: terminal.input_is("private draftordinary 66;52 text"),
                      "a nonempty composer retains ordinary pasted coordinate-like text")
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertEqual(terminal.requests("session/rewind"), [])

    def test_daemon_mobile_fallback_keeps_ordinary_empty_composer_pastes_literal(self):
        fixture, client, session = self.daemon_session("ordinary-coordinate-like-paste")
        terminal = self.attach_daemon(fixture, session, "ordinary-paste")
        terminal.contains("history-output 9999")
        terminal.paste("let x = 1; let y = 2;")
        terminal.wait(lambda: terminal.input_is("let x = 1; let y = 2;"),
                      "ordinary code in an empty composer remains literal text")
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertEqual(terminal.requests("session/rewind"), [])

    def editable_terminal(self, name, configure=None):
        def setup(fixture):
            fixture.native.history_user = True
            if configure:
                configure(fixture)
        fixture, client, session = self.daemon_session(name, configure=setup)
        terminal = self.attach_daemon(fixture, session, name)
        terminal.contains("history-output 9999")
        return fixture, client, session, terminal

    def test_daemon_rewind_keeps_slash_commands_literal_and_cancels_without_a_native_fork(self):
        fixture, client, session, terminal = self.editable_terminal("rewind-literal")
        terminal.rewind("original prompt")
        terminal.wait(lambda:terminal.text.count("original prompt") == 2,
                      "rewind loads the selected original row outside the newest snapshot")
        terminal.replace_input("discarded private edit 🌍")
        terminal.send(b"\x1b")
        terminal.wait(lambda:terminal.input_is(""), "cancel restores the original empty composer")
        self.assertEqual(terminal.requests("session/rewind"), [])
        terminal.rewind("original prompt")
        terminal.replace_input("/rename this is literal edited chat")
        terminal.send(b"\r")
        terminal.contains("Answer: /rename this is literal edited chat")
        self.assertEqual(len(terminal.requests("session/rewind")), 1)
        self.assertEqual(terminal.requests("session/rename"), [])
        self.assertEqual(sum(request.get("method") == "thread/fork" for request in fixture.native.requests), 1)

    def test_daemon_rewind_failure_retries_only_the_part_that_did_not_finish(self):
        for failing, expected_forks in (("thread/fork", 2), ("turn/start", 1)):
            with self.subTest(failing=failing):
                def configure(fixture, failing=failing):
                    handle = fixture.native.handle
                    state = {"fail":True}
                    async def reject_once(link, request):
                        if request.get("method") == failing and state["fail"]:
                            state["fail"] = False
                            await fixture.native.send(link, {"id":request["id"],"error":{"code":-32602,"message":"explicit edit fixture refusal"}})
                        else:
                            await handle(link, request)
                    fixture.native.handle = reject_once
                fixture, client, session, terminal = self.editable_terminal("rewind-failed-" + failing.replace("/", "-"), configure)
                terminal.rewind("original prompt")
                terminal.replace_input("explicit edited retry 日本語")
                terminal.send(b"\r")
                terminal.contains("Edit failed")
                terminal.wait(lambda:terminal.input_is("explicit edited retry 日本語"), "original operation returns only its own edit")
                terminal.send(b"\r")
                terminal.contains("Answer: explicit edited retry 日本語")
                self.assertEqual(sum(request.get("method") == "thread/fork" for request in fixture.native.requests), expected_forks)
                self.assertEqual(len(terminal.requests("session/rewind")), expected_forks)

    def test_daemon_prompt_recall_keeps_rapid_arrow_keys_and_restores_the_private_draft(self):
        fixture, client, session, terminal = self.editable_terminal("recall-arrows")
        terminal.paste("second current human")
        terminal.send(b"\r")
        terminal.contains("Answer: second current human")
        terminal.paste("private unsent composer 🌍")
        terminal.send(b"\x1b[A\x1b[A")
        terminal.wait(lambda:terminal.input_is("original prompt"), "both arrow keys select their exact records")
        terminal.send(b"\x1b[B\x1b[B")
        terminal.wait(lambda:terminal.input_is("private unsent composer 🌍"), "both down keys return the saved composer")
        self.assertEqual(terminal.requests("session/rewind"), [])
        self.assertEqual(len(terminal.requests("session/send")), 1)

    def test_daemon_rename_is_private_and_help_keys_never_submit_hidden_text(self):
        fixture, client, session, terminal = self.editable_terminal("rename-help")
        terminal.paste("/rename")
        terminal.send(b"\r")
        terminal.wait(lambda:terminal.input_is("/rename Shared terminal"), "bare rename reads accepted shared name")
        terminal.replace_input("/rename Renommé 日本語 🌍")
        terminal.send(b"\r")
        terminal.wait(lambda:terminal.placeholder_is("Renommé 日本語 🌍"), "accepted name becomes a dim placeholder")
        self.assertEqual(len(terminal.requests("session/rename")), 1)
        terminal.send(b"?")
        terminal.contains("Help")
        terminal.resize(36,12)
        terminal.wait(lambda: terminal.screen.display[2].strip() == "and stops (Claude/Codex)",
                      "help repaints its wrapped rows at the resized width")
        terminal.send(b"\x1b[F")
        terminal.contains("/tools")
        terminal.paste("must not enter the hidden composer")
        terminal.send(b"\r\x1b")
        terminal.wait(lambda:terminal.input_is(""), "help consumes paste and Enter")
        self.assertEqual(terminal.requests("session/send"), [])

    def test_daemon_early_native_prompt_echo_stays_editable_after_held_send_reply(self):
        echoed, release = threading.Event(), threading.Event()

        def configure(fixture):
            original = fixture.native.handle
            native_history = fixture.native.history
            live_turns = []

            def history():
                thread = native_history()
                thread["turns"].extend(live_turns)
                return thread
            fixture.native.history = history

            async def handle(link, request):
                method, params = request.get("method"), request.get("params", {})
                if method == "thread/fork" and params["beforeTurnId"] == "turn-1":
                    fixture.native.thread_id = "native-second-fork"
                    live_turns.clear()
                    await fixture.native.send(link, {"id":request["id"], "result":{"thread":fixture.native.history()}})
                    return
                if method != "turn/start":
                    await original(link, request)
                    return
                fixture.native.prompts += 1
                turn = f"turn-{fixture.native.prompts}"
                text = params["input"][0]["text"]
                user = {"type":"userMessage", "id":turn + "-user", "clientId":params["clientUserMessageId"],
                        "content":[{"type":"text", "text":text}]}
                answer = {"type":"agentMessage", "id":turn + "-answer", "text":"Answer: " + text}
                live_turns.append({"id":turn, "status":"completed", "items":[user, answer]})
                await fixture.native.notify(link, "turn/started", turn={"id":turn})
                await fixture.native.notify(link, "item/completed", turnId=turn, item=user)
                await fixture.native.notify(link, "item/completed", turnId=turn, item=answer)
                await fixture.native.notify(link, "turn/completed", turn={"id":turn, "status":"completed"})
                if text == "edited second":
                    echoed.set()
                    while not release.is_set():
                        await asyncio.sleep(.005)
                await fixture.native.send(link, {"id":request["id"], "result":{"turn":{"id":turn}}})
            fixture.native.handle = handle

        fixture, client, session, terminal = self.editable_terminal("early-native-edit-echo", configure)
        self.addCleanup(release.set)
        terminal.rewind("original prompt")
        terminal.replace_input("edited second")
        terminal.send(b"\r")
        terminal.contains("Answer: edited second")
        self.assertTrue(echoed.is_set())
        admitted = terminal.requests("session/rewind")[0]
        first = admitted["params"]["operation"]
        result = next(row["result"] for row in terminal.messages("server") if row.get("id") == admitted["id"])
        self.assertIn(result["state"]["kind"], ("pending", "queued"))
        release.set()
        terminal.wait(lambda: any(row.get("params", {}).get("operation") == first
                      and row["params"]["state"]["kind"] == "accepted"
                      for row in terminal.messages("server") if row.get("method") == "operation/updated"),
                      "original edit settles only after its native echo is visible")
        terminal.rewind("edited second")
        selected = terminal.requests("prompt/select")[-1]
        result = next(row["result"] for row in terminal.messages("server") if row.get("id") == selected["id"])
        self.assertIsNotNone(result["record"]["entry"], "latest prompt keeps its native transcript link")
        terminal.replace_input("edited again")
        terminal.send(b"\r")
        terminal.contains("Answer: edited again")
        forks = [request["params"]["beforeTurnId"] for request in fixture.native.requests if request.get("method") == "thread/fork"]
        self.assertEqual(forks, ["history-turn", "turn-1"])
        self.assertEqual([request["params"]["input"][0]["text"] for request in fixture.native.requests if request.get("method") == "turn/start"],
                         ["edited second", "edited again"])
        self.assertEqual(terminal.requests("session/send"), [])
