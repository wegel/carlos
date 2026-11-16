"""Exercise each terminal's older native task window and exact stop target."""

from test_daemon_tasks import F2
from test_daemon_client import daemon_support


class DaemonTaskWindows:
    def test_daemon_f2_older_process_reads_complete_text_in_its_captured_window(self):
        fixture, client, session, terminal = self.daemon_tasks("task-window-long-command", count=257)
        for agent in fixture.native.task_agents:
            agent["status"] = {"type":"notLoaded"}
        fixture.native.task_agents[256]["status"] = {"type":"active"}
        command = "older command 🌍 " * 300 + "OLDER_COMMAND_END"
        fixture.native.task_processes["native-child-256"] = [{"processId":"older-long",
            "itemId":"older-long-item", "osPid":123, "command":command, "cwd":"/tmp/older"}]
        terminal.paste("older long command composer")
        self.open_daemon_tasks(terminal)
        terminal.send(b"l")
        terminal.contains("Thread: native-child-256")
        terminal.send(b"\x1b[B")
        terminal.contains("Process: older-long")
        terminal.wait(lambda:"Reading complete named task fields" not in terminal.text,
                      "captured older command finishes without rereading the same offset")
        terminal.send(b"\x1b[6~" * 25)
        terminal.contains("OLDER_COMMAND_END")
        detail = terminal.requests("task/detail")[-1]["params"]
        reads = [row for row in terminal.requests("task/text") if row["params"]["field"] == "command"]
        self.assertEqual(len(reads), 1)
        self.assertIsNotNone(detail["window"])
        self.assertTrue(all(row["params"]["window"] == detail["window"] and
            row["params"]["expected"] == detail["expected"] and
            row["params"]["task"] == detail["task"] for row in reads))
        self.assertNotEqual(client.call("task/list", session=session, limit=32)["revision"], detail["expected"])
        terminal.send(F2)
        terminal.wait(lambda:terminal.input_is("older long command composer"), "older text retains private draft")
        self.assertEqual(terminal.requests("task/stop"), [])
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("older-command-complete-captured-window")

    def test_daemon_f2_older_agent_reads_and_stops_only_its_original_native_turn(self):
        fixture, client, session, terminal, _ = self.window_terminals("task-window-agent-ui", count=257)
        terminal.paste("older child composer")
        self.open_daemon_tasks(terminal)
        terminal.send(b"l")
        terminal.contains("Thread: native-child-256")
        terminal.send(b"x")
        terminal.contains("Stop requested; waiting for native task event")
        proof = terminal.requests("task/run")[0]["params"]
        stop = terminal.requests("task/stop")[-1]["params"]
        self.assertEqual((proof["task"], proof["window"]), (stop["task"], stop["window"]))
        native = [row["params"] for row in fixture.native.requests if row.get("method") == "turn/interrupt"]
        self.assertEqual(native, [{"threadId":"native-child-256", "turnId":"run-256"}])
        terminal.resize(18, 10)
        terminal.contains("F2 back")
        terminal.send(F2)
        terminal.resize(100, 28)
        terminal.wait(lambda:terminal.input_is("older child composer"), "older child stop keeps composer")
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertFalse(any(row.get("method") == "thread/resume" and row["params"].get("threadId") != "native-root" for row in fixture.native.requests))
        terminal.snapshot("older-agent-original-turn")

    def test_daemon_f2_failed_older_window_keeps_selected_row_and_explicit_retry(self):
        fixture, client, session, terminal, _ = self.window_terminals("task-window-error-ui", count=270)
        terminal.paste("preserve 日本語 draft")
        self.open_daemon_tasks(terminal)
        terminal.send(b"\x1b[B")
        terminal.contains("Thread: native-child-1")
        selected = terminal.requests("task/detail")[-1]["params"]["task"]
        fixture.native.task_window_error = True
        terminal.send(b"l")
        terminal.contains("OLDER_NATIVE_READ_FAILED")
        terminal.contains("Thread: native-child-1")
        self.assertEqual(terminal.requests("task/detail")[-1]["params"]["task"], selected)
        fixture.native.task_window_error = False
        terminal.send(b"l")
        terminal.contains("Thread: native-child-256")
        terminal.send(F2)
        terminal.wait(lambda:terminal.input_is("preserve 日本語 draft"), "failed older read keeps private composer")
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("failed-older-explicit-retry")

    def test_daemon_f2_reconnect_retires_older_controls_and_preserves_the_private_view(self):
        fixture, client, session, terminal, _ = self.window_terminals("task-window-reconnect-ui", count=270)
        terminal.paste("older reconnect composer")
        self.open_daemon_tasks(terminal)
        terminal.send(b"l")
        terminal.contains("Thread: native-child-256")
        selected = terminal.requests("task/detail")[-1]["params"]["task"]
        fixture.stop()
        terminal.contains("F5 reconnects")
        fixture.start()
        terminal.send(b"\x1b[15~")
        terminal.wait(lambda:len(terminal.requests("hello")) == 2, "replacement daemon greeted")
        terminal.contains("Older native task window retired")
        terminal.contains("Thread: native-child-256")
        terminal.send(b"x")
        terminal.contains("Refresh current task facts before stopping")
        self.assertEqual(terminal.requests("task/run"), [])
        self.assertEqual(terminal.requests("task/stop"), [])
        self.assertEqual(terminal.requests("task/detail")[-1]["params"]["task"], selected)
        terminal.send(b"r")
        terminal.contains("Selected row unavailable")
        terminal.send(b"\x1b[B")
        terminal.contains("Thread: native-child-0")
        terminal.send(F2)
        terminal.wait(lambda:terminal.input_is("older reconnect composer"), "reconnect retains composer")
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("older-window-reconnect-explicit-latest")

    def window_terminals(self, name, count=600):
        fixture, client, session, first = self.daemon_tasks(name, count=count)
        for agent in fixture.native.task_agents:
            agent["status"] = {"type":"notLoaded"}
        second = self.attach_daemon(fixture, session, name+"-second")
        second.contains("history-output 9999")
        return fixture, client, session, first, second

    def test_daemon_f2_older_windows_keep_two_selections_and_composer_drafts_private(self):
        fixture, client, session, first, second = self.window_terminals("task-windows-two")
        first.paste("first private older draft 🌍")
        second.paste("second private older draft 日本語")
        self.open_daemon_tasks(first)
        self.open_daemon_tasks(second)
        second.send(b"\x1b[B")
        second.contains("Thread: native-child-1")
        first.send(b"l")
        first.contains("Private older native task window")
        first.contains("Thread: native-child-256")
        second.contains("Thread: native-child-1")
        second.send(b"l")
        second.contains("Thread: native-child-256")
        second.send(b"\x1b[B"*2)
        second.contains("Thread: native-child-258")
        first.send(b"l")
        first.contains("Thread: native-child-512")
        second.contains("Thread: native-child-258")
        a, b = first.requests("task/detail")[-1]["params"], second.requests("task/detail")[-1]["params"]
        self.assertNotEqual(a["window"], b["window"])
        self.assertNotEqual(a["task"], b["task"])
        first.send(b"r")
        first.contains("Selected row unavailable")
        first.send(b"\x1b[B")
        first.contains("Thread: native-child-0")
        second.contains("Thread: native-child-258")
        first.send(F2)
        second.send(F2)
        first.wait(lambda:first.input_is("first private older draft 🌍"), "first private composer")
        second.wait(lambda:second.input_is("second private older draft 日本語"), "second private composer")
        self.assertEqual(first.requests("session/send"), [])
        self.assertEqual(second.requests("session/send"), [])
        self.assertEqual(first.requests("task/stop"), [])
        first.snapshot("older-window-latest-composer")
        second.snapshot("older-window-independent-composer")

    def test_daemon_f2_held_older_window_keeps_the_page_and_releases_on_close(self):
        fixture, client, session, first, second = self.window_terminals("task-window-held-ui", count=270)
        first.paste("held older composer")
        self.open_daemon_tasks(first)
        fixture.native.task_window_hold = True
        first.send(b"l")
        first.contains("Reading older tasks; current page kept")
        first.contains("Thread: native-child-0")
        daemon_support.wait_for(lambda:fixture.native.task_window_pending, "older native read held")
        first.wait(lambda:first.requests("task/window/read"), "original captured window is known")
        original = first.requests("task/window/read")[-1]["params"]["window"]
        self.open_daemon_tasks(second)
        second.send(b"\x1b[B")
        second.contains("Thread: native-child-1")
        first.send(F2)
        first.wait(lambda:first.input_is("held older composer"), "closing pending window retains draft")
        first.wait(lambda:any(row["params"]["window"] == original for row in first.requests("task/window/close")), "close releases only the original window")
        self.assertEqual([row["params"]["window"] for row in first.requests("task/window/close")], [original])
        fixture.native.release_task_window()
        first.send(F2)
        first.contains("Thread: native-child-0")
        self.assertNotIn("Private older native task window", first.text)
        self.assertEqual(first.requests("task/stop"), [])
        self.assertEqual(first.requests("session/send"), [])
        first.snapshot("held-older-close-original-page")

    def test_daemon_f2_older_process_checks_original_pid_before_native_stop(self):
        fixture, client, session, terminal = self.daemon_tasks("task-window-process-ui", count=257)
        for agent in fixture.native.task_agents:
            agent["status"] = {"type":"notLoaded"}
        fixture.native.task_agents[256]["status"] = {"type":"active"}
        process = {"processId":"older-process", "itemId":"older-item", "osPid":123, "command":"sleep original older", "cwd":"/tmp/older"}
        fixture.native.task_processes["native-child-256"] = [process]
        terminal.paste("older process composer")
        self.open_daemon_tasks(terminal)
        terminal.send(b"l")
        terminal.contains("Thread: native-child-256")
        terminal.send(b"\x1b[B")
        terminal.contains("Process: older-process")
        fixture.native.task_process_hold.add("native-child-256")
        terminal.send(b"x")
        daemon_support.wait_for(lambda:fixture.native.task_process_pending, "native original PID proof held")
        fixture.native.observe_task_turn("native-child-0", "unrelated-latest-turn")
        self.assertEqual(terminal.requests("task/stop"), [])
        fixture.native.release_task_process("native-child-256")
        terminal.contains("Stop requested; waiting for native task event")
        stop = terminal.requests("task/stop")[-1]["params"]
        proof = terminal.requests("task/run")[0]["params"]
        self.assertEqual((stop["task"], stop["window"]), (proof["task"], proof["window"]))
        native = [row for row in fixture.native.requests if row.get("method") == "thread/backgroundTerminals/terminate"]
        self.assertEqual([row["params"] for row in native], [{"threadId":"native-child-256", "processId":"older-process"}])
        terminal.resize(26, 14)
        terminal.contains("F2 back")
        terminal.send(F2)
        terminal.resize(100, 28)
        terminal.wait(lambda:terminal.input_is("older process composer"), "exact older stop retains composer")
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("older-process-original-pid")
