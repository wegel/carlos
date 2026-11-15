"""Drive native F2 metadata and exact task stops through actual terminals and daemon sockets."""

from test_daemon_client import daemon_support
from test_daemon_task_controls import DaemonTaskControls
from test_daemon_task_recovery import DaemonTaskRecovery

F2 = b"\x1bOQ"
F3 = b"\x1bOR"


def agent(index):
    return {"id":f"native-child-{index}", "agentNickname":"Alpha 日本語" if index == 0 else f"Child {index:02}",
        "source":{"subAgent":{}}, "parentThreadId":"native-root", "agentRole":"worker",
        "status":{"type":"active"}, "lastObservedTurn":{"id":f"run-{index}", "status":"inProgress"}}


class DaemonTasks(DaemonTaskControls, DaemonTaskRecovery):
    def daemon_tasks(self, name, count=2, processes=None, width=120):
        fixture, client, session = self.daemon_session(name)
        fixture.native.task_agents = [agent(index) for index in range(count)]
        fixture.native.task_turns = {f"native-child-{index}":[{"id":f"run-{index}", "status":"inProgress"}] for index in range(count)}
        fixture.native.task_processes = {"native-root": processes or []}
        terminal = self.attach_daemon(fixture, session, name, width=width)
        terminal.contains("history.txt" if width < 80 else "history-output 9999")
        return fixture, client, session, terminal

    def open_daemon_tasks(self, terminal, label="Alpha 日本語"):
        terminal.send(F2)
        terminal.contains("Codex agents / processes")
        terminal.contains(label)
        terminal.wait(lambda: "Reading agents" not in terminal.text, "native latest task window")

    def test_daemon_f2_stops_only_the_selected_native_agent_and_keeps_the_composer(self):
        fixture, client, session, terminal = self.daemon_tasks("tasks-agent-stop")
        terminal.paste("preserved main draft 日本語")
        self.open_daemon_tasks(terminal)
        terminal.contains("Role: worker")
        terminal.contains("Thread: native-child-0")
        terminal.send(b"x")
        terminal.contains("Stop requested; waiting for native task event")
        native = [request for request in fixture.native.requests if request.get("method") == "turn/interrupt"]
        self.assertEqual([request["params"] for request in native], [{"threadId":"native-child-0", "turnId":"run-0"}])
        self.assertEqual(len(terminal.requests("task/stop")), 1)
        self.assertEqual(terminal.requests("task/stop")[0]["params"]["task"], terminal.requests("task/run")[0]["params"]["task"])
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertFalse(any(request.get("method") == "thread/resume" and request["params"].get("threadId") != "native-root" for request in fixture.native.requests))
        terminal.paste("must not enter composer")
        terminal.wait(lambda:"must not enter composer" in (terminal.root/"keys.log").read_text(), "task panel consumes paste")
        terminal.send(F2)
        terminal.wait(lambda:terminal.input_is("preserved main draft 日本語"), "main draft after exact child stop")
        self.assertNotIn("native-child-0", terminal.text)
        terminal.snapshot("daemon-f2-agent-stop-composer")

    def test_daemon_f2_phone_process_failure_keeps_exact_identity_and_main_draft(self):
        process = {"processId":"101", "itemId":"native-process-item", "osPid":123, "command":"sleep 100", "cwd":"/tmp/native-task"}
        fixture, client, session, terminal = self.daemon_tasks("tasks-process-stop", count=0, processes=[process], width=36)
        fixture.native.task_stop_errors.add("101")
        terminal.paste("phone draft 日本語")
        terminal.wait(lambda:terminal.input_is("phone draft 日本語"), "phone draft painted before resize")
        terminal.resize(36, 14)
        terminal.wait(lambda:terminal.input_is("phone draft 日本語"), "phone resize applied before opening F2")
        self.open_daemon_tasks(terminal, "Process: 101")
        terminal.send(b"x")
        terminal.contains("Stop failed: PRIVATE_TASK_REJECTION")
        native = [request for request in fixture.native.requests if request.get("method") == "thread/backgroundTerminals/terminate"]
        self.assertEqual([request["params"] for request in native], [{"threadId":"native-root", "processId":"101"}])
        terminal.resize(18, 10)
        terminal.contains("F2 back")
        terminal.send(F2)
        terminal.wait(lambda:len(terminal.requests("task/unsubscribe")) == 1, "F2 closes the phone panel before desktop resize")
        terminal.resize(100, 28)
        terminal.wait(lambda:terminal.input_is("phone draft 日本語"), "process failure keeps composer after phone resize")
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertFalse(any(request.get("method") in ("turn/interrupt", "thread/backgroundTerminals/clean") for request in fixture.native.requests))
        terminal.snapshot("daemon-phone-exact-process-failure")

    def test_daemon_f2_two_terminals_keep_paged_selection_private_and_never_stop_a_removed_target(self):
        fixture, client, session, first = self.daemon_tasks("tasks-two-paged", count=45)
        second = self.attach_daemon(fixture, session, "tasks-two-second")
        second.contains("history-output 9999")
        first.paste("first private main draft")
        second.paste("second private main 🌍")
        self.open_daemon_tasks(first)
        self.open_daemon_tasks(second)
        first.send(b"\x1b[B" * 34)
        first.contains("Child 34")
        first.contains("Thread: native-child-34")
        second.send(b"\x1b[B")
        second.contains("Thread: native-child-1")
        first_target = first.requests("task/detail")[-1]["params"]["task"]
        second_target = second.requests("task/detail")[-1]["params"]["task"]
        self.assertNotEqual(first_target, second_target)
        fixture.native.task_agents = [row for row in fixture.native.task_agents if row["id"] != "native-child-34"]
        first.send(b"r")
        first.contains("Selected row unavailable")
        first.send(b"x")
        first.contains("use arrows")
        self.assertEqual(first.requests("task/stop"), [])
        second.send(b"r")
        second.contains("Thread: native-child-1")
        first.send(b"\x1b[B")
        first.contains("Thread: native-child-0")
        second.contains("Thread: native-child-1")
        first.send(F2)
        second.send(F2)
        first.wait(lambda:first.input_is("first private main draft"), "first main draft stays private")
        second.wait(lambda:second.input_is("second private main 🌍"), "second main draft stays private")
        self.assertEqual(first.requests("session/send"), [])
        self.assertEqual(second.requests("session/send"), [])
        self.assertFalse(any(request.get("method") == "turn/interrupt" for request in fixture.native.requests))
        first.snapshot("daemon-task-removed-paged-selection")
        second.snapshot("daemon-task-independent-selection")

    def test_daemon_f2_complete_unicode_process_command_uses_bounded_named_reads(self):
        command = "é🌍" * 18000 + "COMPLETE_NATIVE_COMMAND_END"
        process = {"processId":"unicode-process", "itemId":"unicode-item", "osPid":987,
            "command":command, "cwd":"/tmp/unicode-native"}
        fixture, client, session, terminal = self.daemon_tasks("tasks-full-command", count=0, processes=[process])
        terminal.paste("private Unicode command composer")
        self.open_daemon_tasks(terminal,"Process: unicode-process")
        terminal.wait(lambda:any(isinstance(message.get("result"), dict) and message["result"].get("complete") and
            message["result"].get("field") == "command" for message in terminal.messages("server")), "named command reaches exact EOF")
        terminal.send(b"\x1b[6~" * 220)
        terminal.contains("COMPLETE_NATIVE_COMMAND_END")
        requests = [row for row in terminal.requests("task/text") if row["params"]["field"] == "command"]
        self.assertGreaterEqual(len(requests),2)
        self.assertTrue(all(row["params"]["limit"] <= 65536 for row in requests))
        detail = terminal.requests("task/detail")[-1]["params"]
        self.assertTrue(all(row["params"]["expected"] == detail["expected"] and row["params"]["task"] == detail["task"] for row in requests))
        replies = {message["id"]:message["result"] for message in terminal.messages("server") if "id" in message and "result" in message}
        pieces = [replies[row["id"]] for row in requests]
        self.assertTrue(all(int(piece["offset"]) == int(previous["next_offset"]) for previous,piece in zip(pieces,pieces[1:])))
        self.assertEqual(int(pieces[-1]["next_offset"]),len(command.encode()))
        self.assertTrue(all(len(piece["text"].encode()) <= 65536 for piece in pieces))
        terminal.send(F2)
        terminal.wait(lambda:terminal.input_is("private Unicode command composer"), "named command leaves composer intact")
        self.assertEqual(terminal.requests("task/stop"), [])
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("daemon-task-complete-named-unicode-command")
