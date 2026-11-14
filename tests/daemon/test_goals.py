"""Drive typed root-goal reads/edits through the actual daemon and captured native writer."""

import json

from support import Client, Fixture, wait_for


class GoalTests:
    def test_goal_limits_and_new_goal_requirements_fail_before_native_writes(self):
        fixture, first, other, session, initial = self.goal_clients("goal-bounds")
        invalid = [
            {"objective": "", "token_budget": None, "status": "paused"},
            {"objective": None, "token_budget": None, "status": "active"},
            {"objective": "valid", "token_budget": "0", "status": "paused"},
            {"objective": "valid", "token_budget": "9223372036854775808", "status": "paused"},
            {"objective": "\0" * 32768, "token_budget": None, "status": "paused"},
        ]
        for counter, change in enumerate(invalid, start=2):
            operation = first.operation(counter)
            first.call("goal/set", session=session, operation=operation, expected=initial["revision"], change=change)
            self.assertEqual(first.outcome(operation)["state"]["kind"], "rejected")
        self.assertFalse(any(r.get("method") == "thread/goal/set" for r in fixture.native.requests))
        self.assertEqual(self.known_goal(other, session)["revision"], initial["revision"])

    def test_other_backends_reject_goal_controls_explicitly_without_native_parameters(self):
        fixture = Fixture("grok-goal-unavailable")
        self.addCleanup(fixture.close)
        first = Client(fixture, "unsupported-goal")
        self.addCleanup(first.close)
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="grok", cwd=str(fixture.root), name=None)["session"]
        first.outcome(create)
        response = first.response(first.send("goal/read", session=session))
        self.assertIn("does not support", response["error"]["message"])
        operation = first.operation(2)
        first.call("goal/clear", session=session, operation=operation, expected="0" * 64)
        self.assertEqual(first.outcome(operation)["state"]["kind"], "rejected")
        self.assertFalse(fixture.native_requests("thread/goal/clear"))

    def goal_clients(self, name, history=False):
        fixture = Fixture(name, codex=True)
        fixture.native.goals_enabled = True
        fixture.native.history_user = history
        self.addCleanup(fixture.close)
        first = Client(fixture, "goal-owner")
        other = Client(fixture, "goal-observer")
        self.addCleanup(first.close)
        self.addCleanup(other.close)
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="codex", cwd=str(fixture.root), name=None)["session"]
        first.outcome(create)
        first.call("session/subscribe", session=session, since=None)
        other.call("session/subscribe", session=session, since=None)
        goal = self.known_goal(first, session)
        self.assertIsNone(goal["goal"])
        self.assertFalse(any(r.get("method") in ("thread/goal/set", "thread/goal/clear") for r in fixture.native.requests))
        return fixture, first, other, session, goal

    def known_goal(self, client, session):
        return wait_for(lambda: self._known(client.call("goal/read", session=session)["goal"]), "native goal read completes")

    @staticmethod
    def _known(goal):
        return goal if goal["knowledge"] == "known" and not goal["reading"] else None

    def test_goal_changes_use_exact_revisions_share_facts_and_keep_failures_private(self):
        fixture, first, other, session, initial = self.goal_clients("goal-edits")
        change = {"objective": "Preserve source 🌍", "token_budget": "9007199254740995", "status": "paused"}
        operation = first.operation(2)
        params = dict(session=session, operation=operation, expected=initial["revision"], change=change)
        first.call("goal/set", **params)
        self.assertEqual(first.outcome(operation)["state"]["kind"], "accepted")
        self.assertEqual(first.call("goal/set", **params)["state"]["kind"], "accepted")
        native = [r for r in fixture.native.requests if r.get("method") == "thread/goal/set"]
        self.assertEqual(len(native), 1)
        self.assertEqual(native[0]["params"], {"threadId": "native-root", "origin": "user", "objective": change["objective"], "tokenBudget": 9007199254740995, "status": "paused"})
        goal = self.known_goal(other, session)
        self.assertEqual(goal["goal"]["tokens_used"], "9007199254740993")
        self.assertEqual(goal["goal"]["token_budget"], change["token_budget"])
        snapshot = other.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertEqual(snapshot["goal"], goal)
        stale = first.operation(3)
        first.call("goal/clear", session=session, operation=stale, expected=initial["revision"])
        self.assertEqual(first.outcome(stale)["state"]["kind"], "rejected")
        self.assertFalse(any(r.get("method") == "thread/goal/clear" for r in fixture.native.requests))
        rejected = first.operation(4)
        first.call("goal/set", session=session, operation=rejected, expected=goal["revision"], change={**change, "objective": "reject objective"})
        self.assertIn("PRIVATE_GOAL_REJECTION", first.outcome(rejected)["state"]["message"])
        goal = self.known_goal(other, session)
        self.assertNotIn("PRIVATE_GOAL_REJECTION", json.dumps(goal))
        other.call("sessions/watch")
        self.assertFalse(any(m.get("method") == "operation/updated" for m in other.messages))
        malformed = first.response(first.send("goal/set", session=session, operation=first.operation(5), expected=goal["revision"], change={**change, "threadId": "foreign-root"}))
        self.assertIn("error", malformed)
        cleared = first.operation(6)
        first.call("goal/clear", session=session, operation=cleared, expected=goal["revision"])
        self.assertEqual(first.outcome(cleared)["state"]["kind"], "accepted")
        self.assertIsNone(self.known_goal(other, session)["goal"])

    def test_a_held_goal_reply_keeps_reads_controls_and_private_resume_alive(self):
        fixture, first, other, session, initial = self.goal_clients("goal-held")
        fixture.native.goal_hold = True
        operation = first.operation(2)
        first.call("goal/set", session=session, operation=operation, expected=initial["revision"], change={"objective": "Held goal", "token_budget": None, "status": "paused"})
        wait_for(lambda: fixture.native.goal_pending, "native goal reply held")
        first.close()
        resumed = Client(fixture, "goal-resumed", resume=first.hello)
        self.addCleanup(resumed.close)
        self.assertTrue(resumed.hello["resumed"])
        self.assertEqual(resumed.call("operation/read", operation=operation)["state"]["kind"], "pending")
        self.assertTrue(other.call("goal/read", session=session)["goal"]["pending"])
        send = other.operation(1)
        other.call("session/send", session=session, operation=send, text="held")
        card = other.message(lambda m: m.get("method") == "session/event" and m["params"]["event"]["kind"] == "approval_requested")["params"]["event"]["approval"]
        snapshot = other.call("session/subscribe", session=session, since=None)["snapshot"]
        interrupt = other.operation(2)
        other.call("session/interrupt", session=session, operation=interrupt, turn=snapshot["info"]["current_turn"])
        other.call("session/approve", session=session, operation=other.operation(3), approval=card["id"], answer={"kind": "decision", "decision": "once"})
        self.assertEqual(other.outcome(interrupt)["state"]["kind"], "accepted")
        self.assertEqual(other.outcome(send)["state"]["kind"], "accepted")
        self.assertTrue(other.call("goal/read", session=session)["goal"]["pending"])
        fixture.native.release_goal()
        self.assertEqual(resumed.outcome(operation)["state"]["kind"], "accepted")
        self.assertEqual(len([r for r in fixture.native.requests if r.get("method") == "thread/goal/set"]), 1)

    def test_an_old_goal_reply_after_fork_cannot_mutate_the_new_root_or_strand_its_operation(self):
        fixture, first, other, session, initial = self.goal_clients("goal-fork", history=True)
        fixture.native.goal_hold = True
        operation = first.operation(2)
        first.call("goal/set", session=session, operation=operation, expected=initial["revision"], change={"objective": "Old root goal", "token_budget": None, "status": "paused"})
        wait_for(lambda: fixture.native.goal_pending, "old goal reply held")
        snapshot = other.call("session/subscribe", session=session, since=None)["snapshot"]
        page = other.call("session/history", session=session, before=snapshot["entries"][0]["id"], limit=1000)
        user = next(row for row in page["entries"] if row["kind"] == "user")
        rewind = other.operation(1)
        other.call("session/rewind", session=session, operation=rewind, entry=user["id"], text="after goal fork")
        self.assertEqual(other.outcome(rewind)["state"]["kind"], "accepted")
        self.assertEqual(first.outcome(operation)["state"]["kind"], "uncertain")
        current = self.known_goal(other, session)
        self.assertIsNone(current["goal"])
        self.assertNotEqual(current["revision"], initial["revision"])
        fixture.native.release_goal()
        after = self.known_goal(other, session)
        self.assertEqual(after["revision"], current["revision"])
        self.assertIsNone(after["goal"])
