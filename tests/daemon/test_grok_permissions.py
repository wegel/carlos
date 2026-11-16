"""Keep real Grok choices on shared cards and preserve native callback identity."""

import json

from native_grok_permissions import OPTIONS
from support import Client, Fixture, wait_for


class GrokPermissionTests:
    def test_grok_waits_for_every_offered_choice_and_preserves_integer_and_string_ids(self):
        for identity in ("number", "string"):
            for selected in range(len(OPTIONS)):
                with self.subTest(identity=identity, selected=selected):
                    self.choose_native_permission(identity, selected)

    def choose_native_permission(self, identity, selected):
        fixture = Fixture(f"grok-permission-{identity}-{selected}", mode=f"grok-permission-{identity}")
        try:
            first = Client(fixture, "chooser")
            second = Client(fixture, "observer")
            try:
                created = first.operation(1)
                session = first.call("session/create", operation=created, backend="grok", cwd=str(fixture.root), name=None)["session"]
                first.outcome(created)
                first.call("session/subscribe", session=session, since=None)
                second.call("session/subscribe", session=session, since=None)
                sent = first.operation(2)
                first.call("session/send", session=session, operation=sent, text="permission fixture")
                card = self.native_permission_card(first, fixture, session)
                self.assertEqual(card["display_id"], json.dumps("91001" if identity == "string" else 91001))
                self.assertNotIn("session", card["choices"], "remembered native scope is not inherently this session")
                field = card["input"]["fields"][0]
                self.assertEqual([option["label"] for option in field["options"]], [option["name"] for option in OPTIONS])
                self.assertFalse(field["other"])
                for option in OPTIONS:
                    self.assertNotIn(option["optionId"], json.dumps(card), "native option ids stay private")
                answer = first.operation(3)
                first.call("session/approve", session=session, operation=answer, approval=card["id"], answer={
                    "kind":"input", "value":{"fields":[{"key":field["key"], "values":[field["options"][selected]["value"]], "text":None}]},
                })
                self.assertEqual(first.outcome(answer)["state"]["kind"], "accepted")
                answers = wait_for(lambda: self.native_permission_answers(fixture), "one original native permission answer")
                self.assertEqual(answers, [{"jsonrpc":"2.0", "id":"91001" if identity == "string" else 91001,
                    "result":{"outcome":{"outcome":"selected", "optionId":OPTIONS[selected]["optionId"]}}}])
                self.assertEqual(first.outcome(sent)["state"]["kind"], "accepted")
                self.assertEqual(second.call("session/subscribe", session=session, since=None)["snapshot"]["approvals"], [])
                self.assertEqual(len(fixture.native_requests("session/prompt")), 1)
                self.assertFalse(fixture.native_requests("session/new")[0]["params"].get("_meta", {}).get("yoloMode"))
                launches = [json.loads(line) for line in (fixture.root / "native-pids.jsonl").read_text().splitlines()]
                self.assertFalse(any("--always-approve" in launch["argv"] for launch in launches if launch["backend"] == "grok"))
            finally:
                first.close()
                second.close()
        finally:
            fixture.close()

    def permission_fixture(self, name, mode=None):
        fixture = Fixture("grok-permission-" + name, mode=mode or "grok-permission-number")
        self.addCleanup(fixture.close)
        client = Client(fixture, "chooser")
        self.addCleanup(client.close)
        created = client.operation(1)
        session = client.call("session/create", operation=created, backend="grok", cwd=str(fixture.root), name=None)["session"]
        if mode != "grok-permission-startup":
            client.outcome(created)
        client.call("session/subscribe", session=session, since=None)
        return fixture, client, session

    def permission_prompt(self, client, session):
        sent = client.operation(2)
        client.call("session/send", session=session, operation=sent, text="permission fixture")
        self.assertEqual(client.outcome(sent)["state"]["kind"], "accepted")

    def answer_permission(self, client, session, card, counter=3, selected=0):
        field = card["input"]["fields"][0]
        operation = client.operation(counter)
        client.call("session/approve", session=session, operation=operation, approval=card["id"], answer={
            "kind":"input", "value":{"fields":[{"key":field["key"], "values":[field["options"][selected]["value"]], "text":None}]},
        })
        return client.outcome(operation)["state"]["kind"]

    def test_grok_can_answer_a_startup_permission_before_native_ready(self):
        fixture, client, session = self.permission_fixture("startup", "grok-permission-startup")
        card = self.native_permission_card(client, fixture, session)
        self.assertEqual(client.call("session/subscribe", session=session, since=None)["snapshot"]["info"]["availability"], "starting")
        self.assertEqual(self.answer_permission(client, session, card), "accepted")
        wait_for(lambda: client.call("session/subscribe", session=session, since=None)["snapshot"]["info"]["availability"] == "ready", "native startup resumed after the chosen option")
        self.assertEqual(len(self.native_permission_answers(fixture)), 1)
        self.assertEqual(fixture.native_requests("session/prompt"), [])

    def test_grok_malformed_options_can_only_be_cancelled(self):
        fixture, client, session = self.permission_fixture("malformed", "grok-permission-malformed")
        self.permission_prompt(client, session)
        card = self.native_permission_card(client, fixture, session)
        self.assertEqual(card["input"]["kind"], "unsupported")
        self.assertEqual(card["choices"], ["cancel"])
        operation = client.operation(3)
        client.call("session/approve", session=session, operation=operation, approval=card["id"], answer={"kind":"decision", "decision":"cancel"})
        self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
        self.assertEqual(wait_for(lambda: self.native_permission_answers(fixture), "cancelled native permission")[0]["result"], {"outcome":{"outcome":"cancelled"}})

    def test_grok_reused_native_id_retires_the_old_card_without_replaying_its_answer(self):
        fixture, client, session = self.permission_fixture("reused")
        self.permission_prompt(client, session)
        old = self.native_permission_card(client, fixture, session)
        (fixture.root / "permission-step").write_text("replace")
        fresh = wait_for(lambda: next((card for card in client.call("session/subscribe", session=session, since=None)["snapshot"]["approvals"] if card["id"] != old["id"]), None), "replacement card")
        self.assertEqual(self.answer_permission(client, session, old), "rejected")
        self.assertEqual(self.native_permission_answers(fixture), [])
        self.assertEqual(self.answer_permission(client, session, fresh, counter=4, selected=2), "accepted")
        self.assertEqual(wait_for(lambda: self.native_permission_answers(fixture), "replacement answer")[0]["result"]["outcome"]["optionId"], OPTIONS[2]["optionId"])

    def test_grok_interrupt_cancels_pending_and_late_permissions_without_allowing(self):
        fixture, client, session = self.permission_fixture("interrupt")
        self.permission_prompt(client, session)
        old = self.native_permission_card(client, fixture, session)
        turn = client.call("session/subscribe", session=session, since=None)["snapshot"]["info"]["current_turn"]
        operation = client.operation(3)
        client.call("session/interrupt", session=session, operation=operation, turn=turn)
        self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
        wait_for(lambda: self.native_permission_answers(fixture), "native pending permission cancelled")
        self.assertEqual(self.answer_permission(client, session, old, counter=4), "rejected")
        (fixture.root / "permission-step").write_text("late")
        answers = wait_for(lambda: (answers if len(answers := self.native_permission_answers(fixture)) == 2 else None), "late permission cancelled")
        self.assertTrue(all(answer["result"] == {"outcome":{"outcome":"cancelled"}} for answer in answers))
        self.assertEqual(client.call("session/subscribe", session=session, since=None)["snapshot"]["approvals"], [])
        self.assertEqual(len(fixture.native_requests("session/prompt")), 1)

    def test_grok_eof_retires_the_card_and_another_client_cannot_answer_it(self):
        fixture, client, session = self.permission_fixture("eof")
        self.permission_prompt(client, session)
        old = self.native_permission_card(client, fixture, session)
        observer = Client(fixture, "observer")
        self.addCleanup(observer.close)
        (fixture.root / "permission-step").write_text("close")
        wait_for(lambda: not observer.call("session/subscribe", session=session, since=None)["snapshot"]["approvals"], "closed native owner retired its card")
        self.assertEqual(self.answer_permission(observer, session, old), "rejected")
        self.assertEqual(self.native_permission_answers(fixture), [])

    def test_grok_invalid_choice_is_retryable_and_two_clients_cannot_answer_twice(self):
        fixture, client, session = self.permission_fixture("race")
        self.permission_prompt(client, session)
        card = self.native_permission_card(client, fixture, session)
        field = card["input"]["fields"][0]
        invalid = client.operation(3)
        client.call("session/approve", session=session, operation=invalid, approval=card["id"], answer={
            "kind":"input", "value":{"fields":[{"key":field["key"], "values":["native:once:α"], "text":None}]},
        })
        self.assertEqual(client.outcome(invalid)["state"]["kind"], "rejected")
        self.assertEqual(self.native_permission_answers(fixture), [])
        other = Client(fixture, "racing-choice")
        self.addCleanup(other.close)
        params = {"session":session, "approval":card["id"], "answer":{"kind":"input", "value":{
            "fields":[{"key":field["key"], "values":[field["options"][0]["value"]], "text":None}]}}}
        first, second = client.operation(4), other.operation(1)
        a = client.send("session/approve", operation=first, **params)
        b = other.send("session/approve", operation=second, **params)
        client.response(a)
        other.response(b)
        states = [client.outcome(first)["state"]["kind"], other.outcome(second)["state"]["kind"]]
        self.assertEqual(sorted(states), ["accepted", "rejected"])
        self.assertEqual(len(wait_for(lambda: self.native_permission_answers(fixture), "one winning native answer")), 1)

    def native_permission_answers(self, fixture):
        path = fixture.root / "permission-answers.jsonl"
        return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []

    def native_permission_card(self, client, fixture, session):
        def observed():
            snapshot = client.call("session/subscribe", session=session, since=None)["snapshot"]
            if snapshot["approvals"]:
                return snapshot["approvals"][0]
            answers = self.native_permission_answers(fixture)
            return {"unprompted":answers} if answers else None
        card = wait_for(observed, "native permission became a shared card")
        self.assertNotIn("unprompted", card, "Grok must wait for an explicit offered choice")
        return card
