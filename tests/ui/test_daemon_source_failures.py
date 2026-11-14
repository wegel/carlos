"""Keep failed source reads paused and release failed clipboard jobs in a real terminal."""

import argparse
import asyncio
from contextlib import contextmanager
import fcntl
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from wcwidth import wcswidth

from terminal import Terminal
from test_daemon_client import DaemonClient, daemon_support
from test_daemon_source_recovery import DaemonSourceRecovery


DIRECT_FAILURE = "Injected direct source read failure"
WATCH_FAILURE = "Injected daemon source read failure"


def body(name, size):
    text, tail = "```text\n" + name + " HEAD\n", "\n" + name + " TAIL\n```\n"
    remaining = size - len((text + tail).encode())
    lines, index = [], 0
    while True:
        line = f"{name} row-{index:05} value 🌍 with spaces\n"
        if len(line.encode()) > remaining:
            break
        lines.append(line)
        remaining -= len(line.encode())
        index += 1
    return text + "".join(lines) + "x" * remaining + tail


def history(items):
    return {"id":"native-root", "status":{"type":"idle"}, "model":"fixture-model",
            "reasoningEffort":"low", "turns":[{"id":"history-turn", "status":"completed", "items":items}]}


class DaemonSourceFailures(unittest.TestCase):
    daemon_session = DaemonClient.daemon_session
    attach_daemon = DaemonClient.attach_daemon
    running_sources = DaemonSourceRecovery.running_sources
    source_history = staticmethod(DaemonSourceRecovery.source_history)
    source_append = staticmethod(DaemonSourceRecovery.source_append)

    def summary_root(self):
        root = ARTIFACTS / self._testMethodName
        root.mkdir(parents=True, exist_ok=True)
        return root

    def launch(self, directory, width, resume, extra_args, extra_env=None):
        terminal = Terminal(BINARY, directory, resume=resume, width=width,
                            extra_args=extra_args, extra_env=extra_env)
        self.addCleanup(terminal.close)
        return terminal

    @staticmethod
    def entries(client, session):
        return client.call("session/subscribe", session=session, since=None)["snapshot"]["entries"]

    @staticmethod
    def reads(terminal, entry):
        return [request for request in terminal.requests("entry/source")
                if request["params"]["entry"] == entry]

    def injected(self, terminal, reason, entry):
        def failed():
            ids = {request["id"] for request in terminal.messages("client")
                   if request.get("params", {}).get("entry") == entry}
            return any(message.get("id") in ids and message.get("error", {}).get("message") == reason
                       for message in terminal.messages("server"))
        terminal.wait(failed, "injected source failure belongs to the requested entry", timeout=20)
        terminal.wait(lambda: reason in terminal.text or "Source failed" in terminal.text,
                      "source failure is visible")

    def check_read_failure(self, nth):
        original = body("FAILED", 512 * 1024)
        item = {"type":"agentMessage", "id":"failed-source", "phase":"final_answer", "text":original}
        fixture, client, session = self.running_sources("direct-failure", [item])
        entry = self.entries(client, session)[0]["id"]
        terminal = self.attach_daemon(fixture, session, "direct", source_read_error_after=nth)
        self.injected(terminal, DIRECT_FAILURE, entry)
        self.assertEqual(len(self.reads(terminal, entry)), nth)
        timeline = [{"after":"failure", "reads":nth}]
        for index in range(3):
            delta = f"\nnumbered append-{index}\n"
            self.source_append(fixture, "failed-source", delta)
            expected = len(original.encode()) + len(delta.encode()) * (index + 1)
            terminal.wait(lambda: any(message.get("params", {}).get("event", {}).get("source", {}).get("bytes") == str(expected)
                          for message in terminal.messages("server")), "new numbered source length delivered")
            draft = f"private failed draft-{index}"
            terminal.replace_input(draft)
            terminal.wait(lambda: terminal.input_is(draft), "composer accepts typing after failed source append")
            self.assertEqual(len(self.reads(terminal, entry)), nth, "failed source stays paused before F6")
            timeline.append({"after":index, "reads":len(self.reads(terminal, entry))})
        terminal.send(b"\x1b[17~")
        terminal.contains("numbered append-2")
        self.assertGreater(len(self.reads(terminal, entry)), nth)
        self.assertTrue(terminal.input_is("private failed draft-2"))
        self.assert_private(terminal)
        timeline.append({"after":"F6", "reads":len(self.reads(terminal, entry))})
        (terminal.root / "read-timeline.json").write_text(json.dumps(timeline, indent=2) + "\n")
        terminal.snapshot("source-resumes-only-after-F6")

    def priority_session(self):
        answer = body("A", 8192)
        items = [{"type":"reasoning", "id":"inline-R", "summary":[{"text":"R inline"}]},
                 {"type":"agentMessage", "id":"answer-A", "phase":"final_answer", "text":answer},
                 {"type":"commandExecution", "id":"tool-C", "command":"printf C", "status":"completed",
                  "exitCode":0, "aggregatedOutput":body("C", 512 * 1024)}]
        fixture, client, session = self.daemon_session("copy-priority", lambda:history(items))
        terminal = self.attach_daemon(fixture, session, "priority", extra_env={
            "SSH_TTY":"", "SSH_CONNECTION":"", "SSH_CLIENT":""})
        terminal.contains("C TAIL")
        rows = self.entries(client, session)
        answer_id = next(row["id"] for row in rows if row.get("kind") == "answer")
        tool_id = next(row["id"] for row in rows if row.get("tool"))
        return fixture, client, session, terminal, answer, answer_id, tool_id

    @contextmanager
    def blocked_original(self, terminal, entries):
        root = terminal.root
        program = root / "bin/wl-copy"
        program.write_text("""#!/usr/bin/env python3
import fcntl,os,sys
from pathlib import Path
root=Path(os.environ['CARLOS_TEST_ROOT'])
number=len(list(root.glob('copy-started-*')))+1
(root/f'copy-started-{number}').write_text('started')
if number == 1:
    with (root/'copy-release').open() as barrier:
        fcntl.flock(barrier, fcntl.LOCK_SH)
target=root/f'copy-{number}.tmp'
target.write_bytes(sys.stdin.buffer.read())
target.replace(root/f'copy-{number}.bin')
""")
        program.chmod(0o755)
        barrier = (root / "copy-release").open("w")
        fcntl.flock(barrier, fcntl.LOCK_EX)
        try:
            terminal.send(b"\x19")
            terminal.wait((root / "copy-started-1").exists, "first original-copy child blocks before stdin")
            terminal.wait(lambda: self.watches_accepted(terminal, entries), "A and C tail watches accepted")
            yield
        finally:
            fcntl.flock(barrier, fcntl.LOCK_UN)
            barrier.close()

    @staticmethod
    def watches_accepted(terminal, entries):
        replies = {message.get("id") for message in terminal.messages("server") if "result" in message}
        watched = {request["params"]["entry"] for request in terminal.requests("entry/source/subscribe")
                   if request["id"] in replies}
        return set(entries) <= watched

    def publish(self, fixture, identity, text):
        self.source_append(fixture, identity, text)

    @staticmethod
    def inline_ack(fixture, text):
        async def append():
            for link in list(fixture.native.connections):
                await fixture.native.notify(link, "item/completed", turnId="history-turn", item={
                    "type":"reasoning", "id":"inline-R", "summary":[{"text":"R inline" + text}]})
        asyncio.run_coroutine_threadsafe(append(), fixture.native.loop).result(timeout=5)

    def old_copy_finished(self, terminal, answer):
        target = terminal.root / "copy-1.bin"
        terminal.wait(target.exists, "first blocked answer copy finishes after release")
        self.assertEqual(target.read_bytes(), answer.encode())

    @staticmethod
    def frame_finished(terminal):
        cursor = terminal.screen.cursor
        return not cursor.hidden and cursor.y == terminal.screen.lines - 1 and cursor.x == terminal.content_x

    def healthy_c_head(self, terminal):
        for _ in range(25):
            if "C HEAD" in terminal.text or "C row-" in terminal.text:
                break
            previous = terminal.text
            terminal.send(b"\x1b[6~")
            terminal.wait(lambda: self.frame_finished(terminal) and terminal.text != previous,
                          "PageDown finishes another source view")
        for _ in range(25):
            if "C HEAD" in terminal.text:
                break
            previous = terminal.text
            terminal.send(b"\x1b[<64;2;2M")
            terminal.wait(lambda: self.frame_finished(terminal) and terminal.text != previous,
                          "wheel back to the beginning of healthy C")
        terminal.contains("C HEAD")

    def assert_private(self, terminal):
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertEqual(terminal.requests("session/interrupt"), [])
        self.assertFalse((terminal.root / "copy-started-2").exists(), "failed copy must not start another clipboard child")

    def test_initial_direct_failure_stays_paused_until_explicit_retry(self):
        self.check_read_failure(1)

    def test_mid_prefix_direct_failure_stays_paused_until_explicit_retry(self):
        self.check_read_failure(4)

    def test_failed_original_copy_releases_priority_for_another_source(self):
        fixture, client, session, terminal, answer, answer_id, tool_id = self.priority_session()
        with self.blocked_original(terminal, [answer_id, tool_id]):
            terminal.send(b"\x1b[H")
            terminal.contains("R inline")
            terminal.daemon_recorder.source_error_once = True
            self.publish(fixture, "answer-B", body("B", 24 * 1024))
            daemon_support.wait_for(lambda: len(self.entries(client, session)) == 4, "new offscreen answer B published")
            failed_id = self.entries(client, session)[-1]["id"]
            self.inline_ack(fixture, " B arrived")
            terminal.contains("R inline B arrived")
            terminal.send(b"\x19")
            self.injected(terminal, WATCH_FAILURE, failed_id)
        self.old_copy_finished(terminal, answer)
        self.healthy_c_head(terminal)
        terminal.paste("private recovered copy draft")
        terminal.wait(lambda: terminal.input_is("private recovered copy draft"), "private draft after copy cancellation")
        self.assert_private(terminal)
        terminal.snapshot("failed-original-copy-releases-C")

    def select_c_tail_to_r(self, terminal, reject_read=False):
        row = next(index for index, line in enumerate(terminal.screen.display) if "C TAIL" in line)
        col = wcswidth(terminal.screen.display[row].split("C TAIL")[0]) + 1
        terminal.send(f"\x1b[<0;{col};{row+1}M\x1b[<32;{col+3};{row+1}M".encode())
        terminal.wait(lambda: terminal.screen.buffer[row][col-1].bg == "6c7086", "force Select before Home")
        terminal.send(b"\x1b[H")
        terminal.contains("R inline")
        if reject_read:
            terminal.daemon_recorder.source_read_error_after = 1
        row = next(index for index, line in enumerate(terminal.screen.display) if "R inline" in line)
        col = terminal.screen.display[row].index("R inline") + 1
        terminal.send(f"\x1b[<32;{col};{row+1}M\x1b[<0;{col};{row+1}m".encode())

    def healthy_d(self, fixture, terminal):
        self.publish(fixture, "healthy-D", body("D", 512 * 1024))
        terminal.send(b"\x1b[F")
        terminal.contains("D TAIL")
        terminal.paste("private recovered selection draft")
        terminal.wait(lambda: terminal.input_is("private recovered selection draft"), "private draft after selection cancellation")
        self.assert_private(terminal)

    def test_failed_selection_releases_priority_for_a_new_source(self):
        fixture, _client, _session, terminal, answer, answer_id, tool_id = self.priority_session()
        with self.blocked_original(terminal, [answer_id, tool_id]):
            self.select_c_tail_to_r(terminal, reject_read=True)
        self.old_copy_finished(terminal, answer)
        self.injected(terminal, DIRECT_FAILURE, tool_id)
        self.healthy_d(fixture, terminal)
        terminal.snapshot("failed-selection-releases-D")

    def test_shorter_source_revision_cancels_a_later_selection_stage(self):
        fixture, client, session, terminal, answer, answer_id, tool_id = self.priority_session()
        revision = next(row["tool"]["output_revision"] for row in self.entries(client, session) if row["id"] == tool_id)
        with self.blocked_original(terminal, [answer_id, tool_id]):
            self.select_c_tail_to_r(terminal)

            async def replace():
                for link in list(fixture.native.connections):
                    await fixture.native.notify(link, "item/completed", turnId="history-turn", item={
                        "type":"commandExecution", "id":"tool-C", "command":"printf C", "status":"completed",
                        "exitCode":0, "aggregatedOutput":body("C SHORT", 4096)})
            asyncio.run_coroutine_threadsafe(replace(), fixture.native.loop).result(timeout=5)
            daemon_support.wait_for(lambda: any(row["id"] == tool_id and row["tool"]["output_revision"] != revision
                                    for row in self.entries(client, session)), "authoritative C replacement has a new revision")
            self.inline_ack(fixture, " C replaced")
            terminal.contains("R inline C replaced")
        self.old_copy_finished(terminal, answer)
        self.healthy_d(fixture, terminal)
        terminal.snapshot("shorter-C-revision-cancels-selection-and-releases-D")


