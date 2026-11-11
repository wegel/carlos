"""Check visible native questions and the SDK's real callback answer wire."""

import json
import re


class ClaudeQuestions:
    def open_question(self, text, mode="modern", width=100):
        self.question_case = getattr(self, "question_case", 0) + 1
        terminal = self.launch(backend="claude", resume=None, mode=mode, width=width,
                               directory=self.summary_root() / f"case-{self.question_case}")
        self.claude_ready(terminal)
        terminal.paste(text)
        terminal.send(b"\r")
        terminal.contains("F10 interrupt")
        return terminal

    def native_answers(self, terminal):
        return {message["response"]["request_id"]: message["response"].get("response", {})
                for message in terminal.messages() if message.get("type") == "control_response"}

    def test_claude_ask_single_multi_and_unicode_free_text_use_native_answers(self):
        terminal = self.open_question("ask native")
        terminal.contains("Which output format?")
        terminal.send(b"\r")
        terminal.contains("Which sections?")
        terminal.send(b" \x1b[B \r")
        terminal.contains("Which custom greeting?")
        terminal.send(b"\x1b[B\x1b[B")
        terminal.paste("Salut 🦊 你好")
        terminal.contains("Salut 🦊 你好")
        self.assertLess(terminal.screen.cursor.y, terminal.screen.lines - 1)
        terminal.send(b"\r")
        terminal.contains("Review answers")
        terminal.snapshot("review-native-answers")
        terminal.send(b"\x13")
        terminal.contains("Native callbacks settled")
        answer = self.native_answers(terminal)["ask-question"]
        self.assertEqual(answer["behavior"], "allow")
        self.assertEqual(answer["updatedInput"]["answers"], {"Which output format?": "Summary",
                         "Which sections?": "Introduction, Conclusion", "Which custom greeting?": "Salut 🦊 你好"})
        self.assertEqual(answer["updatedInput"]["metadata"], {"source": "fixture"})
        self.assertEqual(len(self.helper_requests(terminal, "prompt")), 1)

    def test_claude_default_no_question_names_ctrl_s_and_ignores_y_on_review(self):
        terminal = self.open_question("ask default no")
        terminal.contains("Press Ctrl+S twice")
        self.assertNotIn("Press y twice", terminal.text)
        terminal.send(b"\r")
        terminal.contains('Format: "Summary"')
        terminal.send(b"\x13")
        terminal.contains("Press Ctrl+S again")
        terminal.contains("Alt+N declines")
        self.assertEqual(self.helper_requests(terminal, "callback.respond"), [])
        terminal.send(b"y")
        for _ in range(3):
            terminal.pump(.03)
        self.assertEqual(self.helper_requests(terminal, "callback.respond"), [])
        terminal.contains('Format: "Summary"')
        terminal.snapshot("question-confirmation-uses-submit-key")
        terminal.send(b"\x13")
        terminal.contains("Native callbacks settled")
        self.assertEqual(self.native_answers(terminal)["ask-question"]["updatedInput"]["answers"], {"Which output format?": "Summary"})
        self.assertEqual(len(self.helper_requests(terminal, "callback.respond")), 1)

    def test_claude_mcp_schema_error_keeps_editor_and_correction_returns_a_number(self):
        terminal = self.open_question("mcp form")
        terminal.contains("MCP server: counter-server")
        terminal.paste("17")
        terminal.send(b"\x13")
        terminal.contains("at least 18")
        terminal.contains("› 17")
        self.assertNotIn("form-question", self.native_answers(terminal))
        terminal.snapshot("invalid-number-retained")
        terminal.replace_input("23")
        terminal.send(b"\x13")
        terminal.contains("Native callbacks settled")
        self.assertEqual(self.native_answers(terminal)["form-question"], {"action": "accept", "content": {"age": 23}})
        self.assertEqual(len(self.helper_requests(terminal, "callback.respond")), 2)

    def test_claude_mcp_titled_enums_boolean_defaults_and_optional_omit_keep_types(self):
        terminal = self.open_question("typed form")
        terminal.contains("› 30")
        terminal.send(b"\t")
        terminal.contains("Email (required)")
        terminal.paste("a@b")
        terminal.send(b"\t\x1b[B\r")
        terminal.contains("Tags (required)")
        terminal.send(b" \x1b[B \t")
        terminal.contains("Opt in (required)")
        terminal.contains("[x] false")
        terminal.send(b"\t\x0f\t")
        terminal.contains("Review answers")
        terminal.send(b"\x13")
        terminal.contains("Native callbacks settled")
        self.assertEqual(self.native_answers(terminal)["typed-question"]["content"], {
            "a_age": 30, "b_email": "a@b", "c_color": "#0f0", "d_tags": ["r", "w"], "e_optin": False})
        terminal.snapshot("typed-native-form")

    def test_claude_concurrent_child_requests_keep_independent_drafts_and_lifetimes(self):
        terminal = self.open_question("concurrent questions")
        terminal.contains("4 waiting")
        terminal.wait(lambda: any(frame.get("type") == "result" for frame in terminal.messages("server")), "parent result while children wait")
        terminal.send(b"\x1b[B\x1b[B")
        terminal.paste("独立 draft")
        terminal.send(b"\x1bOS")
        terminal.contains("Child: child-two")
        terminal.send(b"n")
        terminal.contains("Child: child-three")
        terminal.send(b"\x1bOS")
        terminal.contains("MCP server: counter-server")
        terminal.paste("31")
        terminal.send(b"\x13")
        terminal.contains("独立 draft")
        terminal.send(b"\x13")
        terminal.contains("Child: child-three")
        terminal.send(b"c")
        terminal.contains("Native callbacks settled")
        answers = self.native_answers(terminal)
        self.assertEqual(set(answers), {"child-question", "child-permission", "child-third", "child-form"})
        self.assertEqual(answers["child-question"]["updatedInput"]["answers"], {"Which output format?": "独立 draft"})
        self.assertEqual(answers["child-form"]["content"], {"age": 31})
        self.assertTrue(answers["child-third"]["interrupt"])
        frames = self.helper_messages(terminal, "server")
        callback_index = max(i for i, frame in enumerate(frames) if frame.get("type") == "callback")
        result_index = next(i for i, frame in enumerate(frames) if frame.get("message", {}).get("type") == "result")
        self.assertLess(callback_index, result_index)
        terminal.snapshot("independent-child-callbacks")

    def test_claude_native_cancellation_withdraws_only_queued_and_active_ids(self):
        terminal = self.open_question("cancel questions")
        terminal.contains("4 waiting")
        terminal.send(b"\x1b[21~")
        terminal.contains("Child: child-three")
        terminal.contains("2 waiting")
        self.assertEqual(len(self.native_controls(terminal, "interrupt")), 1)
        terminal.send(b"n")
        terminal.contains("MCP server: counter-server")
        terminal.paste("25")
        terminal.send(b"\x13")
        terminal.contains("Native callbacks settled")
        replies = self.helper_requests(terminal, "callback.respond")
        self.assertEqual([reply["params"]["callbackId"] for reply in replies], ["child-third", "child-form"])
        withdrawn = [frame["callbackId"] for frame in self.helper_messages(terminal, "server") if frame.get("event") == "callback_cancelled"]
        self.assertEqual(set(withdrawn), {"child-question", "child-permission"})

    def test_claude_late_callback_preserves_main_composer_and_failed_reply_preserves_question(self):
        terminal = self.launch(backend="claude", resume=None, mode="claude-reply-error")
        self.claude_ready(terminal)
        terminal.paste("prepare questions")
        terminal.send(b"\r")
        terminal.contains("Answer: prepare questions")
        terminal.paste("main composer draft")
        terminal.wait(lambda: terminal.input_is("main composer draft"), "main draft before late callback")
        (terminal.root / "show-questions").touch()
        terminal.contains("Answer native questions")
        terminal.send(b"\x1b[B\x1b[B")
        terminal.paste("question draft é")
        terminal.send(b"\x13")
        terminal.contains("Try this answer again")
        terminal.contains("question draft é")
        self.assertNotIn("late-question", self.native_answers(terminal))
        terminal.send(b"\x13")
        terminal.contains("Native callbacks settled")
        terminal.wait(lambda: terminal.input_is("main composer draft"), "original main composer retained")
        self.assertEqual(self.native_answers(terminal)["late-question"]["updatedInput"]["answers"], {"Which output format?": "question draft é"})
        self.assertEqual(len(self.helper_requests(terminal, "prompt")), 1)
        terminal.snapshot("late-callback-drafts-retained")

    def test_claude_plan_approval_shows_native_plan_and_never_sends_an_extra_prompt(self):
        for choice in [b"y", b"n", b"c"]:
            terminal = self.open_question("native plan")
            terminal.contains("# Proposed native plan")
            for _ in range(8):
                terminal.send(b"\x1b[6~")
                terminal.pump(.03)
            terminal.contains("Final plan line")
            terminal.snapshot("plan-last-line")
            terminal.send(choice)
            terminal.contains("Native callbacks settled")
            answer = self.native_answers(terminal)["plan-question"]
            self.assertEqual(answer["behavior"], "allow" if choice == b"y" else "deny")
            self.assertEqual(answer.get("interrupt", False), choice == b"c")
            self.assertEqual(len(self.helper_requests(terminal, "prompt")), 1)
            terminal.close()

    def test_claude_browser_consent_displays_host_and_reports_only_known_completion_once(self):
        terminal = self.open_question("browser consent")
        terminal.contains("MCP server: counter-server")
        terminal.contains("URL host: auth.example.com")
        terminal.contains("https://auth.example.com/confirm?request=fixture")
        terminal.contains("does not confirm completion")
        terminal.snapshot("browser-consent")
        terminal.send(b"y")
        terminal.contains("Browser interaction completed for counter-server")
        terminal.contains("Native callbacks settled")
        self.assertEqual(self.native_answers(terminal)["browser-question"], {"action": "accept"})
        self.assertEqual(terminal.text.count("Browser interaction completed"), 1)

    def test_claude_decline_cancel_and_unsupported_form_never_invent_answers(self):
        for text, choice, behavior in [("ask single", b"\x1bn", "deny"), ("mcp form", b"\x1bc", "cancel"),
                                       ("unsupported form", b"\x1bn", "decline")]:
            terminal = self.open_question(text)
            if text == "unsupported form":
                terminal.contains("Cannot accept")
                terminal.send(b"y")
                self.assertEqual(self.helper_requests(terminal, "callback.respond"), [])
            terminal.send(choice)
            terminal.contains("Native callbacks settled")
            answer = list(self.native_answers(terminal).values())[0]
            self.assertEqual(answer.get("behavior", answer.get("action")), behavior)
            self.assertNotIn("content", answer)
            terminal.close()

    def test_claude_default_no_and_suppressed_rules_escape_untrusted_metadata(self):
        terminal = self.open_question("permission flags")
        terminal.contains(r"Untrusted\u{1b}[2J title")
        terminal.contains("source: plugin")
        terminal.contains("Child: child-flags")
        self.assertNotIn("accept session", terminal.text)
        terminal.send(b"1s")
        for _ in range(3):
            terminal.pump(.03)
        self.assertEqual(self.helper_requests(terminal, "callback.respond"), [])
        terminal.send(b"y")
        terminal.contains("Press y again")
        self.assertEqual(self.helper_requests(terminal, "callback.respond"), [])
        terminal.send(b"y")
        terminal.contains("Native callbacks settled")
        self.assertEqual(self.native_answers(terminal)["flag-question"], {"behavior": "allow", "toolUseID": "tool-flag-question"})

    def test_claude_phone_resize_keeps_answer_caret_and_small_screen_cancel_visible(self):
        terminal = self.open_question("mcp form", width=36)
        terminal.resize(36, 12)
        terminal.contains("Alt+C cancel")
        terminal.contains("MCP server: counter-server")
        terminal.paste("24")
        terminal.contains("› 24")
        terminal.wait(lambda: terminal.screen.cursor.y == 7 and terminal.screen.cursor.x == 6, "native answer caret at edited value")
        terminal.snapshot("phone-question-36x12")
        terminal.resize(18, 10)
        terminal.contains("Resize to edit")
        terminal.contains("Alt+C cancel")
        terminal.snapshot("small-question-18x10")
        terminal.send(b"y\x13")
        terminal.paste("must not change the draft")
        for _ in range(3):
            terminal.pump(.03)
        self.assertEqual(self.helper_requests(terminal, "callback.respond"), [])
        terminal.resize(36, 12)
        terminal.contains("› 24")
        terminal.resize(18, 10)
        terminal.contains("Resize to edit")
        terminal.send(b"\x1bc")
        terminal.wait(lambda: bool(self.native_answers(terminal)), "small terminal native cancel")
        self.assertEqual(self.native_answers(terminal)["form-question"], {"action": "cancel"})

    def test_claude_pending_form_keeps_live_settings_and_interrupt_usable(self):
        terminal = self.open_question("mcp form")
        terminal.paste("26")
        terminal.send(b"\x1b[109;5u")
        terminal.contains("Settings")
        terminal.send(b"\x1b[C\x1b[B\x1b[C\r")
        terminal.contains("› 26")
        terminal.wait(lambda: bool(self.native_controls(terminal, "set_model")), "model control while callback waits")
        self.assertEqual(self.native_controls(terminal, "set_model")[-1]["request"]["model"], "sdk-opus")
        terminal.send(b"\x1b[21~")
        terminal.wait(lambda: bool(self.native_controls(terminal, "interrupt")), "advertised question interrupt key")
        terminal.contains("› 26")
        terminal.send(b"\x13")
        terminal.contains("Native callbacks settled")
        self.assertEqual(self.native_answers(terminal)["form-question"]["content"], {"age": 26})

    def test_claude_startup_question_can_interrupt_before_initialize_and_still_answer(self):
        terminal = self.launch(mode="claude-startup-question", backend="claude", resume=None)
        terminal.contains("Allow startup hook?")
        terminal.send(b"\x1b[21~")
        terminal.wait(lambda: bool(self.native_controls(terminal, "interrupt")), "native interrupt before initialization")
        terminal.contains("Allow startup hook?")
        terminal.send(b"n")
        self.claude_ready(terminal)
        self.assertEqual(self.native_answers(terminal)["startup-question"]["behavior"], "deny")

    def test_claude_undeclared_dialog_error_keeps_interrupt_and_shutdown_usable(self):
        terminal = self.launch(backend="claude", resume=None)
        self.claude_ready(terminal)
        terminal.paste("undeclared dialog")
        terminal.send(b"\r")
        terminal.contains("request unknown-dialog, kind unknown-kind")
        terminal.send(b"\x1b")
        terminal.contains("Turn interrupted")
        self.assertEqual(len(self.native_controls(terminal, "interrupt")), 1)
        self.assertEqual(self.helper_requests(terminal, "callback.respond"), [])
        terminal.close()

    def test_claude_early_browser_completion_waits_for_explicit_consent(self):
        terminal = self.open_question("browser early")
        terminal.contains("Your consent is still required")
        self.assertEqual(self.helper_requests(terminal, "callback.respond"), [])
        terminal.snapshot("browser-completed-before-consent")
        terminal.send(b"y")
        terminal.contains("already reported completion")
        terminal.contains("Native callbacks settled")
        self.assertNotIn("has not reported completion", terminal.text)
        self.assertEqual(terminal.text.count("Browser interaction completed"), 1)
        self.assertEqual(self.native_answers(terminal)["browser-question"], {"action": "accept"})

    def test_claude_integer_precision_error_keeps_exact_input_for_correction(self):
        terminal = self.open_question("precise integer")
        terminal.paste("9007199254740993")
        terminal.send(b"\x13")
        terminal.contains("lose precision")
        terminal.contains("› 9007199254740993")
        self.assertNotIn("integer-question", self.native_answers(terminal))
        terminal.replace_input("9007199254740991")
        terminal.send(b"\x13")
        terminal.contains("Native callbacks settled")
        self.assertEqual(self.native_answers(terminal)["integer-question"]["content"], {"count": 9007199254740991})

    def test_claude_history_plan_text_never_creates_a_fake_actionable_approval(self):
        root = self.summary_root()
        session = "11111111-1111-4111-8111-111111111111"
        folder = root / "claude-home/projects" / re.sub(r"[^a-zA-Z0-9]", "-", str(root))
        folder.mkdir(parents=True)
        records = [
            {"type": "user", "uuid": "22222222-2222-4222-8222-222222222222", "parentUuid": None,
             "message": {"role": "user", "content": "Old request"}},
            {"type": "assistant", "uuid": "33333333-3333-4333-8333-333333333333",
             "parentUuid": "22222222-2222-4222-8222-222222222222", "message": {"role": "assistant", "content": [
                 {"type": "tool_use", "id": "old-plan", "name": "ExitPlanMode", "input": {"plan": "Historical plan"}}]}},
            {"type": "user", "uuid": "44444444-4444-4444-8444-444444444444",
             "parentUuid": "33333333-3333-4333-8333-333333333333", "message": {"role": "user", "content": [
                 {"tool_use_id": "old-plan", "type": "tool_result", "content": "Exit plan mode?", "is_error": True}]}}]
        (folder / f"{session}.jsonl").write_text("".join(json.dumps({**row, "cwd": str(root), "sessionId": session}) + "\n" for row in records))
        terminal = self.launch(mode="claude-startup-question", backend="claude", resume=session, directory=root)
        terminal.contains("Allow startup hook?")
        terminal.send(b"n")
        self.claude_ready(terminal)
        self.assertEqual([reply["params"]["callbackId"] for reply in self.helper_requests(terminal, "callback.respond")], ["startup-question"])
        self.assertEqual(self.helper_requests(terminal, "prompt"), [])

    def test_claude_phone_can_answer_a_single_question_with_unicode_free_text(self):
        terminal = self.open_question("ask single", width=36)
        terminal.resize(36, 12)
        terminal.wait(lambda: "PgUp/PgDn" in terminal.screen.display[-1], "question redrawn at phone height")
        terminal.send(b"\x1b[B\x1b[B")
        terminal.contains("Other")
        terminal.paste("你好 é🦊")
        terminal.contains("› 你好 é🦊")
        terminal.send(b"\x13")
        terminal.contains("Native callbacks settled")
        self.assertEqual(self.native_answers(terminal)["ask-question"]["updatedInput"]["answers"], {"Which output format?": "你好 é🦊"})
        terminal.snapshot("phone-unicode-answer")

    def test_claude_browser_decline_and_cancel_send_no_form_content(self):
        for text, key, action in [("browser consent", b"\x1bn", "decline"), ("browser early", b"\x1bc", "cancel")]:
            terminal = self.open_question(text)
            terminal.contains("URL host: auth.example.com")
            terminal.send(key)
            terminal.contains("Native callbacks settled")
            self.assertEqual(self.native_answers(terminal)["browser-question"], {"action": action})
            terminal.close()
