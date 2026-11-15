"""Exercise native goal controls through actual terminals, daemon, and native peer."""

import json


class DaemonGoals:
    def daemon_goals(self, name, enabled=True, width=120, goal_error_once=False):
        fixture, client, session = self.daemon_session(name)
        fixture.native.goals_enabled = enabled
        terminal = self.attach_daemon(fixture, session, name, width=width, goal_error_once=goal_error_once)
        terminal.contains("history-output 9999")
        return fixture, client, session, terminal

    def open_daemon_goal(self, terminal):
        terminal.send(b"\x1bg")
        terminal.contains("Codex goal")
        terminal.wait(lambda: "Goal state: Known" in terminal.text,
                      "current native goal through daemon")

    def daemon_goal_saved(self, terminal, count):
        terminal.wait(lambda: len(terminal.requests("goal/set")) == count,
                      "explicit typed goal change")
        terminal.wait(lambda: "Goal change pending" not in terminal.text
                      and "Field 1/3" not in terminal.text, "private save settled")

    def test_daemon_goal_create_preserves_composer_and_never_sends_goal_as_chat(self):
        fixture, client, session, terminal = self.daemon_goals("goal-create")
        terminal.paste("keep main draft 日本語")
        self.open_daemon_goal(terminal)
        terminal.send(b"e")
        terminal.contains("Field 1/3: Objective")
        terminal.paste("Build an explicit feature 日本語")
        terminal.send(b"\x13")
        self.daemon_goal_saved(terminal, 1)
        terminal.contains("Status: paused")
        request = terminal.requests("goal/set")[0]["params"]
        self.assertEqual(request["change"], {
            "objective": "Build an explicit feature 日本語", "token_budget": None, "status": "paused"})
        self.assertEqual(len(request["expected"]), 64)
        self.assertNotIn("threadId", json.dumps(request))
        native = [r for r in fixture.native.requests if r.get("method") == "thread/goal/set"]
        self.assertEqual(native[0]["params"], {"threadId": "native-root", "origin": "user",
                         "objective": "Build an explicit feature 日本語", "status": "paused"})
        terminal.send(b"\x1b")
        terminal.wait(lambda: terminal.input_is("keep main draft 日本語"), "main composer survives goal save")
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.replace_input("/goal")
        terminal.send(b"\r")
        terminal.contains("Codex goal")
        terminal.contains("Build an explicit feature 日本語")
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertEqual(terminal.requests("thread/goal/set"), [])
        terminal.snapshot("daemon-explicit-paused-goal")

    def test_daemon_goal_budget_pause_resume_and_clear_keep_counters_and_exact_consent(self):
        fixture, client, session, terminal = self.daemon_goals("goal-actions")
        self.open_daemon_goal(terminal)
        terminal.send(b"e")
        terminal.paste("Saved objective")
        terminal.send(b"\t")
        terminal.paste("9007199254740995")
        terminal.send(b"\x13")
        self.daemon_goal_saved(terminal, 1)
        terminal.contains("Token budget: 9007199254740995")
        terminal.contains("Tokens used: 9007199254740993")
        terminal.send(b"e")
        terminal.paste(" revised")
        terminal.send(b"\x13")
        self.daemon_goal_saved(terminal, 2)
        self.assertIsNone(terminal.requests("goal/set")[1]["params"]["change"]["token_budget"])
        terminal.contains("Token budget: 9007199254740995")
        terminal.send(b"a")
        terminal.contains("may start model work immediately")
        self.assertEqual(len(terminal.requests("goal/set")), 2)
        terminal.send(b"n")
        terminal.send(b"ay")
        self.daemon_goal_saved(terminal, 3)
        terminal.contains("Status: active")
        terminal.send(b"\x12")
        terminal.contains("Pause or clear the Codex goal")
        terminal.send(b"p")
        self.daemon_goal_saved(terminal, 4)
        terminal.contains("Status: paused")
        terminal.contains("Time used (seconds): 7")
        terminal.send(b"d")
        terminal.contains("Clear the saved goal")
        terminal.send(b"n")
        self.assertEqual(terminal.requests("goal/clear"), [])
        terminal.send(b"dy")
        terminal.wait(lambda: bool(terminal.requests("goal/clear")), "explicit clear")
        terminal.contains("No saved goal reported")
        for request in fixture.native.requests:
            if request.get("method") in ("thread/goal/set", "thread/goal/clear"):
                self.assertEqual(request["params"]["origin"], "user")
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("daemon-goal-budget-and-consent")

    def test_daemon_goal_two_terminals_keep_goal_drafts_and_private_rejections_separate(self):
        fixture, client, session, first = self.daemon_goals("goal-private")
        second = self.attach_daemon(fixture, session, "goal-private-second")
        second.contains("history-output 9999")
        first.paste("first composer")
        second.paste("second composer 🌍")
        self.open_daemon_goal(first)
        self.open_daemon_goal(second)
        first.send(b"e")
        first.paste("first unsent objective")
        first.contains("first unsent objective")
        second.send(b"e")
        second.paste("second unsent objective 🌍")
        second.contains("second unsent objective 🌍")
        first.resize(18, 10)
        first.contains("Resize to edit")
        first.paste("IGNORED_TINY_GOAL_PASTE")
        first.send(b"\x13\t")
        first.contains("Field 2/3")
        self.assertEqual(first.requests("goal/set"), [])
        first.resize(120, 32)
        first.contains("first unsent objective")
        first.send(b"\x13")
        self.daemon_goal_saved(first, 1)
        second.contains("second unsent objective 🌍")
        second.send(b"\x01\x0b")
        second.paste("reject objective")
        second.send(b"\x13")
        second.contains("PRIVATE_GOAL_REJECTION")
        second.contains("reject objective")
        self.assertNotIn("PRIVATE_GOAL_REJECTION", first.text)
        second.send(b"\x1b")
        second.contains("Status: paused")
        second.send(b"p")
        second.wait(lambda: len(second.requests("goal/set")) == 2, "status-only change")
        second.wait(lambda: "Goal change pending" not in second.text, "status reply")
        self.assertIsNone(second.requests("goal/set")[1]["params"]["change"]["objective"])
        second.send(b"e")
        second.contains("reject objective")
        second.send(b"\x1b")
        second.wait(lambda: "Field 1/3" not in second.text, "second editor returns to overview")
        second.send(b"\x1b")
        second.wait(lambda: second.input_is("second composer 🌍"), "second main draft preserved")
        first.send(b"\x1b")
        first.wait(lambda: first.input_is("first composer"), "first main draft preserved")
        self.assertEqual(first.requests("session/send"), [])
        self.assertEqual(second.requests("session/send"), [])
        first.snapshot("daemon-first-private-goal")
        second.snapshot("daemon-second-private-goal")

    def test_daemon_goal_pending_write_blocks_ralph_without_blocking_human_chat(self):
        fixture, client, session, terminal = self.daemon_goals("goal-pending")
        (fixture.root / ".agents").mkdir()
        (fixture.root / ".agents/ralph-prompt.md").write_text("RALPH_MUST_NOT_RUN")
        self.open_daemon_goal(terminal)
        terminal.send(b"e")
        terminal.paste("Explicit active goal")
        terminal.send(b"\t\t\x1b[C\x13")
        terminal.contains("may start model work immediately")
        fixture.native.goal_hold = True
        terminal.send(b"y")
        terminal.contains("Goal change pending")
        terminal.send(b"\x12")
        terminal.contains("pending Codex goal change")
        terminal.send(b"\x1b")
        terminal.wait(lambda: "Field 3/3" not in terminal.text, "pending editor returns to overview")
        terminal.send(b"\x1b")
        terminal.wait(lambda: "Codex goal · Esc back" not in terminal.text, "pending editor closes")
        terminal.paste("human while activation pending")
        terminal.send(b"\r")
        terminal.contains("Answer: human while activation pending")
        fixture.native.release_goal()
        terminal.wait(lambda: fixture.native.goal_pending is None, "held native acknowledgement released")
        self.assertEqual(len(terminal.requests("goal/set")), 1)
        self.assertEqual(len(terminal.requests("session/toggleRalph")), 1)
        self.assertEqual([r["params"]["text"] for r in terminal.requests("session/send")],
                         ["human while activation pending"])
        self.assertNotIn("RALPH_MUST_NOT_RUN", str(fixture.native.requests))
        terminal.snapshot("daemon-pending-goal-human-chat")

    def test_daemon_goal_old_confirmation_cannot_activate_a_newer_goal(self):
        fixture, client, session, terminal = self.daemon_goals("goal-stale-consent")
        self.open_daemon_goal(terminal)
        terminal.send(b"e")
        terminal.paste("First objective")
        terminal.send(b"\x13")
        self.daemon_goal_saved(terminal, 1)
        terminal.send(b"a")
        terminal.contains("Start/resume this Codex goal?")
        current = client.call("goal/read", session=session)["goal"]
        operation = client.operation(2)
        client.call("goal/set", session=session, operation=operation, expected=current["revision"],
                    change={"objective": "Newer objective", "token_budget": None, "status": "paused"})
        self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
        terminal.send(b"y")
        terminal.contains("goal changed")
        native = [r for r in fixture.native.requests if r.get("method") == "thread/goal/set"]
        self.assertEqual(len(native), 2)
        self.assertTrue(all(r["params"]["status"] == "paused" for r in native))
        terminal.contains("Newer objective")
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("daemon-stale-goal-consent")

    def test_daemon_goal_disabled_state_stays_visible_without_hidden_chat(self):
        fixture, client, session, terminal = self.daemon_goals("goal-disabled", enabled=False)
        terminal.paste("main draft")
        terminal.send(b"\x1bg")
        terminal.contains("Unavailable/disabled")
        terminal.contains("goals feature is disabled")
        terminal.send(b"e")
        terminal.contains("Read the current Codex goal before editing")
        terminal.send(b"\x1b")
        terminal.wait(lambda: terminal.input_is("main draft"), "unsupported goal preserves composer")
        self.assertEqual(terminal.requests("goal/set"), [])
        self.assertEqual(terminal.requests("goal/clear"), [])
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("daemon-disabled-goal")

    def test_daemon_goal_refresh_lives_until_the_last_terminal_releases_its_panel(self):
        import time
        fixture, client, session, first = self.daemon_goals("goal-interest")
        second = self.attach_daemon(fixture, session, "goal-interest-second")
        second.contains("history-output 9999")
        self.open_daemon_goal(first)
        self.open_daemon_goal(second)
        first.contains("No saved goal reported")
        second.contains("No saved goal reported")
        first.send(b"\x1b")
        first.wait(lambda: bool(first.requests("goal/unsubscribe")), "first panel released")
        fixture.native.goal = {"threadId": "native-root", "objective": "Refresh after first panel closes",
            "status": "paused", "tokenBudget": None, "tokensUsed": 17, "timeUsedSeconds": 3,
            "createdAt": 1700000000, "updatedAt": 1700000001}
        second.wait(lambda: "Refresh after first panel closes" in second.text,
                    "remaining terminal keeps native thirty-second refresh", timeout=35)
        self.open_daemon_goal(first)
        first.contains("Refresh after first panel closes")
        first.close()
        fixture.native.goal["objective"] = "Refresh after first terminal disconnects"
        fixture.native.goal["updatedAt"] += 1
        second.wait(lambda: "Refresh after first terminal disconnects" in second.text,
                    "remaining terminal keeps refresh after disconnect", timeout=35)
        second.send(b"\x1b")
        second.wait(lambda: bool(second.requests("goal/unsubscribe")), "last panel released")
        # The request reply proves the manager applied the close before we count later native reads.
        request = second.requests("goal/unsubscribe")[-1]["id"]
        second.wait(lambda: any(m.get("id") == request for m in second.messages("server")),
                    "last goal close acknowledged")
        previous = len([r for r in fixture.native.requests if r.get("method") == "thread/goal/get"])
        client.call("goal/subscribe", session=session)
        second.wait(lambda: len([r for r in fixture.native.requests if r.get("method") == "thread/goal/get"]) > previous,
                    "headless reader started one native refresh")
        client.call("session/unsubscribe", session=session)
        reads = len([r for r in fixture.native.requests if r.get("method") == "thread/goal/get"])
        deadline = time.monotonic() + 31
        while time.monotonic() < deadline:
            second.pump(.05)
        self.assertEqual(len([r for r in fixture.native.requests if r.get("method") == "thread/goal/get"]), reads)
        self.assertFalse(any(r.get("method") in ("thread/goal/set", "thread/goal/clear", "turn/start")
                             for r in fixture.native.requests))
        second.snapshot("daemon-goal-last-reader-released")

    def test_daemon_goal_restart_preserves_unconfirmed_goal_and_composer_without_resending(self):
        fixture, client, session, terminal = self.daemon_goals("goal-reconnect")
        terminal.paste("main draft survives reconnect")
        self.open_daemon_goal(terminal)
        terminal.send(b"e")
        terminal.paste("Unconfirmed goal draft 🌍")
        fixture.native.goal_hold = True
        terminal.send(b"\x13")
        terminal.contains("Goal change pending")
        terminal.wait(lambda: fixture.native.goal_pending is not None, "native goal write reached held peer")
        fixture.stop()
        terminal.wait(lambda: "F5 reconnects" in terminal.text, "goal panel reports lost daemon")
        fixture.start()
        terminal.send(b"\x1b[15~")
        terminal.wait(lambda: len(terminal.requests("goal/subscribe")) == 2, "goal read restored after reconnect")
        terminal.contains("Unconfirmed goal draft 🌍")
        terminal.wait(lambda: "Goal change pending" not in terminal.text, "lost private command becomes uncertain")
        terminal.paste(" revised locally")
        terminal.contains("revised locally")
        terminal.send(b"\x1b")
        terminal.wait(lambda: "Field 1/3" not in terminal.text, "uncertain editor returns to overview")
        terminal.send(b"\x1b")
        terminal.wait(lambda: terminal.input_is("main draft survives reconnect"), "ordinary draft survives goal reconnect")
        self.assertEqual(len(terminal.requests("goal/set")), 1)
        self.assertEqual(len(terminal.requests("session/subscribe")), 2)
        self.assertEqual(len([r for r in fixture.native.requests if r.get("method") == "thread/goal/set"]), 1)
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("daemon-goal-uncertain-reconnect")

    def test_daemon_goal_opened_offline_starts_periodic_reads_after_reconnect(self):
        fixture, client, session, terminal = self.daemon_goals("goal-offline-open")
        terminal.paste("offline composer draft")
        terminal.wait(lambda: terminal.input_is("offline composer draft"), "draft before daemon stops")
        fixture.stop()
        terminal.contains("F5 reconnects")
        terminal.send(b"\x1bg")
        terminal.contains("Codex goal")
        fixture.start()
        terminal.send(b"\x1b[15~")
        terminal.wait(lambda: len(terminal.requests("goal/subscribe")) == 1, "offline panel restores a new read interest")
        terminal.wait(lambda: "Goal state: Known" in terminal.text and "(reading)" not in terminal.text,
                      "fresh goal facts after offline open")
        fixture.native.goal = {"threadId":"native-root", "objective":"Offline-open timer refreshed this goal",
            "status":"paused", "tokenBudget":None, "tokensUsed":17, "timeUsedSeconds":3,
            "createdAt":1700000000, "updatedAt":1700000001}
        terminal.wait(lambda: "Offline-open timer refreshed this goal" in terminal.text,
                      "offline-open panel keeps native periodic refresh", timeout=35)
        terminal.send(b"\x1b")
        terminal.wait(lambda: terminal.input_is("offline composer draft"), "offline main draft survives")
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertFalse(any(r.get("method") in ("thread/goal/set", "thread/goal/clear", "turn/start")
                             for r in fixture.native.requests))
        terminal.snapshot("daemon-offline-open-read-interest")

    def test_daemon_goal_failed_admitted_read_cancels_the_native_reader(self):
        import time
        fixture, client, session, terminal = self.daemon_goals("goal-failed-read", goal_error_once=True)
        terminal.send(b"\x1bg")
        terminal.contains("Injected admitted goal read failure")
        terminal.wait(lambda: len(terminal.requests("goal/unsubscribe")) == 1,
                      "failed admitted reader explicitly cancelled")
        identity = terminal.requests("goal/unsubscribe")[0]["id"]
        terminal.wait(lambda: any(m.get("id") == identity for m in terminal.messages("server")),
                      "daemon acknowledged failed reader cancellation")
        terminal.wait(lambda: client.call("goal/read", session=session)["goal"]["knowledge"] == "known"
                      and not client.call("goal/read", session=session)["goal"]["reading"],
                      "already dispatched native read settled")
        terminal.send(b"r")
        terminal.wait(lambda: len(terminal.requests("goal/subscribe")) == 2, "explicit refresh reopens a failed read lifetime")
        terminal.wait(lambda: "Injected admitted goal read failure" not in terminal.text and "(reading)" not in terminal.text,
                      "successful retry clears only the read error")
        terminal.send(b"\x1b")
        terminal.wait(lambda: "Codex goal · Esc back" not in terminal.text, "failed panel closes")
        terminal.wait(lambda: len(terminal.requests("goal/unsubscribe")) == 2, "retried reader closed independently")
        reads = len([r for r in fixture.native.requests if r.get("method") == "thread/goal/get"])
        deadline = time.monotonic() + 31
        while time.monotonic() < deadline:
            terminal.pump(.05)
        self.assertEqual(len([r for r in fixture.native.requests if r.get("method") == "thread/goal/get"]), reads)
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("daemon-failed-admitted-read-released")

    def test_daemon_goal_objective_edits_keep_all_five_observed_nonactive_statuses(self):
        fixture, client, session, terminal = self.daemon_goals("goal-observed-statuses")
        terminal.paste("independent five-status composer")
        terminal.wait(lambda:terminal.input_is("independent five-status composer"), "main draft before goals")
        self.open_daemon_goal(terminal)
        for index, status in enumerate(["paused", "blocked", "usageLimited", "budgetLimited", "complete"], 1):
            fixture.native.goal = {"threadId":"native-root", "objective":f"observed {status}", "status":status,
                "tokensUsed":0, "timeUsedSeconds":0, "createdAt":1, "updatedAt":index}
            terminal.send(b"r")
            terminal.contains(f"Status: {status}")
            terminal.contains(f"observed {status}")
            terminal.send(b"e\x01\x0b")
            terminal.paste(f"edited {status}")
            terminal.contains(f"edited {status}")
            # Native clear is deliberately unseen: the client must still send its observed status.
            fixture.native.goal = None
            terminal.send(b"\x13")
            self.daemon_goal_saved(terminal, index)
            typed = terminal.requests("goal/set")[-1]["params"]["change"]
            self.assertEqual(typed, {"objective":f"edited {status}", "token_budget":None, "status":status})
            native = [row for row in fixture.native.requests if row.get("method") == "thread/goal/set"]
            self.assertEqual(native[-1]["params"], {"threadId":"native-root", "origin":"user",
                "objective":f"edited {status}", "status":status})
        self.assertEqual(len(native), 5)
        terminal.send(b"\x1b")
        terminal.wait(lambda:terminal.input_is("independent five-status composer"), "five edits retain main composer")
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("goal-five-observed-statuses-remain-explicit")

    def test_daemon_goal_pending_save_ignores_keys_and_paste_until_its_original_reply(self):
        fixture, client, session, terminal = self.daemon_goals("goal-held-editor")
        terminal.paste("independent held-goal composer")
        terminal.wait(lambda:terminal.input_is("independent held-goal composer"), "main draft before held goal")
        self.open_daemon_goal(terminal)
        terminal.send(b"e")
        terminal.paste("held original objective")
        terminal.contains("held original objective")
        fixture.native.goal_hold = True
        terminal.send(b"\x13")
        terminal.wait(lambda:fixture.native.goal_pending is not None and "Goal change pending" in terminal.text,
                      "original native write held while editor is frozen")
        terminal.paste("DO_NOT_EDIT_PENDING")
        terminal.send(b"X\t")
        terminal.wait(lambda:"key code=Tab" in (terminal.root/"keys.log").read_text(), "pending keys consumed")
        self.assertNotIn("DO_NOT_EDIT_PENDING", terminal.text)
        self.assertEqual(len(terminal.requests("goal/set")), 1)
        self.assertEqual(terminal.requests("goal/set")[0]["params"]["change"]["objective"], "held original objective")
        fixture.native.release_goal()
        self.daemon_goal_saved(terminal, 1)
        terminal.send(b"e")
        terminal.contains("Field 1/3: Objective")
        terminal.contains("held original objective")
        self.assertNotIn("DO_NOT_EDIT_PENDING", terminal.text)
        terminal.send(b"\x1b")
        terminal.wait(lambda:"Field 1/3" not in terminal.text, "editor returns to goal overview")
        terminal.send(b"\x1b")
        terminal.wait(lambda:terminal.input_is("independent held-goal composer"), "held save retains main composer")
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("goal-original-pending-write-keeps-editor-frozen")
