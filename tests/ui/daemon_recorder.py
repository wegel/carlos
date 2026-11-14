"""Relay local daemon bytes unchanged and save both sides for deterministic terminal checks."""

import json
import socket
import threading


class DaemonRecorder:
    def __init__(self, path, upstream, root, source_error_once=False):
        self.source_error_once = source_error_once
        self.device_write = threading.Lock()
        self.path = path
        self.upstream = upstream
        self.root = root
        self.stop = threading.Event()
        self.lock = threading.Lock()
        self.errors = []
        self.peers = set()
        self.listener = socket.socket(socket.AF_UNIX)
        self.listener.bind(str(path))
        self.listener.listen(4)
        self.listener.settimeout(.1)
        self.thread = threading.Thread(target=self.run, daemon=True)
        self.thread.start()

    def run(self):
        while not self.stop.is_set():
            try:
                device, _ = self.listener.accept()
            except socket.timeout:
                continue
            except OSError:
                return
            daemon = socket.socket(socket.AF_UNIX)
            daemon.connect(str(self.upstream))
            for peer in (device, daemon):
                peer.settimeout(.1)
                self.peers.add(peer)
            first = threading.Thread(target=self.relay, args=(device, daemon, "client"), daemon=True)
            second = threading.Thread(target=self.relay, args=(daemon, device, "server"), daemon=True)
            first.start()
            second.start()
            first.join()
            second.join()
            for peer in (device, daemon):
                self.peers.discard(peer)
                peer.close()

    def relay(self, source, target, direction):
        retained = bytearray()
        try:
            while not self.stop.is_set():
                try:
                    data = source.recv(65536)
                except socket.timeout:
                    continue
                if not data:
                    break
                retained.extend(data)
                assert len(retained) <= 16 * 1024 * 1024, "recorder frame bound"
                while b"\n" in retained:
                    line, _, retained = retained.partition(b"\n")
                    message = json.loads(line)
                    self.record(direction, message)
                    if direction == "client" and self.source_error_once and message.get("method") == "entry/source/subscribe":
                        self.source_error_once = False
                        error = {"jsonrpc":"2.0","id":message["id"],"error":{
                            "code":-32012,"message":"Injected daemon source read failure"}}
                        self.record("server", error)
                        with self.device_write:
                            source.sendall(json.dumps(error).encode()+b"\n")
                    elif direction == "server":
                        with self.device_write:
                            target.sendall(line+b"\n")
                    else:
                        target.sendall(line+b"\n")
        except OSError:
            if not self.stop.is_set():
                pass  # Either peer may close while the other direction drains.
        except Exception as error:
            self.errors.append(repr(error))
        finally:
            try:
                target.shutdown(socket.SHUT_WR)
            except OSError:
                pass

    def record(self, direction, message):
        with self.lock, (self.root / "rpc.jsonl").open("a") as output:
            output.write(json.dumps({"direction":direction,"message":message}, ensure_ascii=False)+"\n")

    def close(self):
        self.stop.set()
        self.listener.close()
        for peer in list(self.peers):
            try:
                peer.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
        self.thread.join(timeout=3)
        assert not self.thread.is_alive(), "daemon recorder did not stop"
        assert not self.errors, self.errors
