"""Stop original native runs through the socket and keep results private across late replies."""

import json
import os
from pathlib import Path
import signal
import time

from support import Client, Fixture, wait_for


class TaskStopTests:
    def stop_task(self, client, session, task, counter):
        operation = client.operation(counter)
        params = dict(session=session, operation=operation, task=task["id"], expected_run=task["run"])
        client.call("task/stop", **params)
        return operation, params

    def native_stops(self, fixture):
        return [request for request in fixture.native.requests if request.get("method") in (
            "turn/interrupt", "thread/backgroundTerminals/terminate")]

    def test_task_stop_ack_targets_the_original_turn_without_completing_it_or_replaying_a_retry(self):
        fixture, first, other, session, _ = self.attached_tasks("task-stop-exact", 1)
        fixture.native.observe_task_turn("native-child-0", "native-run-a")
        page = wait_for(lambda: (page if (page := first.call("task/list", session=session))["tasks"][0]["run"] else None), "observed run")
        task = page["tasks"][0]
        operation, params = self.stop_task(first, session, task, 2)
        self.assertEqual(first.outcome(operation)["state"]["kind"], "accepted")
        first.call("task/stop", **params)
        self.assertEqual(len(self.native_stops(fixture)), 1)
        request = self.native_stops(fixture)[0]
        self.assertEqual(request["params"], {"threadId": "native-child-0", "turnId": "native-run-a"})
        current = other.call("task/list", session=session)["tasks"][0]
        self.assertEqual(current["run"], task["run"])
        self.assertEqual(current["state"], "active")
        denied = other.response(other.send("operation/read", operation=operation))
        self.assertIn("error", denied)
        self.assertFalse(any(message.get("method") == "operation/updated" and
            message["params"]["operation"] == operation for message in other.messages))

    def test_stale_unknown_and_foreign_task_stops_do_not_reach_the_native_peer(self):
        fixture, first, other, session, page = self.attached_tasks("task-stop-stale", 1)
        unknown = {**page["tasks"][0], "run": "0" * 64}
        op, _ = self.stop_task(first, session, unknown, 2)
        self.assertEqual(first.outcome(op)["state"]["kind"], "rejected")
        fixture.native.observe_task_turn("native-child-0", "a")
        original = wait_for(lambda: (task if (task := first.call("task/list", session=session)["tasks"][0])["run"] else None), "first run")
        fixture.native.observe_task_turn("native-child-0", "b")
        wait_for(lambda: first.call("task/list", session=session)["tasks"][0]["run"] != original["run"], "new run adopted")
        op, _ = self.stop_task(first, session, original, 3)
        self.assertEqual(first.outcome(op)["state"]["kind"], "rejected")
        op, _ = self.stop_task(other, session, {**original, "id": "0" * 64}, 1)
        self.assertEqual(other.outcome(op)["state"]["kind"], "rejected")
        self.assertEqual(self.native_stops(fixture), [])

    def test_a_late_task_stop_ack_cannot_complete_or_release_a_replacement_run(self):
        fixture, first, other, session, _ = self.attached_tasks("task-stop-late", 1)
        fixture.native.observe_task_turn("native-child-0", "a")
        original = wait_for(lambda: (task if (task := first.call("task/list", session=session)["tasks"][0])["run"] else None), "first run")
        fixture.native.task_stop_hold.add("native-child-0")
        operation, _ = self.stop_task(first, session, original, 2)
        wait_for(lambda: fixture.native.task_stop_pending, "old native stop awaiting acknowledgement")
        fixture.native.observe_task_turn("native-child-0", "b")
        replacement = wait_for(lambda: (task if (task := other.call("task/list", session=session)["tasks"][0])["run"] != original["run"] else None), "replacement adopted")
        fixture.native.release_task_stop("native-child-0")
        self.assertEqual(first.outcome(operation)["state"]["kind"], "accepted")
        second, _ = self.stop_task(other, session, replacement, 1)
        self.assertEqual(other.outcome(second)["state"]["kind"], "accepted")
        self.assertEqual([request["params"]["turnId"] for request in self.native_stops(fixture)], ["a", "b"])
        self.assertEqual(other.call("task/list", session=session)["tasks"][0]["run"], replacement["run"])

    def test_process_stop_ack_keeps_a_reused_native_process_id_and_private_errors(self):
        fixture, first, other, session = self.task_clients("process-stop-reuse", 0)
        fixture.native.task_processes["native-root"] = [{"processId": "process", "itemId": "item", "osPid": 11}]
        first.call("task/list", session=session, refresh=True)
        task = self.known_tasks(first, session)["tasks"][0]
        fixture.native.task_stop_hold.add("process")
        operation, _ = self.stop_task(first, session, task, 2)
        wait_for(lambda: fixture.native.task_stop_pending, "native process stop held")
        fixture.native.task_processes["native-root"][0]["osPid"] = 12
        other.call("task/list", session=session, refresh=True)
        replacement = self.known_tasks(other, session)["tasks"][0]
        self.assertEqual(replacement["id"], task["id"])
        self.assertNotEqual(replacement["run"], task["run"])
        fixture.native.release_task_stop("process")
        self.assertEqual(first.outcome(operation)["state"]["kind"], "accepted")
        self.assertEqual(first.call("task/list", session=session)["tasks"][0]["run"], replacement["run"])
        fixture.native.task_stop_errors.add("process")
        rejected, _ = self.stop_task(first, session, replacement, 3)
        result = first.outcome(rejected)
        self.assertEqual(result["state"]["kind"], "rejected")
        self.assertIn("PRIVATE_TASK_REJECTION", result["state"]["message"])
        shared = other.call("task/list", session=session)
        self.assertNotIn("PRIVATE_TASK_REJECTION", json.dumps(shared))
        detail = other.call("task/detail", session=session, task=replacement["id"], expected=shared["revision"])
        self.assertNotIn("PRIVATE_TASK_REJECTION", json.dumps(detail))

    def test_claude_task_stop_uses_the_real_helper_and_keeps_native_task_state(self):
        fixture = Fixture("claude-public-task-stop")
        self.addCleanup(fixture.close)
        first, other = Client(fixture, "claude-task-owner"), Client(fixture, "claude-task-observer")
        self.addCleanup(first.close)
        self.addCleanup(other.close)
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="claude", cwd=str(fixture.root), name=None)["session"]
        first.outcome(create)
        other.call("session/subscribe", session=session, since=None)
        send = first.operation(2)
        first.call("session/send", session=session, operation=send, text="native phone tasks")
        first.outcome(send)
        page = wait_for(lambda: (page if len((page := first.call("task/list", session=session))["tasks"]) == 2 else None), "real SDK task events")
        task = page["tasks"][1]
        operation, params = self.stop_task(first, session, task, 3)
        self.assertEqual(first.outcome(operation)["state"]["kind"], "accepted")
        first.call("task/stop", **params)
        native = [record["message"] for line in (fixture.root / "rpc.jsonl").read_text().splitlines()
            if (record := json.loads(line))["direction"] == "client" and record["message"].get("type") == "control_request"]
        stops = [request for request in native if request["request"]["subtype"] == "stop_task"]
        self.assertEqual(len(stops), 1)
        self.assertEqual(stops[0]["request"]["task_id"], "beta")
        shared = other.call("task/list", session=session)
        self.assertEqual(shared["tasks"][1]["run"], task["run"])
        self.assertEqual(shared["tasks"][1]["state"], "running")
        denied = other.response(other.send("operation/read", operation=operation))
        self.assertIn("error", denied)

    def test_a_stalled_claude_pipe_keeps_socket_reads_live_and_reports_uncertain_stops(self):
        fixture = Fixture("claude-blocked-stop")
        self.addCleanup(fixture.close)
        first, other = Client(fixture, "pipe-owner"), Client(fixture, "pipe-observer")
        self.addCleanup(first.close)
        self.addCleanup(other.close)
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="claude", cwd=str(fixture.root), name=None)["session"]
        first.outcome(create)
        first.call("session/subscribe", session=session, since=None)
        other.call("session/subscribe", session=session, since=None)
        send = first.operation(2)
        first.call("session/send", session=session, operation=send, text="native phone tasks")
        first.outcome(send)
        page = wait_for(lambda: (page if len((page := first.call("task/list", session=session))["tasks"]) == 2 else None), "native tasks")
        wait_for(lambda: first.call("session/subscribe", session=session, since=None)["snapshot"]["info"]["context"], "initial context read completes")
        native = next(record["pid"] for record in fixture.pids() if record["backend"] == "claude")
        helper = int(next(line.split()[1] for line in Path(f"/proc/{native}/status").read_text().splitlines() if line.startswith("PPid:")))

        def resume():
            try:
                os.kill(helper, signal.SIGCONT)
            except ProcessLookupError:
                pass
        self.addCleanup(resume)
        os.kill(helper, signal.SIGSTOP)
        wait_for(lambda: "T" in next(line for line in Path(f"/proc/{helper}/status").read_text().splitlines() if line.startswith("State:")), "only fixture helper is stopped")
        descriptor = os.open(f"/proc/{helper}/fd/0", os.O_WRONLY | os.O_NONBLOCK)
        try:
            # Whitespace before the next JSON frame is valid; fill only this owned fixture pipe.
            while True:
                try:
                    os.write(descriptor, b" " * 4096)
                except BlockingIOError:
                    break
        finally:
            os.close(descriptor)
        operation, _ = self.stop_task(first, session, page["tasks"][0], 3)
        trace = fixture.root / "claude-controller.jsonl"
        wait_for(lambda: any(record["direction"] == "client" and record["message"].get("method") == "task.stop"
            for line in trace.read_text().splitlines() if (record := json.loads(line))), "native writer reaches the full pipe")
        started = time.monotonic()
        identity = other.send("task/list", session=session)
        while True:
            reply = other.receive(timeout=.25)
            if reply.get("id") == identity:
                break
        self.assertLess(time.monotonic() - started, .4, "the owner must not wait for the 500-ms pipe timeout")
        self.assertEqual(len(reply["result"]["tasks"]), 2)
        self.assertEqual(first.outcome(operation)["state"]["kind"], "uncertain")
        resume()
