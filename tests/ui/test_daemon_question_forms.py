"""Check typed native forms, caller labels and consent through the daemon terminal."""

import json
import sys

from test_daemon_client import daemon_support
from test_daemon_task_questions import F4, F10, SUBMIT


class DaemonQuestionForms:
    def claude_question_session(self, name, text, width=100):
        runner = sys.modules[type(self).__module__]
        daemon_support.BINARY = runner.BINARY.parent / "carlosd"
        daemon_support.ARTIFACTS = runner.ARTIFACTS / "daemon"
        fixture = daemon_support.Fixture(name, mode="modern")
        self.addCleanup(fixture.close)
        client = daemon_support.Client(fixture, "question-form-owner")
        self.addCleanup(client.close)
        operation = client.operation(1)
        session = client.call("session/create", operation=operation, backend="claude", cwd=str(fixture.root), name="Native form")["session"]
        self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
        daemon_support.wait_for(lambda: client.call("session/subscribe", session=session, since=None)["snapshot"]["info"]["availability"] == "ready", "Claude native ready")
        terminal = self.attach_daemon(fixture, session, name, width=width)
        terminal.contains("Idle")
        terminal.paste("private main composer 🌍")
        terminal.wait(lambda: terminal.input_is("private main composer 🌍"), "composer before callback")
        operation = client.operation(2)
        client.call("session/send", session=session, operation=operation, text=text)
        self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
        terminal.contains("F10 interrupt")
        return fixture, client, session, terminal

    def claude_callback_answers(self, fixture):
        path = fixture.root / "rpc.jsonl"
        return {frame["response"]["request_id"]: frame["response"].get("response", {})
                for line in path.read_text().splitlines()
                if (record := json.loads(line))["direction"] == "client"
                and (frame := record["message"]).get("type") == "control_response"}

    def test_daemon_claude_phone_form_pins_caller_and_preserves_value_at_tiny_size(self):
        fixture, client, session, terminal = self.claude_question_session("claude-form-phone", "mcp form", width=36)
        terminal.resize(36, 12)
        terminal.contains("MCP server: counter-server")
        terminal.contains("Alt+C cancel")
        terminal.paste("24")
        terminal.contains("› 24")
        terminal.wait(lambda: terminal.screen.cursor.y == 7 and terminal.screen.cursor.x == 6, "same native form caret at 36x12")
        self.assertIn("MCP server: counter-server", terminal.screen.display[2])
        terminal.snapshot("daemon-form-phone-caller")
        terminal.resize(18, 10)
        terminal.contains("Resize to edit")
        terminal.send(b"y" + SUBMIT)
        terminal.paste("must not replace 24")
        for _ in range(3):
            terminal.pump(.03)
        terminal.resize(36, 12)
        terminal.contains("› 24")
        self.assertEqual(terminal.requests("session/approve"), [])
        terminal.send(SUBMIT)
        terminal.contains("Native callbacks settled")
        answers = self.claude_callback_answers(fixture)
        self.assertEqual(answers["form-question"], {"action":"accept", "content":{"age":24}})
        approval = terminal.requests("session/approve")[-1]["params"]
        self.assertEqual(approval["answer"], {"kind":"input", "value":{"age":24}})
        self.assertTrue(approval["approval"])
        terminal.wait(lambda: terminal.input_is("private main composer 🌍"), "form preserves private composer")
        self.assertEqual(terminal.requests("session/send"), [])

    def test_daemon_claude_typed_form_keeps_defaults_enum_values_and_optional_omission(self):
        fixture, client, session, terminal = self.claude_question_session("claude-form-types", "typed form")
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
        terminal.send(SUBMIT)
        terminal.contains("Native callbacks settled")
        expected = {"a_age":30, "b_email":"a@b", "c_color":"#0f0", "d_tags":["r", "w"], "e_optin":False}
        self.assertEqual(self.claude_callback_answers(fixture)["typed-question"], {"action":"accept", "content":expected})
        self.assertEqual(terminal.requests("session/approve")[-1]["params"]["answer"], {"kind":"input", "value":expected})
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("daemon-native-form-types")

    def test_daemon_claude_child_query_interrupt_without_root_turn_keeps_original_answer(self):
        fixture, client, session, terminal = self.claude_question_session("claude-child-query", "concurrent questions")
        terminal.contains("4 waiting")
        daemon_support.wait_for(lambda: client.call("session/subscribe", session=session, since=None)["snapshot"]["info"].get("current_turn") is None, "parent turn finished while children wait")
        for _ in range(4):
            if "Which output format?" in terminal.text:
                break
            terminal.send(F4)
            terminal.pump(.03)
        terminal.contains("Which output format?")
        terminal.send(b"\x1b[B\x1b[B")
        terminal.paste("child answer kept 日本語")
        terminal.contains("child answer kept 日本語")
        terminal.send(F10)
        terminal.contains("Interrupt query requested; question remains pending")
        terminal.contains("child answer kept 日本語")
        self.assertEqual(self.claude_callback_answers(fixture), {})
        sent = terminal.requests("session/approve")[-1]["params"]
        self.assertEqual(sent["answer"], {"kind":"interrupt_query"})
        self.assertEqual(terminal.requests("session/interrupt"), [])
        trace = [json.loads(line) for line in (fixture.root / "claude-controller.jsonl").read_text().splitlines()]
        self.assertTrue(any(row.get("message", {}).get("method") == "interrupt" for row in trace), trace[-10:])
        terminal.send(SUBMIT)
        daemon_support.wait_for(lambda: "child-question" in self.claude_callback_answers(fixture), "original child callback answered after query interrupt")
        answer = self.claude_callback_answers(fixture)["child-question"]
        self.assertEqual(answer["updatedInput"]["answers"], {"Which output format?":"child answer kept 日本語"})
        self.assertEqual(terminal.requests("session/approve")[-1]["params"]["approval"], sent["approval"])
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("daemon-child-exact-query-interrupt")

    def test_daemon_claude_invalid_form_keeps_exact_text_until_corrected_typed_submit(self):
        fixture, client, session, terminal = self.claude_question_session("claude-form-invalid", "mcp form")
        terminal.paste("17")
        terminal.send(SUBMIT)
        terminal.contains("at least 18")
        terminal.contains("› 17")
        self.assertEqual(terminal.requests("session/approve"), [])
        self.assertEqual(self.claude_callback_answers(fixture), {})
        terminal.send(b"\x15")
        terminal.paste("23")
        terminal.send(SUBMIT)
        terminal.contains("Native callbacks settled")
        self.assertEqual(self.claude_callback_answers(fixture)["form-question"], {"action":"accept", "content":{"age":23}})
        self.assertEqual(terminal.requests("session/approve")[-1]["params"]["answer"], {"kind":"input", "value":{"age":23}})
        terminal.snapshot("daemon-invalid-form-corrected")

    def test_daemon_claude_default_no_requires_second_submit_without_typing_review_keys(self):
        fixture, client, session, terminal = self.claude_question_session("claude-question-default-no", "ask default no")
        terminal.contains("Press Ctrl+S twice")
        terminal.send(b"\r")
        terminal.contains('Format: "Summary"')
        terminal.send(SUBMIT)
        terminal.contains("Press Ctrl+S again")
        self.assertEqual(terminal.requests("session/approve"), [])
        terminal.send(b"y")
        for _ in range(3):
            terminal.pump(.03)
        self.assertEqual(terminal.requests("session/approve"), [])
        terminal.contains('Format: "Summary"')
        terminal.send(SUBMIT)
        terminal.contains("Native callbacks settled")
        self.assertEqual(self.claude_callback_answers(fixture)["ask-question"]["updatedInput"]["answers"], {"Which output format?":"Summary"})
        self.assertEqual(len(terminal.requests("session/approve")), 1)
        terminal.snapshot("daemon-default-no-frozen-review")
