"""Answer exact native callback cards and interrupt their own query without changing shared focus."""

from native_task_questions import form, question
from support import wait_for


class TaskQuestionTests:
    def question_clients(self, name):
        fixture, first, second, session = self.task_clients(name, count=2)
        self.known_tasks(first, session)
        return fixture, first, second, session

    def question_cards(self, client, session, count):
        return wait_for(lambda:(cards if len(cards := client.call("session/subscribe", session=session, since=None)["snapshot"]["approvals"]) == count else None), "exact live native callback cards")

    def answer_card(self, client, session, counter, card, answer):
        operation = client.operation(counter)
        client.call("session/approve", session=session, operation=operation, approval=card["id"], answer=answer)
        return client.outcome(operation)["state"]

    def test_native_question_answers_keep_opaque_cards_field_keys_and_native_id_types(self):
        fixture, first, second, session = self.question_clients("question-opaque")
        fixture.native.emit_question(question(9))
        fixture.native.emit_question(question("9", thread="native-child-1", turn="run-1", secret=True))
        cards = self.question_cards(first, session, 2)
        a = next(card for card in cards if card["input"]["fields"][0]["key"] == "field-int")
        b = next(card for card in cards if card["input"]["fields"][0]["key"] == "field-str")
        self.assertTrue(a["active"])
        self.assertFalse(b["active"])
        self.assertFalse(a["blocking"] or b["blocking"])
        bad = self.answer_card(second, session, 1, b, {"kind":"input", "value":{"fields":[{"key":"foreign", "values":[], "text":"private rejected"}]}})
        self.assertEqual(bad["kind"], "rejected")
        self.assertEqual(fixture.native.task_answers, [])
        state = self.answer_card(second, session, 2, b, {"kind":"input", "value":{"fields":[{"key":"field-str", "values":[], "text":"  private 日本語  "}]}})
        self.assertEqual(state["kind"], "accepted")
        self.assertTrue(self.question_cards(first, session, 1)[0]["active"])
        self.assertEqual(fixture.native.task_answers[0], {"id":"9", "result":{"answers":{"field-str":{"answers":["user_note: private 日本語"]}}}})
        state = self.answer_card(first, session, 2, a, {"kind":"input", "value":{"fields":[{"key":"field-int", "values":["Listed value"], "text":None}]}})
        self.assertEqual(state["kind"], "accepted")
        wait_for(lambda:len(fixture.native.task_answers) == 2, "both original native responses")
        self.assertEqual(fixture.native.task_answers[1], {"id":9, "result":{"answers":{"field-int":{"answers":["Listed value"]}}}})
        self.assertEqual([row for row in fixture.native.requests if row.get("method") in ("turn/start", "turn/steer")], [])

    def test_query_interrupt_and_native_cancel_send_different_original_rpcs(self):
        fixture, first, second, session = self.question_clients("question-query-control")
        fixture.native.emit_question({"id":"command-query", "method":"item/commandExecution/requestApproval", "params":{
            "threadId":"native-child-1", "turnId":"run-1", "itemId":"command-item", "command":"pwd", "cwd":"/tmp/native-child"}})
        card = self.question_cards(first, session, 1)[0]
        state = self.answer_card(first, session, 2, card, {"kind":"interrupt_query"})
        self.assertEqual(state["kind"], "accepted")
        kept = self.question_cards(first, session, 1)[0]
        self.assertEqual(kept["id"], card["id"])
        self.assertFalse(kept["submitted"])
        self.assertEqual(fixture.native.task_answers, [])
        self.assertEqual([row["params"] for row in fixture.native.task_query_interrupts], [{"threadId":"native-child-1", "turnId":"run-1"}])
        self.assertFalse(any(row.get("params", {}).get("event", {}).get("kind") == "approval_answered" for row in first.messages))
        repeat = self.answer_card(second, session, 1, kept, {"kind":"interrupt_query"})
        self.assertEqual(repeat["kind"], "rejected")
        self.assertEqual(len(fixture.native.task_query_interrupts), 1)
        cancelled = self.answer_card(first, session, 3, kept, {"kind":"decision", "decision":"cancel"})
        self.assertEqual(cancelled["kind"], "accepted")
        wait_for(lambda:fixture.native.task_answers, "native command cancel response")
        self.assertEqual(fixture.native.task_answers, [{"id":"command-query", "result":{"decision":"cancel"}}])

    def test_form_query_without_turn_stays_pending_and_typed_answer_keeps_value_types(self):
        fixture, first, second, session = self.question_clients("question-form-types")
        fixture.native.emit_question(form("typed-form"))
        card = self.question_cards(first, session, 1)[0]
        rejected = self.answer_card(first, session, 2, card, {"kind":"interrupt_query"})
        self.assertEqual(rejected["kind"], "rejected")
        self.assertEqual(fixture.native.task_query_interrupts, [])
        self.assertEqual(self.question_cards(second, session, 1)[0]["id"], card["id"])
        values = {"count":5, "mode":"fr", "enabled":True}
        state = self.answer_card(first, session, 3, card, {"kind":"input", "value":values})
        self.assertEqual(state["kind"], "accepted")
        wait_for(lambda:fixture.native.task_answers, "typed native form content")
        self.assertEqual(fixture.native.task_answers, [{"id":"typed-form", "result":{"action":"accept", "content":values}}])

    def test_retired_opaque_card_cannot_answer_a_replacement_native_callback(self):
        fixture, first, second, session = self.question_clients("question-retired")
        fixture.native.emit_question(question(9))
        old = self.question_cards(first, session, 1)[0]
        fixture.native.resolve_question(9)
        wait_for(lambda:not second.call("session/subscribe", session=session, since=None)["snapshot"]["approvals"], "old native query retired")
        fresh = question("replacement")
        fresh["params"]["questions"][0]["id"] = "replacement-field"
        fixture.native.emit_question(fresh)
        current = self.question_cards(second, session, 1)[0]
        self.assertNotEqual(current["id"], old["id"])
        stale = self.answer_card(first, session, 2, old, {"kind":"input", "value":{"fields":[{"key":"field-int", "values":[], "text":"never redirected"}]}})
        self.assertEqual(stale["kind"], "rejected")
        self.assertEqual(fixture.native.task_answers, [])
        self.assertFalse(self.question_cards(second, session, 1)[0]["submitted"])
