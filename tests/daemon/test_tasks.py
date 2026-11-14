"""Read typed native tasks through the real socket without sharing a client's selected row."""

import json

from support import Client, Fixture, wait_for


class TaskTests:
    def task_clients(self, name, count=40):
        fixture = Fixture(name, codex=True)
        fixture.native.task_agents = [{"id": f"native-child-{index}", "name": f"Child {index}",
            "source": {"subAgent": {}}, "parentThreadId": "native-root", "agentRole": "worker",
            "status": {"type": "notLoaded"}, "lastObservedTurn": {"id": "native-run", "status": "completed"}}
            for index in range(count)]
        self.addCleanup(fixture.close)
        first, other = Client(fixture, "task-owner"), Client(fixture, "task-observer")
        self.addCleanup(first.close)
        self.addCleanup(other.close)
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="codex", cwd=str(fixture.root), name=None)["session"]
        first.outcome(create)
        first.call("session/subscribe", session=session, since=None)
        other.call("session/subscribe", session=session, since=None)
        return fixture, first, other, session

    def known_tasks(self, client, session):
        def current():
            page = client.call("task/list", session=session)
            return page if page["known"] and not page["reading"] else None
        return wait_for(current, "native task reader completes")

    def attached_tasks(self, name, count=2):
        fixture, first, other, session = self.task_clients(name, count)
        for agent in fixture.native.task_agents:
            agent.pop("lastObservedTurn")
            agent["status"] = {"type": "active"}
        first.call("task/list", session=session, refresh=True)
        return fixture, first, other, session, self.known_tasks(first, session)

    def known_task_run(self, client, session, task, read):
        def current():
            run = client.call("task/run", session=session, task=task, read=read)
            return run if not run["reading"] else None
        return wait_for(current, "native current-run read settles")

    def test_attached_tasks_read_exact_runs_independently_without_resuming_or_stopping_children(self):
        fixture, first, other, session, page = self.attached_tasks("task-current-runs")
        a, b = [task["id"] for task in page["tasks"]]
        self.assertTrue(all(task["run"] is None for task in page["tasks"]))
        fixture.native.task_turns["native-child-0"] = [{"id": "native-live-run", "status": "inProgress"}]
        fixture.native.task_run_hold.update(["native-child-0", "native-child-1"])
        first_read = first.call("task/run", session=session, task=a)
        other_read = other.call("task/run", session=session, task=b)
        self.assertTrue(first_read["reading"])
        self.assertTrue(other_read["reading"])
        wait_for(lambda: len(fixture.native.task_run_pending) == 2, "both native run requests held")
        fixture.native.release_task_run("native-child-1")
        empty = self.known_task_run(other, session, b, other_read["read"])
        self.assertTrue(empty["known"])
        self.assertIsNone(empty["run"])
        self.assertIsNone(empty["native_state"])
        self.assertTrue(first.call("task/run", session=session, task=a)["reading"])
        fixture.native.release_task_run("native-child-0")
        live = self.known_task_run(first, session, a, first_read["read"])
        self.assertTrue(live["known"])
        self.assertIsNone(live["error"])
        self.assertEqual(live["native_state"], "inProgress")
        self.assertIsNotNone(live["run"])
        self.assertNotIn("native-", json.dumps(live))
        updated = first.call("task/list", session=session)
        self.assertEqual(updated["tasks"][0]["run"], live["run"])
        self.assertNotEqual(updated["revision"], page["revision"])
        stale = first.response(first.send("task/detail", session=session, expected=page["revision"], task=a))
        self.assertIn("error", stale)
        forged = first.response(first.send("task/run", session=session, task="0" * 64))
        self.assertIn("error", forged)
        methods = [request.get("method") for request in fixture.native.requests]
        self.assertFalse(any(method in methods for method in ("thread/resume", "turn/interrupt", "thread/backgroundTerminals/terminate")))

    def test_a_late_native_run_read_reports_its_old_intent_and_keeps_newer_turn_facts(self):
        fixture, first, other, session, page = self.attached_tasks("task-late-run", count=1)
        task = page["tasks"][0]["id"]
        fixture.native.task_turns["native-child-0"] = [{"id": "retired-run", "status": "inProgress"}]
        fixture.native.task_run_hold.add("native-child-0")
        original = first.call("task/run", session=session, task=task)
        wait_for(lambda: fixture.native.task_run_pending, "old run response held")
        fixture.native.observe_task_turn("native-child-0", "replacement-run")
        replacement = wait_for(lambda: (lambda page: page["tasks"][0]["run"])(
            other.call("task/list", session=session)), "newer native turn observed")
        fixture.native.release_task_run("native-child-0")
        refused = self.known_task_run(first, session, task, original["read"])
        self.assertIn("not retargeted", refused["error"])
        self.assertEqual(refused["run"], replacement)
        self.assertEqual(refused["native_state"], "inProgress")
        self.assertFalse(any(request.get("method") == "turn/interrupt" for request in fixture.native.requests))

    def test_a_task_run_refresh_never_retargets_an_older_clients_poll(self):
        fixture, first, other, session, page = self.attached_tasks("task-run-replaced", count=1)
        task = page["tasks"][0]["id"]
        fixture.native.task_turns["native-child-0"] = [{"id": "first-run", "status": "inProgress"}]
        first_read = first.call("task/run", session=session, task=task)
        original = self.known_task_run(first, session, task, first_read["read"])
        fixture.native.task_turns["native-child-0"] = [{"id": "second-run", "status": "inProgress"}]
        second_read = other.call("task/run", session=session, task=task, refresh=True)
        replacement = self.known_task_run(other, session, task, second_read["read"])
        self.assertNotEqual(replacement["run"], original["run"])
        self.assertNotEqual(replacement["read"], original["read"])
        stale = first.response(first.send("task/run", session=session, task=task, read=original["read"]))
        self.assertIn("superseded", stale["error"]["message"])
        malformed = first.response(first.send("task/run", session=session, task=task, read=replacement["read"], refresh=True))
        self.assertIn("refresh cannot continue", malformed["error"]["message"])

    def test_fresh_task_run_reads_and_empty_results_override_old_observed_turn_metadata(self):
        fixture, first, _, session = self.task_clients("task-run-authority", count=1)
        page = self.known_tasks(first, session)
        task = page["tasks"][0]["id"]
        self.assertIsNotNone(page["tasks"][0]["run"])
        fixture.native.task_turns["native-child-0"] = [{"id": "fresh-run", "status": "inProgress"}]
        started = first.call("task/run", session=session, task=task, refresh=True)
        current = self.known_task_run(first, session, task, started["read"])
        self.assertEqual(current["native_state"], "inProgress")
        self.assertNotEqual(current["run"], page["tasks"][0]["run"])
        fixture.native.task_turns["native-child-0"] = []
        started = first.call("task/run", session=session, task=task, refresh=True)
        empty = self.known_task_run(first, session, task, started["read"])
        self.assertTrue(empty["known"])
        self.assertIsNone(empty["run"])
        self.assertIsNone(empty["native_state"])

    def test_task_pages_and_explicit_details_are_independent_and_reject_changed_views(self):
        fixture, first, other, session = self.task_clients("task-pages")
        page = self.known_tasks(first, session)
        self.assertEqual(len(page["tasks"]), 32)
        self.assertTrue(page["has_more"])
        self.assertFalse(page["native_older_available"])
        self.assertNotIn("native-", json.dumps(page))
        last = other.call("task/list", session=session, expected=page["revision"], after=page["tasks"][-1]["id"], limit=1)
        self.assertEqual(last["tasks"][0]["label"], "Child 32")
        a = first.call("task/detail", session=session, expected=page["revision"], task=page["tasks"][0]["id"])
        b = other.call("task/detail", session=session, expected=page["revision"], task=last["tasks"][0]["id"])
        self.assertEqual(a["task"]["label"], "Child 0")
        self.assertEqual(b["task"]["label"], "Child 32")
        self.assertEqual(first.call("task/list", session=session), page)
        self.assertFalse(any(r.get("method") == "thread/resume" for r in fixture.native.requests))
        fixture.native.task_agents[0]["name"] = "Renamed child"
        first.call("task/list", session=session, refresh=True)
        current = self.known_tasks(other, session)
        self.assertEqual(current["tasks"][0]["id"], page["tasks"][0]["id"])
        self.assertEqual(current["tasks"][0]["label"], "Renamed child")
        stale = first.response(first.send("task/detail", session=session, expected=page["revision"], task=a["task"]["id"]))
        self.assertIn("error", stale)
        stale_page = first.response(first.send("task/list", session=session, expected=page["revision"], after=a["task"]["id"]))
        self.assertIn("error", stale_page)

    def test_task_field_chunks_preserve_exact_unicode_without_enlarging_metadata_replies(self):
        fixture, first, other, session = self.task_clients("task-text", count=0)
        text = "🌍\0line\n" * 100000
        fixture.native.task_processes["native-root"] = [{"processId": "native-process", "itemId": "native-item",
            "command": text, "cwd": "/tmp/é", "osPid": 9007199254740993}]
        first.call("task/list", session=session, refresh=True)
        page = self.known_tasks(first, session)
        task = page["tasks"][0]["id"]
        detail = first.call("task/detail", session=session, expected=page["revision"], task=task)
        command = next(prop for prop in detail["properties"] if prop["field"] == "command")
        self.assertFalse(command["complete"])
        self.assertEqual(command["bytes"], str(len(text.encode())))
        self.assertLess(len(json.dumps(detail)), 512 * 1024)
        offset, parts = "0", []
        while True:
            chunk = other.call("task/text", session=session, expected=page["revision"], task=task, field="command", offset=offset)
            self.assertEqual(chunk["offset"], offset)
            self.assertLessEqual(len(chunk["text"].encode()), 65536)
            parts.append(chunk["text"])
            offset = chunk["next_offset"]
            if chunk["complete"]:
                break
        self.assertEqual("".join(parts), text)
        rounded = first.call("task/text", session=session, expected=page["revision"], task=task, field="command", offset="2", limit=4)
        self.assertEqual((rounded["offset"], rounded["text"]), ("0", "🌍"))
        bad = first.response(first.send("task/text", session=session, expected=page["revision"], task=task, field="arbitrary.native.path", offset="0"))
        self.assertIn("error", bad)
        changed = {**fixture.native.task_processes["native-root"][0], "itemId": "replacement-item"}
        fixture.native.task_processes["native-root"] = [changed]
        first.call("task/list", session=session, refresh=True)
        current = self.known_tasks(first, session)
        self.assertEqual(current["tasks"][0]["id"], task)
        self.assertNotEqual(current["tasks"][0]["run"], page["tasks"][0]["run"])
        stale = first.response(first.send("task/text", session=session, expected=page["revision"], task=task, field="command", offset="0"))
        self.assertIn("error", stale)

    def test_held_native_task_read_keeps_private_prompts_and_priority_controls_responsive(self):
        fixture, first, other, session = self.task_clients("task-held", count=0)
        fixture.native.task_hold = True
        pending = first.call("task/list", session=session, refresh=True)
        self.assertTrue(pending["reading"])
        wait_for(lambda: fixture.native.task_pending, "native task reply held")
        send = other.operation(1)
        other.call("session/send", session=session, operation=send, text="held")
        card = other.message(lambda m: m.get("method") == "session/event" and m["params"]["event"]["kind"] == "approval_requested")["params"]["event"]["approval"]
        snapshot = other.call("session/subscribe", session=session, since=None)["snapshot"]
        interrupt = other.operation(2)
        other.call("session/interrupt", session=session, operation=interrupt, turn=snapshot["info"]["current_turn"])
        other.call("session/approve", session=session, operation=other.operation(3), approval=card["id"], answer={"kind": "decision", "decision": "once"})
        self.assertEqual(other.outcome(interrupt)["state"]["kind"], "accepted")
        self.assertEqual(other.outcome(send)["state"]["kind"], "accepted")
        fixture.native.release_tasks()
        self.assertEqual(self.known_tasks(first, session)["tasks"], [])

    def test_claude_tasks_keep_independent_explicit_targets_and_lossless_named_fields(self):
        fixture = Fixture("task-claude")
        self.addCleanup(fixture.close)
        first, other = Client(fixture, "task-claude-owner"), Client(fixture, "task-claude-other")
        self.addCleanup(first.close)
        self.addCleanup(other.close)
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="claude", cwd=str(fixture.root), name=None)["session"]
        self.assertEqual(first.outcome(create)["state"]["kind"], "accepted")
        operation = first.operation(2)
        first.call("session/send", session=session, operation=operation, text="native phone tasks")
        self.assertEqual(first.outcome(operation)["state"]["kind"], "accepted")
        page = wait_for(lambda: (lambda page: page if len(page["tasks"]) == 2 else None)(
            first.call("task/list", session=session)), "both native tasks arrive")
        self.assertEqual([row["label"] for row in page["tasks"]], ["First visible task", "Second visible task"])
        a = first.call("task/detail", session=session, expected=page["revision"], task=page["tasks"][0]["id"])
        b = other.call("task/detail", session=session, expected=page["revision"], task=page["tasks"][1]["id"])
        self.assertEqual(a["task"]["label"], "First visible task")
        self.assertEqual(b["task"]["label"], "Second visible task")
        self.assertTrue(all(row["run"] for row in page["tasks"]))
        self.assertEqual(first.call("task/list", session=session)["tasks"], page["tasks"])

    def test_native_task_scope_failure_stays_visible_and_unsupported_backend_is_explicit(self):
        fixture, first, _, session = self.task_clients("task-unavailable", count=0)
        fixture.native.task_errors.add("native-root")
        first.call("task/list", session=session, refresh=True)
        page = self.known_tasks(first, session)
        self.assertIn("process scope unavailable", page["read_error"])
        self.assertEqual(page["tasks"], [])
        create = first.operation(2)
        grok = first.call("session/create", operation=create, backend="grok", cwd=str(fixture.root), name=None)["session"]
        first.outcome(create)
        refused = first.response(first.send("task/list", session=grok))
        self.assertIn("does not support", refused["error"]["message"])
