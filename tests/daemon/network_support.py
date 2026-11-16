"""Use the real WebSocket listener and operator CLI while keeping synthetic credentials out of traces."""

import json
import subprocess

from websockets.sync.client import connect

import support
from support import Client


def grant(fixture, name):
    path = fixture.root / (name + ".token")
    result = subprocess.run([str(support.BINARY), "device", "add", "--socket", str(fixture.socket),
                             "--name", name, "--token-file", str(path)], env=fixture.env,
                            cwd=fixture.root, capture_output=True, text=True, timeout=10)
    assert result.returncode == 0, result.stderr
    return path.read_text().strip()


class NetworkClient(Client):
    def __init__(self, fixture, name, token=None, resume=None, greet=True, origin=None, max_queue=16):
        self.path = fixture.root / (name + ".jsonl")
        self.context = connect(fixture.ws_uri, origin=origin, compression=None, proxy=None,
                               open_timeout=3, close_timeout=.2, ping_interval=None, max_queue=max_queue,
                               max_size=8 * 1024 * 1024)
        self.socket = self.context.__enter__()
        self.closed = False
        self.messages = []
        self.serial = 0
        self.hello = None
        if greet:
            params = self.greeting(name, token, resume)
            try:
                self.hello = self.call("hello", **params)
            except BaseException:
                self.close()
                raise

    @staticmethod
    def greeting(name, token, resume=None):
        params = {"protocol": 1, "client": name, "token": token or ""}
        if resume:
            params.update(resume=resume["client"], resume_token=resume["resume_token"], instance=resume["instance"])
        return params

    def record(self, direction, message):
        safe = dict(message)
        for field in ("params", "result"):
            body = message.get(field)
            if isinstance(body, dict) and any(key in body for key in ("token", "resume_token")):
                safe[field] = {key: "[redacted]" if key in ("token", "resume_token") else value
                               for key, value in body.items()}
        super().record(direction, safe)

    def send(self, method, **params):
        self.serial += 1
        message = {"jsonrpc": "2.0", "id": self.serial, "method": method, "params": params}
        self.record("client", message)
        self.socket.send(json.dumps(message))
        return self.serial

    def receive(self, timeout=8):
        message = json.loads(self.socket.recv(timeout=timeout))
        self.record("server", message)
        self.messages.append(message)
        return message

    def close(self):
        if not self.closed:
            self.closed = True
            self.context.__exit__(None, None, None)
