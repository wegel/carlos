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


class Terminal:
    def __init__(self, binary, root, mode="modern", resume="thread-original", width=100, backend="codex",
                 codex=None):
        self.root = root
        self.root.mkdir(parents=True, exist_ok=True)
        shim = root / "bin"
        shim.mkdir(exist_ok=True)
        peer = "native_codex_peer.py" if codex else "fake_codex.py"
        if not (shim / "codex").exists():
            (shim / "codex").symlink_to(Path(__file__).with_name(peer).resolve())
        if not (shim / "claude").exists():
            (shim / "claude").symlink_to(Path(__file__).with_name("fake_claude.py").resolve())
        self.screen = pyte.Screen(width, 32)
        self.stream = pyte.Stream(self.screen)
        self.decoder = codecs.getincrementaldecoder("utf-8")("replace")
        self.output = (root / "terminal.ansi").open("ab")
        env = {**os.environ, "TERM": "xterm-256color", "CARLOS_TEST_ROOT": str(root),
               "CARLOS_TEST_SERVER": mode, "HOME": str(root),
               "CODEX_HOME": str(root / "codex-home"), "CARLOS_METRICS": "0",
               "XDG_CONFIG_HOME": str(root / "config"),
               "CLAUDE_CONFIG_DIR": str(root / "claude-home"),
               "CARLOS_EVENT_TRACE": str(root / "keys.log"),
               "PATH": str(shim) + os.pathsep + os.environ["PATH"]}
        env.pop("NO_COLOR", None)
        env["COLORTERM"] = "truecolor"
        if codex:
            env["CARLOS_TEST_CODEX"] = str(codex)
        args = [str(binary), "--backend", backend]
        if resume:
            args += ["resume", resume]
        self.fd, slave = pty.openpty()
        try:
            self.process = subprocess.Popen(
                [sys.executable, str(Path(__file__).with_name("terminal_child.py")), *args],
                stdin=slave, stdout=slave, stderr=slave, cwd=root, env=env, start_new_session=True)
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

    def input_is(self, text):
        line = self.screen.display[-1].strip()
        if text:
            return line == "> " + text
        return line == ">" or self.placeholder_is()

    def placeholder_is(self, name=None):
        line = self.screen.display[-1]
        cells = [self.screen.buffer[self.screen.lines - 1][x]
                 for x in range(2, self.screen.columns)
                 if self.screen.buffer[self.screen.lines - 1][x].data.strip()]
        return (line.startswith(">") and bool(cells)
                and all(cell.fg == "6c7086" for cell in cells)
                and (name is None or line[1:].strip() == name))

    def send(self, data):
        os.write(self.fd, data)

    def paste(self, text):
        self.send(b"\x1b[200~" + text.encode() + b"\x1b[201~")

    def rewind(self, expected):
        self.send(b"\x1b")
        for _ in range(3):
            self.pump(0.05)
        self.send(b"\x1b")
        self.wait(lambda: self.input_is(expected),
                  "rewind composer contains " + repr(expected))

    def replace_input(self, text):
        self.send(b"\x01\x0b")  # Ctrl+A, Ctrl+K: start of line, delete to end.
        self.paste(text)

    def snapshot(self, name):
        (self.root / (name + ".txt")).write_text(self.text)

    def messages(self, direction="client"):
        log = self.root / "rpc.jsonl"
        if not log.exists():
            return []
        return [record["message"] for line in log.read_text().splitlines()
                if (record := json.loads(line))["direction"] == direction]

    def requests(self, method):
        return [message for message in self.messages() if message.get("method") == method]

    def close(self):
        if self.output.closed:
            return
        self.snapshot("last-screen")
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
