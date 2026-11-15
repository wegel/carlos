"""Drive Carlos through a Unix terminal and retain screens and wire traffic."""

import codecs
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import sys
import termios
import time

import pyte

from terminal_screen import TerminalScreen


class Terminal:
    daemon_binary = None

    def __init__(self, binary, root, mode="modern", resume="thread-original", width=100, backend="codex",
                 codex=None, claude=None, extra_env=None, extra_args=None):
        self.root = root
        self.owned_daemon = None
        self.root.mkdir(parents=True, exist_ok=True)
        shim = root / "bin"
        shim.mkdir(exist_ok=True)
        peer = "native_codex_peer.py" if codex else "fake_codex.py"
        if not (shim / "codex").exists():
            (shim / "codex").symlink_to(Path(__file__).with_name(peer).resolve())
        if not (shim / "claude").exists():
            (shim / "claude").symlink_to(claude or Path(__file__).with_name("fake_claude.py").resolve())
        if backend == "grok" and not (shim / "grok").exists():
            (shim / "grok").symlink_to(Path(__file__).with_name("fake_grok.py").resolve())
        self.screen = TerminalScreen(width, 32)
        self.stream = pyte.Stream(self.screen)
        self.decoder = codecs.getincrementaldecoder("utf-8")("replace")
        self.output = (root / "terminal.ansi").open("ab")
        inherited = os.environ if not claude else {key:value for key,value in os.environ.items() if not key.startswith(("ANTHROPIC_", "CLAUDE_", "CARLOS_CLAUDE_"))}
        env = {**inherited, "TERM": "xterm-256color", "CARLOS_TEST_ROOT": str(root),
               "CARLOS_TEST_SERVER": mode, "HOME": str(root),
               "CODEX_HOME": str(root / "codex-home"), "GROK_HOME": str(root / "grok-home"), "CARLOS_METRICS": "0",
               "XDG_CONFIG_HOME": str(root / "config"),
               "XDG_STATE_HOME": str(root / "state"),
               "CARLOS_TOOL_SUMMARIES": "on" if mode.startswith("summary") else "off",
               "CLAUDE_CONFIG_DIR": str(root / "claude-home"),
               "CARLOS_EVENT_TRACE": str(root / "keys.log"),
               "CARLOS_CLAUDE_WIRE_TRACE": str(root / "claude-controller.jsonl"),
               "PATH": str(shim) + os.pathsep + os.environ["PATH"]}
        if extra_env:
            env.update(extra_env)
        env.pop("NO_COLOR", None)
        env["COLORTERM"] = "truecolor"
        if codex:
            env["CARLOS_TEST_CODEX"] = str(codex)
        if mode in {"claude-reply-error", "claude-approval-switch"}:
            fixture = "flaky_claude_controller.py" if mode == "claude-reply-error" else "approval_switch_controller.py"
            env["CARLOS_CLAUDE_CONTROLLER"] = str(Path(__file__).with_name(fixture).resolve())
            env["CARLOS_REAL_CLAUDE_CONTROLLER"] = str(binary.parent.parent / "claude-controller/carlos-claude-controller")
        arguments = list(extra_args or [])
        args = [str(binary), "--backend", backend, *arguments]
        if resume == "__continue__":
            args += ["continue"]
        elif resume == "__picker__":
            args += ["resume"]
        elif resume:
            args += ["resume", resume]
        try:
            if self.daemon_binary and "--daemon-socket" not in arguments:
                from terminal_daemon import OwnedDaemon
                codex_socket = arguments[arguments.index("--codex-socket") + 1] if "--codex-socket" in arguments else None
                self.owned_daemon = OwnedDaemon(self.daemon_binary, root, env, codex_socket)
                args[1:1] = ["--daemon-socket", str(self.owned_daemon.endpoint)]
            self.open_process(args, env, width)
        except BaseException:
            self.output.close()
            if self.owned_daemon:
                self.owned_daemon.close()
            raise

    def open_process(self, args, env, width):
        self.fd, slave = pty.openpty()
        try:
            self.process = subprocess.Popen(
                [sys.executable, str(Path(__file__).with_name("terminal_child.py")), *args],
                stdin=slave, stdout=slave, stderr=slave, cwd=self.root, env=env, start_new_session=True)
        except BaseException:
            os.close(self.fd)
            raise
        finally:
            os.close(slave)
        self.pid = self.process.pid
        self.resize(width, 32)

    def resize(self, width, height):
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))
        self.screen.resize(lines=height, columns=width)
        os.kill(self.pid, signal.SIGWINCH)

    def pump(self, timeout=0.05):
        if select.select([self.fd], [], [], timeout)[0]:
            try:
                data = os.read(self.fd, 65536)
            except OSError:
                data = b""
            if data:
                self.output.write(data)
                self.output.flush()
                self.stream.feed(self.decoder.decode(data))

    def wait(self, predicate, description, timeout=8):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            self.pump()
            if predicate():
                return
        self.snapshot("failure")
        raise AssertionError(f"Timed out: {description}\n{self.text}\nArtifacts: {self.root}")

    @property
    def text(self):
        return "\n".join(self.screen.display)

    def contains(self, text):
        self.wait(lambda: text in self.text, "screen contains " + repr(text))

    @property
    def phone_layout(self):
        path = self.root / "config/carlos/ui-settings.json"
        settings = json.loads(path.read_text()) if path.exists() else {}
        profile = "phone" if self.screen.columns < 80 or self.screen.lines < 20 else "desktop"
        legacy = settings.get("layout", "auto")
        default = legacy if legacy != "auto" else profile
        theme = settings.get("themes", {}).get(profile, default)
        return theme in ("phone", "flow")

    @property
    def content_x(self):
        return 1 if self.phone_layout else 2

    def input_is(self, text):
        line = self.screen.display[-1].strip()
        if not text and self.phone_layout and '━' not in self.screen.display[-2]:
            return False  # A blank screen before the first frame is not an empty composer.
        if text:
            return line == ("" if self.phone_layout else "❯ ") + text
        return line == ("" if self.phone_layout else "❯") or self.placeholder_is()

    def wait_ready_composer(self):
        self.wait(lambda: self.input_is(""), "empty composer before a new prompt")
        if self.owned_daemon or hasattr(self, "daemon_recorder"):
            self.wait(self.daemon_is_idle, "accepted ready snapshot before a new prompt")

    def wait_settings_editor(self):
        self.contains("Settings")
        if self.owned_daemon or hasattr(self, "daemon_recorder"):
            self.contains("Tab field, arrows, Enter save")

    def placeholder_is(self, name=None):
        line = self.screen.display[-1]
        cells = [self.screen.buffer[self.screen.lines - 1][x]
                 for x in range(self.content_x, self.screen.columns)
                 if self.screen.buffer[self.screen.lines - 1][x].data.strip()]
        return ((line.startswith(" ") if self.phone_layout else line.startswith("❯")) and bool(cells)
                and all(cell.fg == "6c7086" for cell in cells)
                and (name is None or line[1:].strip() == name))

    def send(self, data):
        os.write(self.fd, data)

    def paste(self, text):
        self.send(b"\x1b[200~" + text.encode() + b"\x1b[201~")

    def rewind(self, expected):
        if self.owned_daemon or hasattr(self, "daemon_recorder"):
            self.wait(self.daemon_is_idle, "current daemon facts finish the turn before idle rewind")
        self.send(b"\x1b")
        for _ in range(3):
            self.pump(0.05)
        self.send(b"\x1b")
        self.wait(lambda: self.input_is(expected),
                  "rewind composer contains " + repr(expected))

    def daemon_is_idle(self):
        for message in reversed(self.daemon_messages("server")):
            result = message.get("result")
            result = result if isinstance(result, dict) else {}
            params = message.get("params", {})
            event = params.get("event", {})
            info = (result.get("snapshot", {}).get("info") or params.get("snapshot", {}).get("info")
                    or event.get("info") or event.get("snapshot", {}).get("info"))
            if info:
                activity = (info.get("status") or {}).get("activity") or {}
                return (info["availability"] == "ready" and info["phase"] == "idle"
                        and info.get("current_turn") is None and activity.get("state") != "running")
        return False

    def replace_input(self, text):
        self.send(b"\x01\x0b")  # Ctrl+A, Ctrl+K: start of line, delete to end.
        self.paste(text)

    def snapshot(self, name):
        (self.root / (name + ".txt")).write_text(self.text)

    def messages(self, direction="client"):
        log = self.root / "rpc.jsonl"
        if not log.exists():
            return []
        return [record["message"] for line in log.read_bytes().splitlines(keepends=True) if line.endswith(b"\n")
                if (record := json.loads(line))["direction"] == direction]

    def requests(self, method):
        return [message for message in self.messages() if message.get("method") == method]

    def daemon_messages(self, direction="client"):
        if self.owned_daemon:
            return self.owned_daemon.messages(direction)
        return self.messages(direction) if hasattr(self, "daemon_recorder") else []

    def daemon_requests(self, method):
        return [message for message in self.daemon_messages() if message.get("method") == method]

    def close(self):
        try:
            if not self.output.closed:
                try:
                    self.snapshot("last-screen")
                finally:
                    self.close_process()
        finally:
            if self.owned_daemon:
                self.owned_daemon.close()

    def close_process(self):
        try:
            self.send(b"\x03")
        except OSError:
            pass
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            self.pump()
            if self.process.poll() is not None:
                break
        else:
            self.process.kill()
            self.process.wait()
        os.close(self.fd)
        self.output.close()
        assert self.process.returncode == 0, f"Carlos exited abnormally; see {self.root}"
