"""Replay captured large originals through real daemon sockets and a real terminal.

Standalone: pass --binary, --daemon and --capture-root (the probe capture directory).
The runner changes no shared test registration and never sends a large native frame.
"""

import argparse
import asyncio
import fcntl
import hashlib
import json
from pathlib import Path
import socket
import tempfile
import time
import unittest

from terminal import Terminal
from test_daemon_client import DaemonClient, daemon_support


class DaemonLargeSource(unittest.TestCase):
    daemon_session = DaemonClient.daemon_session
    attach_daemon = DaemonClient.attach_daemon

    def summary_root(self):
        root = ARTIFACTS / self._testMethodName
        root.mkdir(parents=True, exist_ok=True)
        return root

    def launch(self, directory, width, resume, extra_args, extra_env=None):
        terminal = Terminal(BINARY, directory, resume=resume, width=width,
                            extra_args=extra_args, extra_env=extra_env)
        self.addCleanup(terminal.close)
        return terminal

    def captured(self, kind):
        path = CAPTURE_ROOT / kind / "clipboard-120.txt"
        original = path.read_bytes()
        self.assertGreater(len(original), 8 * 1024 * 1024)
        text = original.decode("utf-8")
        manifest = json.loads((CAPTURE_ROOT / "manifest.json").read_text())
        saved = next(case for case in manifest["cases"] if case["name"] == kind)
        self.assertEqual(len(original), saved["bytes"])
        self.assertEqual(hashlib.sha256(original).hexdigest(), saved["sha256"])
        identity = {"path": str(path.resolve()), "bytes": len(original), "sha256": saved["sha256"]}
        (self.summary_root() / "original-capture.json").write_text(json.dumps(identity, indent=2) + "\n")
        return text, identity

    @staticmethod
    def pieces(text):
        # Split on UTF-8 boundaries; ANSI controls may span pieces, as on the wire.
        data = text.encode()
        start = 0
        while start < len(data):
            end = min(start + 65536, len(data))
            while end < len(data) and data[end] & 0xc0 == 0x80:
                end -= 1
            yield data[start:end].decode()
            start = end

    def replay_session(self, kind):
        text, identity = self.captured(kind)
        empty = lambda: {"id":"native-root", "status":{"type":"idle"},
                         "model":"fixture-model", "reasoningEffort":"low", "turns":[]}
        fixture, client, session = self.daemon_session("large-" + kind, empty)
        first, *remaining = self.pieces(text)

        async def start(link, request):
            await fixture.native.notify(link, "turn/started", turn={"id":"large-turn"})
            if kind == "ansi":
                await fixture.native.notify(link, "item/started", turnId="large-turn", item={
                    "type":"commandExecution", "id":"large-source", "command":"printf probe",
                    "status":"inProgress"})
            await self.emit(fixture, link, kind, first)
        fixture.native.prompt = start
        operation = client.operation(2)
        client.call("session/send", session=session, operation=operation, text="replay captured " + kind)
        client.outcome(operation)
        daemon_support.wait_for(lambda: self.source_bytes(client, session) >= len(first.encode()),
                                "initial captured prefix published")
        return fixture, client, session, text, remaining, identity

    @staticmethod
    async def emit(fixture, link, kind, piece):
        assert len(piece.encode()) <= 65536
        method = "item/commandExecution/outputDelta" if kind == "ansi" else "item/agentMessage/delta"
        await fixture.native.notify(link, method, turnId="large-turn", itemId="large-source", delta=piece)

    @staticmethod
    def source_bytes(client, session):
        entries = client.call("session/subscribe", session=session, since=None)["snapshot"]["entries"]
        sizes = [int(entry.get("source", {}).get("bytes", 0)) for entry in entries]
        for entry in entries:
            if (entry.get("tool") or {}).get("has_output"):
                chunk = client.call("entry/source", session=session, entry=entry["id"], cursor=None, limit=64)
                sizes.append(int(chunk["source"]["bytes"]))
        return max(sizes, default=0)

    def grow(self, fixture, kind, pieces):
        async def append():
            links = list(fixture.native.connections)
            self.assertEqual(len(links), 1)
            for piece in pieces:
                await self.emit(fixture, links[0], kind, piece)
        asyncio.run_coroutine_threadsafe(append(), fixture.native.loop).result(timeout=40)

    def ready(self, terminal, marker):
        failures = ("exceeds the body cache", "output queue filled", "Cannot format accepted source")
        terminal.wait(lambda: (marker in terminal.text and self.frame_finished(terminal))
                      or any(error in terminal.text for error in failures),
                      "captured viewport or explicit source failure", timeout=40)
        self.assertIn(marker, terminal.text, "the terminal must paint the large captured original")
        self.assertNotIn("exceeds the body cache", terminal.text)

    @staticmethod
    def frame_finished(terminal):
        cursor = terminal.screen.cursor
        return not cursor.hidden and cursor.y == terminal.screen.lines - 1

    def check_viewports(self, terminal, early, late, draft):
        terminal.send(b"\x1b[5~")
        terminal.wait(lambda: late not in terminal.text and terminal.input_is(draft),
                      "PageUp scrolls while keeping the private draft")
        terminal.send(b"\x1b[6~")
        self.ready(terminal, late)
        terminal.resize(28, 32)
        terminal.wait(lambda: terminal.input_is(draft), "phone resize keeps the private draft")
        # Home/End navigate the transcript only with an empty composer.
        terminal.replace_input("")
        terminal.wait(lambda: terminal.input_is(""), "clear draft explicitly before Home/End")
        for width in (120, 28):
            terminal.resize(width, 32)
            terminal.send(b"\x1b[F")
            self.ready(terminal, late)
            terminal.snapshot(f"late-{width}")
            terminal.send(b"\x1b[H")
            terminal.wait(lambda: early in terminal.text and late not in terminal.text
                          and self.frame_finished(terminal),
                          "Home paints the original early viewport", timeout=40)
            terminal.snapshot(f"early-{width}")
        terminal.send(b"\x1b[F")
        self.ready(terminal, late)
        terminal.paste(draft)
        terminal.wait(lambda: terminal.input_is(draft), "restore private draft after viewport checks")

    def restored_source(self, terminal, offset, expected_entry, expected_bytes):
        state = {"offset": offset, "request": None, "chunk": None}

        def received():
            with (terminal.root / "rpc.jsonl").open("rb") as log:
                log.seek(state["offset"])
                for line in log:
                    if not line.endswith(b"\n"):
                        break
                    state["offset"] += len(line)
                    record = json.loads(line)
                    message = record["message"]
                    if record["direction"] == "client" and message.get("method") == "entry/source/subscribe":
                        self.assertEqual(message["params"]["entry"], expected_entry)
                        state["request"] = message["id"]
                    elif record["direction"] == "server" and state["request"] is not None and message.get("id") == state["request"]:
                        self.assertIn("result", message, "replacement source subscription must succeed")
                        state["chunk"] = message["result"]
                        return True
            return False

        terminal.wait(received, "successful replacement source subscription", timeout=40)
        self.assertEqual(state["chunk"]["entry"], expected_entry)
        self.assertEqual(int(state["chunk"]["source"]["bytes"]), expected_bytes)

    def check_reconnect(self, fixture, terminal, early, late, draft, expected_bytes):
        # Save a byte boundary so an old reply cannot satisfy the replacement read.
        log = terminal.root / "rpc.jsonl"
        expected_entry = None
        offset = 0
        with log.open("rb") as records:
            for line in records:
                if not line.endswith(b"\n"):
                    break
                offset += len(line)
                if line.startswith(b'{"direction": "client",'):
                    message = json.loads(line)["message"]
                    if message.get("method") == "entry/source/subscribe":
                        expected_entry = message["params"]["entry"]
        self.assertIsNotNone(expected_entry)
        for peer in list(terminal.daemon_recorder.peers):
            try:
                peer.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
        terminal.contains("F5 reconnects")
        self.assertIsNone(fixture.process.poll())
        terminal.send(b"\x1b[15~")
        self.restored_source(terminal, offset, expected_entry, expected_bytes)
        terminal.wait(lambda: "F5 reconnects" not in terminal.text, "offline notice clears")
        terminal.wait(lambda: terminal.input_is(draft), "private draft survives source reconnect")
        terminal.replace_input("")
        terminal.wait(lambda: terminal.input_is(""), "clear restored draft before Home/End")
        terminal.send(b"\x1b[H")
        terminal.wait(lambda: early in terminal.text and late not in terminal.text,
                      "fresh early viewport after source reconnect", timeout=40)
        terminal.send(b"\x1b[F")
        self.ready(terminal, late)
        terminal.paste(draft)
        terminal.wait(lambda: terminal.input_is(draft), "restore reconnected private draft")
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertEqual(terminal.requests("session/interrupt"), [])
        starts = [request for request in fixture.native.requests if request.get("method") == "turn/start"]
        self.assertEqual(len(starts), 1, "reconnect must not replay the native input")
        terminal.snapshot("reconnected-large-original")

    def run_capture(self, kind, early, late):
        fixture, client, session, text, remaining, identity = self.replay_session(kind)
        # Publish the capture before attaching, so replay cannot starve the PTY reader.
        self.grow(fixture, kind, remaining)
        daemon_support.wait_for(lambda: self.source_bytes(client, session) == len(text.encode()),
                                "complete unfinished capture published", timeout=40)
        terminal = self.attach_daemon(fixture, session, kind, width=120, extra_env={
            "SSH_TTY":"", "SSH_CONNECTION":"", "SSH_CLIENT":""})
        self.ready(terminal, late)
        # Auto folds tools at phone width; pin Full only in this fixture's private home.
        if kind == "ansi":
            self.assert_text_color(terminal, late, "cd0000")
            terminal.paste("/tools full")
            terminal.send(b"\r")
            settings_path = terminal.root / "config/carlos/ui-settings.json"
            terminal.wait(lambda: terminal.input_is("") and settings_path.exists()
                          and json.loads(settings_path.read_text()).get("tool_output") == "full",
                          "local full-output preference accepted")
            settings = json.loads((terminal.root / "config/carlos/ui-settings.json").read_text())
            self.assertEqual(settings["tool_output"], "full")
        terminal.daemon_recorder.pause_sources()
        suffix = "\nappended captured tail\n"
        self.grow(fixture, kind, [suffix])
        text += suffix
        draft = "draft while pending"
        try:
            terminal.wait(lambda: bool(terminal.daemon_recorder.held_sources),
                          "one bounded append waits behind source backpressure")
            started = time.monotonic()
            terminal.paste(draft)
            terminal.wait(lambda: terminal.input_is(draft), "typing while source is pending", timeout=2)
            identity["pending_typing_ms"] = (time.monotonic() - started) * 1000
            terminal.snapshot("typing-during-source-backpressure")
        finally:
            terminal.daemon_recorder.resume_sources()
        self.ready(terminal, "appended captured tail")
        self.check_viewports(terminal, early, late, draft)
        terminal.wait(lambda: terminal.input_is(draft), "draft survives large-source resizes")
        self.check_reconnect(fixture, terminal, early, late, draft, len(text.encode()))
        self.assert_wire_bounds(terminal, fixture, identity)
        identity.update(final_bytes=len(text.encode()), final_sha256=hashlib.sha256(text.encode()).hexdigest())
        (terminal.root / "capture-identity.json").write_text(json.dumps(identity, indent=2) + "\n")
        return terminal, text

    def assert_wire_bounds(self, terminal, fixture, identity):
        chunks = []
        for message in terminal.messages("server"):
            result = message.get("result", {})
            chunk = result if isinstance(result, dict) and "source" in result else message.get("params", {}).get("chunk", {})
            if "source" in chunk:
                chunks.append(chunk)
        self.assertTrue(chunks)
        self.assertTrue(all(len(chunk.get("text", "").encode()) <= 65536 for chunk in chunks))
        self.assertTrue(any(int(chunk["source"]["bytes"]) > 8*1024*1024 for chunk in chunks))
        self.assertTrue(any(not chunk.get("done", True) for chunk in chunks), "source must remain unfinished")
        native = [json.loads(line)["message"] for line in (fixture.root / "codex.jsonl").read_text().splitlines()]
        maximum = max(len(json.dumps(message).encode()) for message in native)
        self.assertLess(maximum, 8*1024*1024, "native frames stay within the existing bound")
        identity.update(source_chunks=len(chunks), maximum_native_frame_bytes=maximum)

    def assert_text_color(self, terminal, text, expected):
        row = next(index for index, line in enumerate(terminal.screen.display) if text in line)
        column = terminal.screen.display[row].index(text)
        self.assertEqual(terminal.screen.buffer[row][column].fg, expected)

    def test_captured_ansi_tool_keeps_early_late_rows_and_private_input(self):
        terminal, _ = self.run_capture("ansi", "red sample", "tail stays red")
        # The old wrapper uses the tool default on this final soft-wrapped row.
        self.assert_text_color(terminal, "tail stays red", "9399b2")
        self.assert_text_color(terminal, "appended captured tail", "cd0000")

    def test_captured_markdown_answer_copies_exact_original_after_resize_and_reconnect(self):
        # The existing shortcut copies the last assistant answer, so use its actual format.
        terminal, text = self.run_capture("markdown", "literal 🌍", "Markdown tail")
        root = terminal.root
        program = root / "bin/wl-copy"
        program.write_text("""#!/usr/bin/env python3
import fcntl,os,sys
from pathlib import Path
root=Path(os.environ['CARLOS_TEST_ROOT'])
(root/'copy-started').write_text('started')
with (root/'copy-release').open() as barrier:
    fcntl.flock(barrier, fcntl.LOCK_SH)
target=root/'copy.tmp'
target.write_bytes(sys.stdin.buffer.read())
target.replace(root/'copied.txt')
""")
        program.chmod(0o755)
        barrier = (root / "copy-release").open("w")
        fcntl.flock(barrier, fcntl.LOCK_EX)
        target = root / "copied.txt"
        try:
            terminal.send(b"\x19")
            terminal.wait((root / "copy-started").exists, "clipboard child waits before reading stdin", timeout=40)
            started = time.monotonic()
            terminal.replace_input("draft during copy")
            terminal.wait(lambda: terminal.input_is("draft during copy"),
                          "composer stays live while clipboard pipe is blocked", timeout=2)
            typing_ms = (time.monotonic() - started) * 1000
            self.assertFalse(target.exists(), "the child cannot finish copying before release")
            self.assertEqual(terminal.requests("session/send"), [])
            (root / "clipboard-pending.json").write_text(json.dumps({
                "typing_ms": typing_ms}, indent=2) + "\n")
            terminal.snapshot("typing-during-blocked-clipboard")
        finally:
            fcntl.flock(barrier, fcntl.LOCK_UN)
            barrier.close()
        terminal.wait(target.exists, "complete captured original copied", timeout=40)
        copied = target.read_bytes()
        self.assertEqual(len(copied), len(text.encode()))
        self.assertEqual(hashlib.sha256(copied).hexdigest(), hashlib.sha256(text.encode()).hexdigest())
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.wait(lambda: terminal.input_is("draft during copy"), "private copy draft survives completion")
        terminal.snapshot("copied-complete-captured-markdown")