def identity(path):
    return {"path": str(path.resolve()), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--daemon", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path)
    parser.add_argument("case", choices=("both", "reads", "original", "selection", "revision"), nargs="?", default="both")
    options = parser.parse_args()
    ARTIFACTS = (options.artifacts or Path(tempfile.mkdtemp(prefix="carlos-source-fail-ui-"))).resolve()
    ARTIFACTS.mkdir(parents=True, exist_ok=True)
    pair = ARTIFACTS / "executables"
    pair.mkdir()
    BINARY = pair / "carlos"
    BINARY.symlink_to(options.binary.resolve())
    (pair / "carlosd").symlink_to(options.daemon.resolve())
    (ARTIFACTS / "identities.json").write_text(json.dumps({
        "terminal": identity(BINARY), "daemon": identity(options.daemon),
        "test": identity(Path(__file__))}, indent=2) + "\n")
    names = ("test_initial_direct_failure_stays_paused_until_explicit_retry",
             "test_mid_prefix_direct_failure_stays_paused_until_explicit_retry",
             "test_failed_original_copy_releases_priority_for_another_source",
             "test_failed_selection_releases_priority_for_a_new_source",
             "test_shorter_source_revision_cancels_a_later_selection_stage")
    suite = unittest.TestSuite(DaemonSourceFailures(name) for name in names if options.case == "both"
                              or options.case in name or options.case == "reads" and "direct_failure" in name)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    raise SystemExit(not result.wasSuccessful())
