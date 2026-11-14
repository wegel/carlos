"""Fence native task reads and private stop intent through actual daemon terminals."""

import json
import sys

from test_daemon_client import daemon_support

F2 = b"\x1bOQ"


class DaemonTaskControls:
    def test_daemon_f2_simultaneous_opens_share_the_held_native_list_and_keep_drafts(self):
        fixture, client, session, first = self.daemon_tasks("tasks-shared-read")
        second = self.attach_daemon(fixture, session, "shared-read-second")
        second.contains("history-output 9999")
        first.paste("first held list draft")
        second.paste("second held list draft")
        fixture.native.task_hold = True
        before = len([row for row in fixture.native.requests if row.get("method") == "thread/list"])
        first.send(F2)
        first.wait(lambda:fixture.native.task_pending is not None, "first native latest read held")
        second.send(F2)
        second.contains("Reading agents")
        first.contains("Reading agents")
        self.assertEqual(len([row for row in fixture.native.requests if row.get("method") == "thread/list"]), before+1)
        self.assertEqual(first.requests("task/list"), [])
        self.assertEqual(second.requests("task/list"), [])
        fixture.native.release_tasks()
        first.contains("Thread: native-child-0")
        second.contains("Thread: native-child-0")
        first.send(F2)
        second.send(F2)
        first.wait(lambda:first.input_is("first held list draft"), "first draft after shared list")
        second.wait(lambda:second.input_is("second held list draft"), "second draft after shared list")
        self.assertEqual(first.requests("task/stop"), [])
        self.assertEqual(second.requests("session/send"), [])
        first.snapshot("shared-native-list-first")
        second.snapshot("shared-native-list-second")

    def test_daemon_f2_same_value_run_completion_wakes_x_and_keeps_the_original_read(self):
        fixture, client, session, terminal = self.daemon_tasks("tasks-run-wakeup", count=1)
        terminal.paste("run wakeup composer 🌍")
        self.open_daemon_tasks(terminal)
        fixture.native.task_run_hold.add("native-child-0")
        terminal.send(b"x")
        terminal.contains("Reading current native run before stop")
        terminal.wait(lambda: "native-child-0" in fixture.native.task_run_pending, "native run waits")
        original = terminal.requests("task/run")[0]
        terminal.wait(lambda:any(message.get("id") == original["id"] and message.get("result",{}).get("reading") for message in terminal.messages("server")), "original run read returns its token")
        reply = next(message["result"] for message in terminal.messages("server") if message.get("id") == original["id"])
        self.assertEqual(terminal.requests("task/stop"), [])
        fixture.native.release_task_run("native-child-0")
        terminal.contains("Stop requested; waiting for native task event")
        continuations = [row for row in terminal.requests("task/run") if row["params"].get("read")]
        self.assertTrue(continuations)
        self.assertTrue(all(row["params"]["read"] == reply["read"] for row in continuations))
        self.assertTrue(all(not row["params"].get("refresh",False) for row in continuations))
        stops = [row["params"] for row in fixture.native.requests if row.get("method") == "turn/interrupt"]
        self.assertEqual(stops, [{"threadId":"native-child-0", "turnId":"run-0"}])
        self.assertEqual(len(terminal.requests("task/stop")), 1)
        terminal.send(F2)
        terminal.wait(lambda:terminal.input_is("run wakeup composer 🌍"), "wakeup preserves main draft")
        terminal.snapshot("same-value-run-wakeup")

    def test_daemon_f2_two_x_intents_never_retarget_a_superseded_original_read(self):
        fixture, client, session, first = self.daemon_tasks("tasks-superseded-x", count=2)
        second = self.attach_daemon(fixture, session, "superseded-x-second")
        second.contains("history-output 9999")
        first.paste("first x composer")
        second.paste("second x composer")
        self.open_daemon_tasks(first)
        self.open_daemon_tasks(second)
        fixture.native.task_run_hold.add("native-child-0")
        first.send(b"x")
        first.wait(lambda: "native-child-0" in fixture.native.task_run_pending, "first x native read held")
        first.contains("Reading current native run")
        second.send(b"\x1b[B")
        second.contains("Thread: native-child-1")
        fixture.native.observe_task_turn("native-child-0", "replacement-run")
        first.contains("replacement-run")
        fixture.native.release_task_run("native-child-0")
        first.contains("no stop sent")
        self.assertEqual(first.requests("task/stop"), [])
        second.contains("Thread: native-child-1")
        second.send(b"\x1b[A")
        second.contains("Thread: native-child-0")
        second.send(b"x")
        second.contains("Stop requested; waiting for native task event")
        self.assertEqual(len(second.requests("task/stop")), 1)
        first.send(b"\x1b[B")
        first.contains("Thread: native-child-1")
        second.contains("Thread: native-child-0")
        self.assertEqual(first.requests("task/stop"), [])
        first.send(F2)
        second.send(F2)
        first.wait(lambda:first.input_is("first x composer"), "first superseded draft retained")
        second.wait(lambda:second.input_is("second x composer"), "second superseding draft retained")
        self.assertEqual([row["params"] for row in fixture.native.requests if row.get("method") == "turn/interrupt"],
            [{"threadId":"native-child-0", "turnId":"replacement-run"}])
        first.snapshot("superseded-x-private-error")
        second.snapshot("superseding-x-exact-stop")

    def claude_task_terminal(self, name, mode="modern"):
        runner = sys.modules[type(self).__module__]
        daemon_support.BINARY = runner.BINARY.parent / "carlosd"
        daemon_support.ARTIFACTS = runner.ARTIFACTS / "daemon"
        fixture = daemon_support.Fixture(name, mode=mode)
        self.addCleanup(fixture.close)
        client = daemon_support.Client(fixture, name+"-owner")
        self.addCleanup(client.close)
        create = client.operation(1)
        session = client.call("session/create", operation=create, backend="claude", cwd=str(fixture.root), name="Shared Claude tasks")["session"]
        self.assertEqual(client.outcome(create)["state"]["kind"], "accepted")
        daemon_support.wait_for(lambda:client.call("session/subscribe",session=session,since=None)["snapshot"]["info"]["availability"] == "ready", "Claude daemon ready")
        terminal = self.attach_daemon(fixture,session,name,width=120)
        terminal.wait(lambda:any(message.get("result",{}).get("snapshot",{}).get("info",{}).get("availability") == "ready" for message in terminal.messages("server")), "terminal accepts Claude subscription")
        terminal.contains("Idle")
        return fixture,client,session,terminal

    def claude_task_prompts(self, fixture):
        path = fixture.root / "claude-controller.jsonl"
        return [record["message"] for line in path.read_text().splitlines()
            if (record := json.loads(line))["direction"] == "client" and record["message"].get("method") == "prompt"]

    def test_daemon_claude_f3_enqueues_during_a_live_turn_and_f2_shows_native_overview(self):
        fixture, client, session, terminal = self.claude_task_terminal("tasks-claude-queue", "claude-native-queue")
        terminal.paste("hold this turn")
        terminal.send(b"\r")
        terminal.contains("Answer: hold this turn")
        terminal.send(b"\x1bOR")
        terminal.contains("Queue")
        terminal.paste("queued 日本語")
        terminal.send(b"\r")
        terminal.wait(lambda:len(self.claude_task_prompts(fixture)) == 2, "native Claude accepts queued human while main remains running")
        prompts = self.claude_task_prompts(fixture)
        self.assertEqual([row["params"]["priority"] for row in prompts], ["later", "later"])
        self.assertEqual([row["params"]["text"] for row in prompts], ["hold this turn", "queued 日本語"])
        self.assertTrue(all(row["params"]["origin"] == {"kind":"human"} for row in prompts))
        terminal.paste("newer Claude composer 🌍")
        terminal.send(F2)
        terminal.contains("Tasks 0/0")
        terminal.contains("Main: Running")
        terminal.contains("accepted · queued")
        terminal.contains(prompts[1]["params"]["uuid"])
        self.assertEqual(terminal.requests("queue/change"), [])
        self.assertEqual([row["params"]["policy"] for row in terminal.requests("session/send")], ["steer", "queue"])
        terminal.send(F2)
        terminal.wait(lambda:terminal.input_is("newer Claude composer 🌍"), "native queue and overview preserve newer composer")
        terminal.snapshot("daemon-claude-native-queue-overview")

    def test_daemon_claude_f2_keeps_selection_and_stops_the_exact_native_task(self):
        fixture, client, session, terminal = self.claude_task_terminal("tasks-claude-stop")
        terminal.paste("native phone tasks")
        terminal.send(b"\r")
        terminal.contains("Two native tasks")
        terminal.paste("Claude main draft")
        terminal.send(F2)
        terminal.contains("Tasks 1/2")
        terminal.contains("Task alpha")
        terminal.send(b"\x1b[B")
        terminal.contains("Task beta")
        terminal.contains("x stop")
        terminal.send(b"x")
        terminal.contains("Stop requested; waiting for native task event")
        native = [record["message"] for line in (fixture.root/"rpc.jsonl").read_text().splitlines()
            if (record := json.loads(line))["direction"] == "client" and record["message"].get("type") == "control_request"]
        self.assertEqual([row["request"]["task_id"] for row in native if row["request"]["subtype"] == "stop_task"], ["beta"])
        self.assertEqual(len(terminal.requests("task/stop")),1)
        self.assertEqual(terminal.requests("task/stop")[0]["params"]["task"], terminal.requests("task/run")[0]["params"]["task"])
        terminal.send(F2)
        terminal.wait(lambda:terminal.input_is("Claude main draft"), "Claude selected task leaves main draft intact")
        self.assertEqual(len(terminal.requests("session/send")),1)
        terminal.snapshot("daemon-claude-exact-task-stop")