def identity(path):
    return {"path": str(path.resolve()), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--daemon", type=Path, required=True)
    parser.add_argument("--capture-root", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path)
    parser.add_argument("case", choices=("both", "ansi", "markdown"), nargs="?", default="both")
    options = parser.parse_args()
    ARTIFACTS = (options.artifacts or Path(tempfile.mkdtemp(prefix="carlos-large-ui-"))).resolve()
    ARTIFACTS.mkdir(parents=True, exist_ok=True)
    CAPTURE_ROOT = options.capture_root.resolve()
    pair = ARTIFACTS / "executables"
    pair.mkdir()
    BINARY = pair / "carlos"
    BINARY.symlink_to(options.binary.resolve())
    (pair / "carlosd").symlink_to(options.daemon.resolve())
    (ARTIFACTS / "executables.json").write_text(json.dumps({
        "terminal": identity(options.binary), "daemon": identity(options.daemon),
        "test": identity(Path(__file__))}, indent=2) + "\n")
    # Keep this dedicated runner bounded to its own two workflows.
    suite = unittest.TestSuite(DaemonLargeSource(name) for name in (
        "test_captured_ansi_tool_keeps_early_late_rows_and_private_input",
        "test_captured_markdown_answer_copies_exact_original_after_resize_and_reconnect")
        if options.case == "both" or options.case in name)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    raise SystemExit(not result.wasSuccessful())
