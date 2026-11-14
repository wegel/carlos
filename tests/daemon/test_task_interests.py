"""Observe compact task facts and native polling lifetimes through actual daemon sockets."""

import asyncio
import json
import time

from support import Client, Fixture, wait_for


class TaskInterestTests:
    def native_task_reads(self, fixture):
        return len([request for request in fixture.native.requests if request.get("method") == "thread/list"])

    def task_label(self, client, session, label):
        page = client.call("task/list", session=session)
        return page if page["known"] and not page["reading"] and page["tasks"][0]["label"] == label else None

    def test_visible_task_reader_publishes_compact_facts_to_an_observer_without_polling_interest(self):
        fixture, first, other, session = self.task_clients("task-global-facts", count=1)
        fixture.native.task_agents[0]["agentNickname"] = "PRIVATE_NATIVE_TASK_LABEL"
        before = self.native_task_reads(fixture)
        first.call("task/subscribe", session=session)
        wait_for(lambda: self.native_task_reads(fixture) > before, "visible reader starts the existing native timer")
        page = self.known_tasks(first, session)
        self.assertEqual(page["tasks"][0]["label"], "PRIVATE_NATIVE_TASK_LABEL")
        event = other.message(lambda message: message.get("method") == "session/event"
            and message["params"]["event"]["kind"] == "tasks_changed"
            and message["params"]["event"]["tasks"]["known"]
            and not message["params"]["event"]["tasks"]["reading"])
        facts = event["params"]["event"]["tasks"]
        self.assertEqual(facts["counts"]["agents"], 1)
        self.assertNotIn("PRIVATE_NATIVE_TASK_LABEL", json.dumps(event))
        snapshot = other.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertEqual(snapshot["tasks"]["revision"], page["revision"])
        self.assertNotIn("PRIVATE_NATIVE_TASK_LABEL", json.dumps(snapshot["entries"]))
        first.call("task/unsubscribe", session=session)
        self.assertFalse(any(request.get("method") in ("turn/start", "turn/steer", "turn/interrupt")
                             for request in fixture.native.requests))

    def test_two_task_readers_keep_polling_after_one_closes_or_disconnects_and_last_close_releases_it(self):
        fixture, first, other, session = self.task_clients("task-reader-lifetime", count=1)
        first.call("task/subscribe", session=session)
        other.call("task/subscribe", session=session)
        self.known_tasks(first, session)
        first.call("task/unsubscribe", session=session)
        fixture.native.task_agents[0]["name"] = "Other reader still refreshes"
        wait_for(lambda: self.task_label(other, session, "Other reader still refreshes"), "remaining reader preserves native refresh")
        first.call("task/subscribe", session=session)
        first.close()
        fixture.native.task_agents[0]["name"] = "Other reader survives disconnect"
        wait_for(lambda: self.task_label(other, session, "Other reader survives disconnect"), "disconnected reader cannot stop another timer")
        other.call("task/unsubscribe", session=session)
        self.known_tasks(other, session)
        reads = self.native_task_reads(fixture)
        fixture.native.task_agents[0]["name"] = "No reader must poll this change"
        time.sleep(3.4)
        self.assertEqual(self.native_task_reads(fixture), reads)
        self.assertEqual(self.known_tasks(other, session)["tasks"][0]["label"], "Other reader survives disconnect")
        self.assertFalse(any(request.get("method") in ("turn/start", "turn/steer", "turn/interrupt", "thread/backgroundTerminals/terminate")
                             for request in fixture.native.requests))

    def test_task_reader_requires_an_accepted_session_subscription_before_starting_native_work(self):
        fixture, first, _, session = self.task_clients("task-reader-admission", count=1)
        unbound = Client(fixture, "unbound-task-reader")
        self.addCleanup(unbound.close)
        before = self.native_task_reads(fixture)
        reply = unbound.response(unbound.send("task/subscribe", session=session))
        self.assertIn("finish subscribing", reply["error"]["message"])
        unbound.call("sessions/watch")
        self.assertEqual(self.native_task_reads(fixture), before)
        first.call("task/subscribe", session=session)
        self.assertEqual(self.known_tasks(first, session)["tasks"][0]["label"], "Child 0")
        first.call("session/unsubscribe", session=session)
        reads = self.native_task_reads(fixture)
        time.sleep(3.4)
        self.assertEqual(self.native_task_reads(fixture), reads)

    def test_task_details_expose_named_native_labels_states_and_exact_process_identity_without_native_writes(self):
        fixture, first, _, session = self.task_clients("task-display-fields", count=1)
        fixture.native.task_agents[0]["agentNickname"] = "Alpha 日本語"
        fixture.native.task_agents[0]["status"] = {"type":"active", "activeFlags":["waitingOnUserInput"]}
        fixture.native.task_processes["native-root"] = [{"processId":"101", "itemId":"process-item", "osPid":123,
            "command":"sleep 100", "cwd":str(fixture.root)}]
        first.call("task/list", session=session, refresh=True)
        page = self.known_tasks(first, session)
        agent = next(task for task in page["tasks"] if task["kind"] == "agent")
        self.assertEqual(agent["label"], "Alpha 日本語")
        detail = first.call("task/detail", session=session, expected=page["revision"], task=agent["id"])
        fields = {property["field"]:property["text"] for property in detail["properties"]}
        self.assertEqual(fields["native_id"], "native-child-0")
        self.assertEqual(fields["last_turn"], "native-run")
        self.assertEqual(fields["native_state"], "active (waitingOnUserInput)")
        process = next(task for task in page["tasks"] if task["kind"] == "process")
        detail = first.call("task/detail", session=session, expected=page["revision"], task=process["id"])
        fields = {property["field"]:property["text"] for property in detail["properties"]}
        self.assertEqual((fields["native_id"], fields["owner"], fields["process_item"]), ("101", "native-root", "process-item"))
        self.assertEqual(fields["can_stop"], "true")
        self.assertFalse(any(request.get("method") in ("turn/start", "turn/steer", "turn/interrupt", "thread/backgroundTerminals/terminate")
                             for request in fixture.native.requests))

    def test_claude_overview_uses_named_readonly_fields_and_publishes_native_activity_to_other_readers(self):
        fixture = Fixture("task-claude-overview")
        self.addCleanup(fixture.close)
        first, other = Client(fixture, "task-overview"), Client(fixture, "task-observer")
        self.addCleanup(first.close)
        self.addCleanup(other.close)
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="claude", cwd=str(fixture.root), name=None)["session"]
        self.assertEqual(first.outcome(create)["state"]["kind"], "accepted")
        first.call("session/subscribe", session=session, since=None)
        other.call("session/subscribe", session=session, since=None)
        first.call("task/subscribe", session=session)
        for counter, text in [(2, "native quotas"), (3, "native thinking")]:
            operation = first.operation(counter)
            first.call("session/send", session=session, operation=operation, text=text)
            self.assertEqual(first.outcome(operation)["state"]["kind"], "accepted")
        def overview():
            page = first.call("task/overview", session=session)
            return page if any(entry["data"].get("thinking_tokens") == 43 for entry in page["entries"]) else None
        page = wait_for(overview, "native thinking and account windows enter the task overview")
        main = next(entry for entry in page["entries"] if entry["data"]["kind"] == "main")
        self.assertEqual(main["data"]["state"], "running")
        windows = [entry for entry in page["entries"] if entry["data"]["kind"] == "quota"]
        self.assertEqual(sorted(entry["data"]["used_pct"] for entry in windows), [25.0, 40.0])
        for entry in windows:
            text = first.call("task/overview/text", session=session, expected=page["revision"],
                entry=entry["id"], field="label", offset="0", limit=64)
            self.assertEqual(text["text"], entry["data"]["label"]["text"])
            self.assertTrue(text["complete"])
            self.assertEqual(text["entry"], entry["id"])
        denied = first.response(first.send("task/overview/text", session=session, expected=page["revision"],
            entry=main["id"], field="message", offset="0"))
        self.assertIn("no named field", denied["error"]["message"])
        event = other.message(lambda message: message.get("method") == "session/event"
            and message["params"]["event"]["kind"] == "tasks_changed"
            and message["params"]["event"]["tasks"]["overview_revision"] == page["revision"])
        self.assertEqual(event["params"]["event"]["tasks"]["counts"]["tasks"], 0)
        self.assertNotIn("native thinking", json.dumps(page))
        self.assertNotIn("Visible native thought", json.dumps(page))
        first.call("task/unsubscribe", session=session)
        wire = [json.loads(line) for line in (fixture.root / "claude-controller.jsonl").read_text().splitlines()]
        optional = [record for record in wire if record.get("direction") == "client"
            and record.get("message", {}).get("method") == "usage.optional"]
        self.assertEqual(len(optional), 1, "the daemon schedules one optional account read without a terminal polling clock")

    def emit_task_native(self, fixture, frame):
        async def emit():
            for link in list(fixture.native.connections):
                await fixture.native.send(link, frame)
        asyncio.run_coroutine_threadsafe(emit(), fixture.native.loop).result(timeout=3)

    def test_child_approval_arrival_and_retirement_publish_active_counts_without_native_list_refresh(self):
        fixture, first, other, session = self.task_clients("task-callback-counts", count=1)
        page = self.known_tasks(first, session)
        initial = first.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertEqual(initial["tasks"]["counts"]["active"], 0)
        reads = self.native_task_reads(fixture)
        self.emit_task_native(fixture, {"id":"child-card", "method":"item/commandExecution/requestApproval",
            "params":{"threadId":"native-child-0", "turnId":"native-run", "itemId":"child-command", "command":"true"}})
        other.message(lambda message: message.get("method") == "session/event"
            and message["params"]["event"]["kind"] == "approval_requested")
        active = first.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertEqual(active["tasks"]["counts"]["active"], 1)
        changed = other.message(lambda message: message.get("method") == "session/event"
            and message["params"]["event"]["kind"] == "tasks_changed"
            and message["params"]["event"]["tasks"]["counts"]["active"] == 1)
        self.assertEqual(changed["params"]["event"]["tasks"]["revision"], page["revision"])
        self.emit_task_native(fixture, {"method":"serverRequest/resolved",
            "params":{"threadId":"native-child-0", "requestId":"child-card"}})
        other.message(lambda message: message.get("method") == "session/event"
            and message["params"]["event"]["kind"] == "approval_resolved")
        retired = first.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertEqual(retired["tasks"]["counts"]["active"], 0)
        self.assertEqual(retired["tasks"]["revision"], page["revision"])
        self.assertEqual(self.native_task_reads(fixture), reads, "callback counts need no native list refresh")

    def test_same_run_state_changes_publish_task_facts_and_retire_old_named_detail_revisions(self):
        fixture, first, other, session = self.task_clients("task-run-state-revision", count=1)
        page = self.known_tasks(first, session)
        task = page["tasks"][0]["id"]
        for state in ["inProgress", "completed"]:
            fixture.native.task_turns["native-child-0"] = [{"id":"native-run", "status":state}]
            started = first.call("task/run", session=session, task=task, refresh=True)
            run = self.known_task_run(first, session, task, started["read"])
            updated = self.known_tasks(first, session)
            self.assertNotEqual(updated["revision"], page["revision"])
            self.assertEqual(updated["tasks"][0]["run"], page["tasks"][0]["run"], "state changes preserve the exact stop token")
            event = other.message(lambda message: message.get("method") == "session/event"
                and message["params"]["event"]["kind"] == "tasks_changed"
                and message["params"]["event"]["tasks"]["revision"] == updated["revision"])
            self.assertGreater(int(event["params"]["seq"]), int(page["seq"]))
            for method in ["task/detail", "task/text"]:
                params = dict(session=session, expected=page["revision"], task=task)
                if method == "task/text":
                    params.update(field="last_turn_state", offset="0")
                stale = first.response(first.send(method, **params))
                self.assertIn("changed", stale["error"]["message"])
            detail = first.call("task/detail", session=session, expected=updated["revision"], task=task)
            fields = {property["field"]:property["text"] for property in detail["properties"]}
            self.assertEqual(fields["last_turn_state"], state)
            self.assertEqual(run["native_state"], state)
            page = updated
