"""Relay local daemon bytes unchanged and save both sides for deterministic terminal checks."""

import json
import socket
import threading


class DaemonRecorder:
    def __init__(self, path, upstream, root, source_error_once=False, goal_error_once=False, source_read_error_after=0, defer_source_watch_once=False, defer_subscription_once=False):
        self.source_error_once = source_error_once
        self.source_read_error_after = source_read_error_after
        self.goal_error_once = goal_error_once
        self.goal_error_id = None
        self.task_page_error_once = False
        self.task_page_error_id = None
        self.device_write = threading.Lock()
        self.drop_text_once = False
        self.hold_sources = False
        self.held_sources = []
        self.defer_source_watch_once = defer_source_watch_once
        self.deferred_source_watch = None
        self.defer_subscription_once = defer_subscription_once
        self.deferred_subscription = None
        self.source_watch_held = threading.Event()
        self.path = path
        self.upstream = upstream
        self.root = root
        self.stop = threading.Event()
        self.hold_task_pages = False
        self.task_page_held = threading.Event()
        self.task_page_release = threading.Event()
        self.task_page_release.set()
        self.hold_queue_pages = False
        self.queue_page_held = threading.Event()
        self.queue_page_release = threading.Event()
        self.queue_page_release.set()
        self.hold_subscriptions = False
        self.subscription_error_once = False
        self.subscription_held = threading.Event()
        self.subscription_release = threading.Event()
        self.subscription_release.set()
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
                    if direction == "client" and self.goal_error_once and message.get("method") == "goal/subscribe":
                        self.goal_error_once = False
                        self.goal_error_id = message["id"]
                    elif direction == "server" and self.goal_error_id is not None and message.get("id") == self.goal_error_id:
                        self.goal_error_id = None
                        message = {"jsonrpc":"2.0", "id":message["id"], "error":{
                            "code":-32012, "message":"Injected admitted goal read failure"}}
                        line = json.dumps(message).encode()
                    if direction == "client" and self.task_page_error_once and message.get("method") == "task/list":
                        self.task_page_error_once = False
                        self.task_page_error_id = message["id"]
                    elif direction == "server" and self.task_page_error_id is not None and message.get("id") == self.task_page_error_id:
                        self.task_page_error_id = None
                        message = {"jsonrpc":"2.0", "id":message["id"], "error":{
                            "code":-32012, "message":"Injected private task page failure"}}
                        line = json.dumps(message).encode()
                    result = message.get("result")
                    subscription_reply = direction == "server" and isinstance(result, dict) and result.get("kind") in ("snapshot", "replay")
                    if subscription_reply and self.subscription_error_once:
                        self.subscription_error_once = False
                        message = {"jsonrpc":"2.0", "id":message["id"], "error":{
                            "code":-32012, "message":"Injected resumed subscription failure"}}
                        line = json.dumps(message).encode()
                    self.record(direction, message)
                    if direction == "client" and self.hold_queue_pages and message.get("method") == "queue/read" and message.get("params", {}).get("after") is not None:
                        self.queue_page_held.set()
                        self.wait_gate(self.queue_page_release)
                    elif direction == "client" and self.hold_task_pages and message.get("method") == "task/list":
                        self.task_page_held.set()
                        self.wait_gate(self.task_page_release)
                    elif subscription_reply and self.hold_subscriptions:
                        self.subscription_held.set()
                        self.wait_gate(self.subscription_release)
                    if self.stop.is_set():
                        break
                    error = self.source_failure(message) if direction == "client" else None
                    if direction == "client" and self.defer_subscription_once and message.get("method") == "session/subscribe":
                        self.defer_subscription_once = False
                        with self.device_write:
                            self.deferred_subscription = (source, message["id"])
                            self.subscription_held.set()
                    elif direction == "client" and self.defer_source_watch_once and message.get("method") == "entry/source/subscribe":
                        self.defer_source_watch_once = False
                        with self.device_write:
                            self.deferred_source_watch = (source, message["id"])
                            self.source_watch_held.set()
                    elif error is not None:
                        self.record("server", error)
                        with self.device_write:
                            source.sendall(json.dumps(error).encode()+b"\n")
                    elif direction == "server":
                        with self.device_write:
                            if self.drop_text_once and message.get("method") == "session/event" and message.get("params",{}).get("event",{}).get("kind") == "text_appended":
                                self.drop_text_once = False
                            elif self.hold_sources and message.get("method") == "entry/source":
                                self.held_sources.append((target, line+b"\n"))
                                assert sum(len(data) for _,data in self.held_sources) <= 1024*1024
                            else:
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

    def source_failure(self, message):
        method = message.get("method")
        if method == "entry/source/subscribe" and self.source_error_once:
            self.source_error_once = False
            reason = "Injected daemon source read failure"
        elif method == "entry/source" and self.source_read_error_after > 0:
            self.source_read_error_after -= 1
            if self.source_read_error_after:
                return None
            reason = "Injected direct source read failure"
        else:
            return None
        return {"jsonrpc":"2.0", "id":message["id"], "error":{"code":-32012,"message":reason}}

    def record(self, direction, message):
        with self.lock, (self.root / "rpc.jsonl").open("a") as output:
            output.write(json.dumps({"direction":direction,"message":message}, ensure_ascii=False)+"\n")

    def wait_gate(self, release):
        while not self.stop.is_set() and not release.wait(.05):
            pass

    def pause_task_pages(self):
        self.task_page_held.clear()
        self.task_page_release.clear()
        self.hold_task_pages = True

    def resume_task_pages(self):
        self.hold_task_pages = False
        self.task_page_release.set()

    def pause_queue_pages(self):
        self.queue_page_held.clear()
        self.queue_page_release.clear()
        self.hold_queue_pages = True

    def resume_queue_pages(self):
        self.hold_queue_pages = False
        self.queue_page_release.set()

    def pause_subscriptions(self, fail=False):
        self.subscription_error_once = fail
        self.subscription_held.clear()
        self.subscription_release.clear()
        self.hold_subscriptions = True

    def resume_subscriptions(self):
        self.hold_subscriptions = False
        self.subscription_release.set()

    def pause_sources(self):
        with self.device_write:
            self.hold_sources = True

    def reject_deferred_source_watch(self):
        self.finish_deferred_source_watch({"error":{
            "code":-32012, "message":"output watch was superseded"}})

    def reject_deferred_subscription(self):
        with self.device_write:
            device, identity = self.deferred_subscription
            self.deferred_subscription = None
            message = {"jsonrpc":"2.0", "id":identity, "error":{
                "code":-32012, "message":"subscription was superseded"}}
            self.record("server", message)
            device.sendall(json.dumps(message).encode()+b"\n")

    def finish_deferred_source_watch(self, reply):
        with self.device_write:
            device, identity = self.deferred_source_watch
            self.deferred_source_watch = None
            message = {"jsonrpc":"2.0", "id":identity, **reply}
            self.record("server", message)
            device.sendall(json.dumps(message).encode()+b"\n")

    def resume_sources(self):
        with self.device_write:
            self.hold_sources = False
            for target, data in self.held_sources:
                target.sendall(data)
            self.held_sources.clear()

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
