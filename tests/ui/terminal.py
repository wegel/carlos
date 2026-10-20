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
import termios
import time

import pyte


class Terminal:
    def __init__(self, binary, root, mode="modern", resume="thread-original", width=100, backend="codex"):
        self.root = root
        self.root.mkdir(parents=True, exist_ok=True)
        shim = root / "bin"
        shim.mkdir(exist_ok=True)
        (shim / "codex").symlink_to(Path(__file__).with_name("fake_codex.py").resolve())
        (shim / "claude").symlink_to(Path(__file__).with_name("fake_claude.py").resolve())
        self.screen = pyte.Screen(width, 32)
        self.stream = pyte.Stream(self.screen)
        self.decoder = codecs.getincrementaldecoder("utf-8")("replace")
        self.output = (root / "terminal.ansi").open("ab")
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            os.chdir(root)
            env = {**os.environ, "TERM": "xterm-256color", "CARLOS_TEST_ROOT": str(root),
                   "CARLOS_TEST_SERVER": mode, "HOME": str(root),
                   "CODEX_HOME": str(root / "codex-home"), "CARLOS_METRICS": "0",
                   "CLAUDE_CONFIG_DIR": str(root / "claude-home"),
                   "CARLOS_EVENT_TRACE": str(root / "keys.log"),
                   "PATH": str(shim) + os.pathsep + os.environ["PATH"]}
            args = [str(binary), "--backend", backend]
            if resume:
                args += ["resume", resume]
            os.execve(binary, args, env)
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
        return self.screen.display[-1].strip() == "> " + text if text else self.screen.display[-1].strip() == ">"

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
        self.snapshot("last-screen")
        try:
            self.send(b"\x03")
        except OSError:
            pass
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            self.pump()
            done, status = os.waitpid(self.pid, os.WNOHANG)
            if done:
                break
        else:
            os.kill(self.pid, signal.SIGKILL)
            _, status = os.waitpid(self.pid, 0)
        os.close(self.fd)
        self.output.close()
        assert os.waitstatus_to_exitcode(status) == 0, f"Carlos exited abnormally; see {self.root}"
