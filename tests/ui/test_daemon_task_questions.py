"""Drive native callback editors through actual F2/F4 terminals and the daemon."""

import socket

from test_daemon_client import daemon_support
from test_daemon_tasks import F2
from native_task_questions import form, question

F4 = b"\x1bOS"
F5 = b"\x1b[15~"
F10 = b"\x1b[21~"
SUBMIT = b"\x13"


class DaemonTaskQuestions:
    def test_daemon_structured_question_keeps_its_answer_private_while_native_queue_mode_is_on(self):
        fixture, client, session, terminal = self.daemon_tasks("question-queue-editor")
        terminal.paste("ordinary main draft before native question")
        terminal.wait(lambda:terminal.input_is("ordinary main draft before native question"), "main draft before queue mode")
        terminal.send(b"\x1bOR")
        terminal.contains("Queue")
        fixture.native.emit_question(question(9))
        original = self.question_cards(client, session)[0]
        terminal.contains("request waiting (1)")
        terminal.send(F4)
        terminal.contains("Native int field")
        self.edit_other_answer(terminal, "private callback answer 日本語")
        terminal.contains("private callback answer 日本語")
        terminal.send(b"\r" + SUBMIT)
        daemon_support.wait_for(lambda:fixture.native.task_answers, "Enter stays in the question; explicit submit answers it")
        self.assertEqual(fixture.native.task_answers, [{"id":9, "result":{"answers":{"field-int":{"answers":["user_note: private callback answer 日本語"]}}}}])
        answer = terminal.requests("session/approve")[-1]["params"]
        self.assertEqual(answer["approval"], original["id"])
        self.assertEqual(answer["answer"]["value"], {"fields":[{"key":"field-int", "values":[], "text":"private callback answer 日本語"}]})
        self.assertEqual(terminal.requests("queue/change"), [])
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertEqual(fixture.native.queue_entries, [])
        terminal.wait(lambda:terminal.input_is("ordinary main draft before native question"), "structured editor leaves composer intact in queue mode")
        terminal.snapshot("structured-question-keeps-native-queue-empty")

    def question_cards(self, client, session, count=1):
        return daemon_support.wait_for(lambda:(cards if len(cards := client.call("session/subscribe", session=session, since=None)["snapshot"]["approvals"]) == count else None), "exact pending native cards")

    def edit_other_answer(self, terminal, text):
        terminal.send(b"\x1b[B")
        terminal.paste(text)

    def disconnect_question(self, terminal, fail=False):
        recorder = terminal.daemon_recorder
        recorder.pause_subscriptions(fail=fail)
        self.addCleanup(recorder.resume_subscriptions)
        for peer in list(recorder.peers):
            try:
                peer.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
        terminal.contains("F5 reconnects")
        terminal.send(F5)
        terminal.wait(recorder.subscription_held.is_set, "same-daemon subscription deliberately unaccepted")
        return recorder

    def test_daemon_f4_two_private_question_drafts_keep_native_ids_and_older_task_selection(self):
        fixture, client, session, first, second = self.window_terminals("question-two-ui", count=257)
        first.paste("first main draft 🌍")
        second.paste("second main draft 日本語")
        first.wait(lambda:first.input_is("first main draft 🌍"), "first composer before questions")
        second.wait(lambda:second.input_is("second main draft 日本語"), "second composer before questions")
        fixture.native.emit_question(question(9, thread="native-child-256", turn="run-256"))
        fixture.native.emit_question(question("9", thread="native-child-256", turn="run-256", secret=True))
        cards = self.question_cards(client, session, 2)
        a = next(card for card in cards if card["input"]["fields"][0]["key"] == "field-int")
        b = next(card for card in cards if card["input"]["fields"][0]["key"] == "field-str")
        first.contains("request waiting (2)")
        second.contains("request waiting (2)")
        self.open_daemon_tasks(first)
        first.send(b"l")
        first.contains("Thread: native-child-256")
        selected = first.requests("task/detail")[-1]["params"]["task"]
        first.send(F4)
        first.contains("Native int field")
        first.contains("Thread: native-child-256")
        self.edit_other_answer(first, "first private answer 日本語")
        first.contains("first private answer 日本語")
        second.send(F4 + F4)
        second.contains("Native str field")
        self.edit_other_answer(second, "secret second answer 🌍")
        second.send(b"\t")
        second.contains("(hidden)")
        self.assertNotIn("secret second answer", second.text)
        current = self.question_cards(client, session, 2)
        self.assertEqual([card["id"] for card in current if card["active"]], [a["id"]])
        second.send(SUBMIT)
        daemon_support.wait_for(lambda:len(fixture.native.task_answers) == 1, "second terminal's exact native answer")
        self.assertEqual(second.requests("session/approve")[-1]["params"]["approval"], b["id"])
        self.assertEqual(second.requests("session/approve")[-1]["params"]["answer"]["value"],
            {"fields":[{"key":"field-str", "values":[], "text":"secret second answer 🌍"}]})
        self.assertEqual(fixture.native.task_answers[0], {"id":"9", "result":{"answers":{"field-str":{"answers":["user_note: secret second answer 🌍"]}}}})
        first.contains("first private answer 日本語")
        first.send(SUBMIT)
        daemon_support.wait_for(lambda:len(fixture.native.task_answers) == 2, "first terminal's original native answer")
        self.assertEqual(first.requests("session/approve")[-1]["params"]["approval"], a["id"])
        self.assertEqual(fixture.native.task_answers[1]["id"], 9)
        first.wait(lambda:first.input_is("first main draft 🌍"), "first composer after callback")
        second.wait(lambda:second.input_is("second main draft 日本語"), "second composer after callback")
        first.send(F2)
        first.contains("Thread: native-child-256")
        self.assertEqual(first.requests("task/detail")[-1]["params"]["task"], selected)
        first.send(F2)
        self.assertEqual(first.requests("session/send") + second.requests("session/send"), [])
        first.snapshot("question-first-original-composer")
        second.snapshot("question-second-secret-composer")

    def test_daemon_nonblocking_question_hides_without_sending_its_private_answer_as_chat(self):
        fixture, client, session, terminal = self.daemon_tasks("question-nonblocking-ui")
        terminal.paste("ordinary composer text")
        terminal.wait(lambda:terminal.input_is("ordinary composer text"), "ordinary composer")
        fixture.native.emit_question(question(9))
        self.question_cards(client, session)
        terminal.contains("request waiting (1)")
        terminal.send(F4)
        terminal.contains("Native int field")
        self.edit_other_answer(terminal, "private answer kept")
        terminal.contains("private answer kept")
        terminal.send(b"\x1b")
        terminal.wait(lambda:terminal.input_is("ordinary composer text"), "nonblocking question hides with draft intact")
        terminal.send(b"\r")
        terminal.contains("Answer: ordinary composer text")
        self.assertEqual([request["params"]["text"] for request in terminal.requests("session/send")], ["ordinary composer text"])
        self.assertEqual(fixture.native.task_answers, [])
        terminal.send(F4)
        terminal.contains("private answer kept")
        terminal.send(SUBMIT)
        daemon_support.wait_for(lambda:fixture.native.task_answers, "explicit answer remains a native callback")
        self.assertEqual(fixture.native.task_answers[0]["result"], {"answers":{"field-int":{"answers":["user_note: private answer kept"]}}})
        terminal.snapshot("nonblocking-private-answer")

    def test_daemon_question_reconnect_waits_for_current_subscription_before_explicit_send(self):
        fixture, client, session, terminal = self.daemon_tasks("question-reconnect-ui")
        terminal.paste("main across question gap")
        fixture.native.emit_question(question(9))
        self.question_cards(client, session)
        terminal.contains("request waiting (1)")
        terminal.send(F4)
        terminal.contains("Native int field")
        self.edit_other_answer(terminal, "answer across same daemon gap 🌍")
        terminal.contains("answer across same daemon gap 🌍")
        recorder = self.disconnect_question(terminal)
        terminal.contains("answer across same daemon gap 🌍")
        terminal.send(SUBMIT)
        terminal.contains("Wait for accepted session facts")
        self.assertEqual(terminal.requests("session/approve"), [])
        recorder.resume_subscriptions()
        terminal.wait(lambda:"Waiting for accepted session facts; F5" not in terminal.text, "new connection accepted its own subscription")
        terminal.send(SUBMIT)
        daemon_support.wait_for(lambda:fixture.native.task_answers, "one explicit answer after restore")
        self.assertEqual(len(terminal.requests("session/approve")), 1)
        self.assertEqual(fixture.native.task_answers[0]["result"], {"answers":{"field-int":{"answers":["user_note: answer across same daemon gap 🌍"]}}})
        terminal.wait(lambda:terminal.input_is("main across question gap"), "same-daemon gap keeps composer")
        terminal.snapshot("question-restored-explicit-answer")

    def test_daemon_question_f10_interrupts_original_query_and_alt_c_cancels_native_command(self):
        fixture, client, session, terminal = self.daemon_tasks("question-f10-ui")
        terminal.paste("main draft before query interrupt")
        fixture.native.emit_question({"id":"command-query", "method":"item/commandExecution/requestApproval", "params":{
            "threadId":"native-child-1", "turnId":"run-1", "itemId":"original-command", "command":"pwd", "cwd":"/tmp/native-child"}})
        card = self.question_cards(client, session)[0]
        terminal.contains("pwd")
        terminal.send(F10)
        terminal.contains("Interrupt query requested; question remains pending")
        self.assertEqual(fixture.native.task_answers, [])
        self.assertEqual([row["params"] for row in fixture.native.task_query_interrupts], [{"threadId":"native-child-1", "turnId":"run-1"}])
        self.assertEqual(terminal.requests("session/approve")[-1]["params"]["answer"], {"kind":"interrupt_query"})
        terminal.send(b"\x1bc")
        daemon_support.wait_for(lambda:fixture.native.task_answers, "native command cancel after query interrupt")
        self.assertEqual(fixture.native.task_answers, [{"id":"command-query", "result":{"decision":"cancel"}}])
        self.assertTrue(all(row["params"]["approval"] == card["id"] for row in terminal.requests("session/approve")))
        terminal.wait(lambda:terminal.input_is("main draft before query interrupt"), "query controls preserve composer")
        self.assertEqual(terminal.requests("session/interrupt"), [])
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("query-interrupt-then-native-cancel")

    def test_daemon_failed_question_restore_keeps_draft_until_explicit_successful_retry(self):
        fixture, client, session, terminal = self.daemon_tasks("question-failed-restore-ui")
        fixture.native.emit_question(question(9))
        self.question_cards(client, session)
        terminal.contains("request waiting (1)")
        terminal.send(F4)
        terminal.contains("Native int field")
        self.edit_other_answer(terminal, "private answer after rejected subscribe")
        terminal.contains("private answer after rejected subscribe")
        recorder = self.disconnect_question(terminal, fail=True)
        recorder.resume_subscriptions()
        terminal.wait(lambda:any("Injected resumed subscription failure" in row.get("error", {}).get("message", "") for row in terminal.messages("server")), "failed restore reply arrived while original card stays visible")
        terminal.send(SUBMIT)
        terminal.contains("Wait for accepted session facts")
        self.assertEqual(terminal.requests("session/approve"), [])
        terminal.contains("private answer after rejected subscribe")
        terminal.send(F5)
        terminal.wait(lambda:"Waiting for accepted session facts; F5" not in terminal.text, "explicit retry accepted current snapshot")
        terminal.send(SUBMIT)
        daemon_support.wait_for(lambda: fixture.native.task_answers, "one later explicit original answer")
        self.assertEqual(len(terminal.requests("session/approve")), 1)
        self.assertEqual(fixture.native.task_answers[0]["result"], {"answers":{"field-int":{"answers":["user_note: private answer after rejected subscribe"]}}})
        terminal.snapshot("question-rejected-restore-keeps-answer")

    def test_daemon_unsupported_native_form_mode_cannot_invent_form_content(self):
        fixture, client, session, terminal = self.daemon_tasks("question-unsupported-ui")
        fixture.native.emit_question(form("unsupported", mode="future-native-mode"))
        original = self.question_cards(client, session)[0]
        terminal.contains("Unsupported request mode")
        terminal.send(b"y" + SUBMIT)
        terminal.paste("must not become a form body")
        terminal.contains("Cannot accept")
        self.assertEqual(terminal.requests("session/approve"), [])
        self.assertEqual(fixture.native.task_answers, [])
        terminal.send(b"\x1bn")
        daemon_support.wait_for(lambda: fixture.native.task_answers, "explicit unsupported form decline")
        self.assertEqual(fixture.native.task_answers, [{"id":"unsupported", "result":{"action":"decline"}}])
        self.assertEqual(terminal.requests("session/approve")[-1]["params"]["approval"], original["id"])
        self.assertEqual(terminal.requests("session/send"), [])

    def test_daemon_failed_query_interrupt_keeps_draft_and_allows_only_explicit_original_answer(self):
        fixture, client, session, terminal = self.daemon_tasks("question-interrupt-failure-ui")
        fixture.native.task_query_interrupt_errors.add(("native-child-0", "run-0"))
        fixture.native.emit_question(question(9))
        original = self.question_cards(client, session)[0]
        terminal.contains("request waiting (1)")
        terminal.send(F4)
        terminal.contains("Native int field")
        self.edit_other_answer(terminal, "answer after failed interrupt")
        terminal.contains("answer after failed interrupt")
        terminal.send(F10)
        terminal.contains("Answer outcome uncertain")
        terminal.contains("answer after failed interrupt")
        self.assertEqual(fixture.native.task_answers, [])
        terminal.send(SUBMIT)
        daemon_support.wait_for(lambda:fixture.native.task_answers, "explicit callback answer after failed query interrupt")
        self.assertEqual(fixture.native.task_answers, [{"id":9, "result":{"answers":{"field-int":{"answers":["user_note: answer after failed interrupt"]}}}}])
        self.assertEqual([row["params"]["approval"] for row in terminal.requests("session/approve")], [original["id"], original["id"]])
        self.assertEqual(len(fixture.native.task_query_interrupts), 1)
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("query-interrupt-failure-private-answer")

    def test_daemon_retired_question_draft_never_answers_a_replacement_native_card(self):
        fixture, client, session, terminal = self.daemon_tasks("question-replaced-ui")
        fixture.native.emit_question(question(9))
        old = self.question_cards(client, session)[0]
        terminal.contains("request waiting (1)")
        terminal.send(F4)
        terminal.contains("Native int field")
        self.edit_other_answer(terminal, "retired private answer")
        terminal.contains("retired private answer")
        fixture.native.resolve_question(9)
        terminal.wait(lambda:"retired private answer" not in terminal.text, "original opaque request retired")
        fixture.native.emit_question(question("replacement"))
        replacement = self.question_cards(client, session)[0]
        self.assertNotEqual(old["id"], replacement["id"])
        terminal.contains("request waiting (1)")
        terminal.send(F4)
        terminal.contains("Native str field")
        self.assertNotIn("retired private answer", terminal.text)
        self.edit_other_answer(terminal, "fresh replacement answer")
        terminal.send(SUBMIT)
        daemon_support.wait_for(lambda:fixture.native.task_answers, "new explicit answer to replacement")
        self.assertEqual(fixture.native.task_answers, [{"id":"replacement", "result":{"answers":{"field-str":{"answers":["user_note: fresh replacement answer"]}}}}])
        self.assertEqual([row["params"]["approval"] for row in terminal.requests("session/approve")], [replacement["id"]])
        self.assertEqual(terminal.requests("session/send"), [])
