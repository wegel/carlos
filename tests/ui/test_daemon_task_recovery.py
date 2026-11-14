"""Retire only old task read failures and preserve private task intent across socket gaps."""

import asyncio
import json
import time

from test_daemon_client import daemon_support

F2 = b"\x1bOQ"
F5 = b"\x1b[15~"


class DaemonTaskRecovery:
    def emit_root_task_delta(self, fixture, text):
        async def emit():
            for link in list(fixture.native.connections):
                await fixture.native.notify(link, "item/agentMessage/delta", turnId="root-text-only",
                    itemId="root-text-only", delta=text)
        asyncio.run_coroutine_threadsafe(emit(), fixture.native.loop).result(timeout=3)

    def test_daemon_f2_read_failure_waits_for_new_task_facts_and_ignores_root_text_deltas(self):
        fixture, client, session, first = self.daemon_tasks("tasks-read-error-fence")
        second = self.attach_daemon(fixture, session, "read-error-second")
        second.contains("history-output 9999")
        first.paste("first failed read draft")
        second.paste("second native list draft")
        first.daemon_recorder.task_page_error_once = True
        first.send(F2)
        first.contains("Injected private task page failure")
        reads = len(first.requests("task/list"))
        self.emit_root_task_delta(fixture, "only transcript text")
        first.wait(lambda:any(message.get("params",{}).get("event",{}).get("kind") == "entry_added" and
            "only transcript text" in str(message["params"]["event"]) for message in first.messages("server")), "root-only new entry accepted after failed task page")
        self.emit_root_task_delta(fixture, " continued")
        first.wait(lambda:any(message.get("params",{}).get("event",{}).get("kind") == "text_appended" and
            message["params"]["event"].get("text") == " continued" for message in first.messages("server")), "root-only numbered delta accepted after failed task page")
        self.assertEqual(len(first.requests("task/list")), reads, "unrelated text cannot retire a private task read failure")
        first.contains("Injected private task page failure")
        self.open_daemon_tasks(second)
        second.send(b"\x1b[B")
        second.contains("Thread: native-child-1")
        first.contains("Thread: native-child-0")
        self.assertNotIn("Injected private task page failure", first.text)
        self.assertGreater(len(first.requests("task/list")), reads)
        first.send(F2)
        second.send(F2)
        first.wait(lambda:first.input_is("first failed read draft"), "failed task read keeps first composer")
        second.wait(lambda:second.input_is("second native list draft"), "fresh native read keeps second composer")
        self.assertEqual(first.requests("task/stop"), [])
        self.assertEqual(second.requests("session/send"), [])
        first.snapshot("native-task-sequence-retires-read-error")
        second.snapshot("native-task-sequence-independent-selection")

    def test_daemon_f2_disconnect_retires_only_unfinished_x_consent_and_restores_selection(self):
        fixture, client, session, terminal = self.daemon_tasks("tasks-disconnected-x", count=2)
        terminal.paste("unsent main after x disconnect")
        self.open_daemon_tasks(terminal)
        terminal.send(b"\x1b[B")
        terminal.contains("Thread: native-child-1")
        fixture.native.task_run_hold.add("native-child-1")
        terminal.send(b"x")
        terminal.wait(lambda:"native-child-1" in fixture.native.task_run_pending, "native run deliberately held before consent completes")
        terminal.contains("Reading current native run")
        recorder = terminal.daemon_recorder
        recorder.pause_subscriptions()
        self.addCleanup(recorder.resume_subscriptions)
        import socket
        for peer in list(recorder.peers):
            try:
                peer.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
        terminal.contains("F5 reconnects")
        terminal.contains("Native run read interrupted")
        self.assertEqual(terminal.requests("task/stop"), [])
        fixture.native.release_task_run("native-child-1")
        terminal.send(F5)
        terminal.wait(recorder.subscription_held.is_set, "resumed subscription deliberately unaccepted")
        terminal.send(b"x")
        terminal.contains("Wait for accepted session facts")
        self.assertEqual(terminal.requests("task/stop"), [])
        recorder.resume_subscriptions()
        terminal.contains("Thread: native-child-1")
        terminal.wait(lambda:len(terminal.requests("task/subscribe")) == 2, "visible task list restores once on accepted connection")
        self.assertEqual(terminal.requests("task/stop"), [], "reconnect does not finish old x consent automatically")
        terminal.send(b"x")
        terminal.contains("Stop requested; waiting for native task event")
        self.assertEqual(len(terminal.requests("task/stop")),1)
        native = [row["params"] for row in fixture.native.requests if row.get("method") == "turn/interrupt"]
        self.assertEqual(native,[{"threadId":"native-child-1", "turnId":"run-1"}])
        terminal.send(F2)
        terminal.wait(lambda:terminal.input_is("unsent main after x disconnect"), "x reconnect keeps composer draft")
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("native-task-reconnect-explicit-new-consent")

    def test_daemon_f2_pending_stop_stays_private_and_reconnect_never_replays_it(self):
        fixture, client, session, first = self.daemon_tasks("tasks-private-uncertain", count=1)
        second = self.attach_daemon(fixture, session, "uncertain-other")
        second.contains("history-output 9999")
        first.paste("uncertain owner composer")
        second.paste("uncertain observer composer")
        self.open_daemon_tasks(first)
        self.open_daemon_tasks(second)
        fixture.native.task_stop_hold.add("native-child-0")
        first.send(b"x")
        first.wait(lambda:"native-child-0" in fixture.native.task_stop_pending, "native stop admitted exactly once and acknowledgement held")
        first.contains("Sending stop request")
        recorder = first.daemon_recorder
        import socket
        for peer in list(recorder.peers):
            try:
                peer.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
        first.contains("Stop pending; waiting for accepted connection")
        first.contains("No automatic retry")
        self.assertNotIn("Stop pending",second.text)
        first.send(F5)
        first.wait(lambda:len(first.requests("task/subscribe")) == 2, "visible task interest restored on same daemon")
        first.contains("Thread: native-child-0")
        self.assertEqual(len(first.requests("task/stop")),1)
        self.assertEqual(len([row for row in fixture.native.requests if row.get("method") == "turn/interrupt"]),1)
        fixture.native.release_task_stop("native-child-0")
        first.contains("Stop requested; waiting for native task event")
        self.assertNotIn("Stop requested",second.text)
        self.assertEqual(second.requests("task/stop"),[])
        first.send(F2)
        second.send(F2)
        first.wait(lambda:first.input_is("uncertain owner composer"), "uncertain owner draft retained")
        second.wait(lambda:second.input_is("uncertain observer composer"), "uncertain observer draft retained")
        self.assertEqual(first.requests("session/send"),[])
        first.snapshot("private-stop-recovery-no-replay")
        second.snapshot("other-terminal-no-private-stop-outcome")

    def test_daemon_f2_malformed_native_stop_ack_stays_uncertain_and_private_after_reconnect(self):
        fixture, client, session, first = self.daemon_tasks("tasks-native-uncertain", count=1)
        second = self.attach_daemon(fixture, session, "native-uncertain-other")
        second.contains("history-output 9999")
        first.paste("uncertain native acknowledgement draft")
        self.open_daemon_tasks(first)
        self.open_daemon_tasks(second)
        fixture.native.task_stop_hold.add("native-child-0")
        first.send(b"x")
        first.wait(lambda:"native-child-0" in fixture.native.task_stop_pending, "native stop acknowledgement held")
        fixture.native.task_stop_pending["native-child-0"][1]["result"] = None
        fixture.native.release_task_stop("native-child-0")
        first.contains("stop uncertain")
        first.contains("No automatic retry")
        self.assertNotIn("stop uncertain",second.text)
        import socket
        for peer in list(first.daemon_recorder.peers):
            try:
                peer.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
        first.contains("F5 reconnects")
        first.contains("stop uncertain")
        first.send(F5)
        first.wait(lambda:len(first.requests("task/subscribe")) == 2, "uncertain owner's visible interest restored")
        first.contains("Thread: native-child-0")
        first.contains("stop uncertain")
        self.assertEqual(len(first.requests("task/stop")),1)
        self.assertEqual(len([row for row in fixture.native.requests if row.get("method") == "turn/interrupt"]),1)
        self.assertEqual(second.requests("task/stop"),[])
        first.send(F2)
        first.wait(lambda:first.input_is("uncertain native acknowledgement draft"), "uncertain acknowledgement retains main draft")
        first.snapshot("private-native-uncertain-no-replay")

    def test_daemon_f2_prior_run_rejection_remains_private_after_native_progress(self):
        fixture, client, session, first = self.daemon_tasks("tasks-prior-run-rejection", count=1)
        second = self.attach_daemon(fixture, session, "prior-run-other")
        second.contains("history-output 9999")
        first.paste("prior run owner draft")
        second.paste("prior run observer draft")
        self.open_daemon_tasks(first)
        self.open_daemon_tasks(second)
        fixture.native.task_stop_hold.add("native-child-0")
        fixture.native.task_stop_errors.add("native-child-0")
        first.send(b"x")
        first.wait(lambda:"native-child-0" in fixture.native.task_stop_pending, "old native stop held")
        fixture.native.observe_task_turn("native-child-0", "replacement-run")
        first.contains("replacement-run")
        second.contains("replacement-run")
        fixture.native.release_task_stop("native-child-0")
        first.contains("Prior-run: Stop failed: PRIVATE_TASK_REJECTION")
        self.assertNotIn("PRIVATE_TASK_REJECTION",second.text)
        self.assertEqual([row["params"] for row in fixture.native.requests if row.get("method") == "turn/interrupt"],
            [{"threadId":"native-child-0", "turnId":"run-0"}])
        self.assertEqual(second.requests("task/stop"),[])
        first.send(F2)
        second.send(F2)
        first.wait(lambda:first.input_is("prior run owner draft"), "prior-run failure preserves owner composer")
        second.wait(lambda:second.input_is("prior run observer draft"), "prior-run failure preserves observer composer")
        first.snapshot("private-prior-run-stop-rejection")

    def test_daemon_f2_four_mib_native_command_keeps_scroll_and_close_responsive(self):
        process = {"processId":"large-process", "itemId":"large-item", "osPid":321,
            "command":"a" * (4 * 1024 * 1024) + "COMPLETE_LARGE_COMMAND_END", "cwd":"/tmp"}
        fixture, client, session, terminal = self.daemon_tasks("tasks-large-responsive", count=0, processes=[process], width=36)
        terminal.paste("large command private composer")
        self.open_daemon_tasks(terminal,"Process: large-process")
        terminal.wait(lambda:any(message.get("result",{}).get("complete") and
            message["result"].get("field") == "command" for message in terminal.messages("server")), "complete four MiB native field read", timeout=30)
        started = time.monotonic()
        terminal.send(b"\x1b[6~")
        terminal.send(F2)
        terminal.wait(lambda:terminal.input_is("large command private composer"), "scroll and close after complete large field", timeout=15)
        elapsed = time.monotonic() - started
        (terminal.root/"task-latency.json").write_text(json.dumps({"command_bytes":4*1024*1024,"scroll_close_seconds":elapsed}))
        self.assertLess(elapsed,2.0,"task keys must not wait for repeated full-body wrapping")
        self.assertEqual(terminal.requests("task/stop"),[])
        self.assertEqual(terminal.requests("session/send"),[])
        terminal.snapshot("large-native-field-responsive-close")

    def test_daemon_f2_arrow_during_a_revision_gap_waits_for_the_original_selected_identity(self):
        fixture, client, session, terminal = self.daemon_tasks("tasks-arrow-gap", count=2)
        terminal.paste("arrow gap private main draft")
        self.open_daemon_tasks(terminal)
        terminal.contains("Thread: native-child-0")
        selected = terminal.requests("task/detail")[-1]["params"]["task"]
        recorder = terminal.daemon_recorder
        recorder.pause_task_pages()
        self.addCleanup(recorder.resume_task_pages)
        fixture.native.task_run_hold.add("native-child-0")
        started = client.call("task/run",session=session,task=selected,refresh=True)
        self.assertTrue(started["reading"])
        terminal.wait(recorder.task_page_held.is_set, "new task revision page held before arrow")
        terminal.contains("Reading native task page")
        terminal.send(b"\x1b[A\x1b[B")
        terminal.wait(lambda:"key code=Down" in (terminal.root/"keys.log").read_text(), "explicit arrow consumed while original position is unknown")
        recorder.resume_task_pages()
        terminal.contains("Thread: native-child-1")
        fixture.native.release_task_run("native-child-0")
        terminal.contains("Thread: native-child-1")
        self.assertEqual(terminal.requests("task/stop"),[])
        self.assertFalse(any(row.get("method") == "turn/interrupt" for row in fixture.native.requests))
        terminal.send(F2)
        terminal.wait(lambda:terminal.input_is("arrow gap private main draft"), "arrow gap keeps main composer")
        terminal.snapshot("explicit-arrow-across-task-revision-gap")

    def test_daemon_f2_private_rejection_remains_visible_while_native_metadata_refresh_waits(self):
        fixture, client, session, terminal = self.daemon_tasks("tasks-rejection-during-read", count=1)
        self.open_daemon_tasks(terminal)
        fixture.native.task_stop_errors.add("native-child-0")
        terminal.send(b"x")
        terminal.contains("Stop failed: PRIVATE_TASK_REJECTION")
        fixture.native.task_hold = True
        terminal.send(b"r")
        terminal.wait(lambda:fixture.native.task_pending is not None, "native metadata refresh held after rejected stop")
        terminal.contains("Reading agents")
        terminal.contains("Stop failed: PRIVATE_TASK_REJECTION")
        fixture.native.release_tasks()
        terminal.contains("Thread: native-child-0")
        terminal.contains("Stop failed: PRIVATE_TASK_REJECTION")
        self.assertEqual(len(terminal.requests("task/stop")),1)
        terminal.snapshot("private-stop-rejection-during-native-refresh")
