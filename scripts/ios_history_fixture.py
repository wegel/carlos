"""Hold actual native history replies so XCTest can measure both sides of a prepend."""

from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from threading import Thread

from native_history_windows import NativeHistoryWindows
from support import Client


def configure_history(fixture):
    native = fixture.native
    peer = NativeHistoryWindows(native, turns=130)
    for index, turn in enumerate(peer.turns):
        if index == 0 or index >= 30 and index % 15 == 0 or index == 129:
            item = {"type": "userMessage", "id": f"history-user-{index}", "content": [{
                "type": "text", "text": f"history anchor {index} 🌍\nKeep this exact native text in place.\nOlder pages belong above it.\nFourth native line."}]}
        else:
            item = {"type": "reasoning", "id": f"empty-reason-{index}", "summary": [], "content": []}
        turn["items"] = [item]
    peer.fail_cursor = "PRIVATE_HISTORY:desc:100"
    peer.pause = True
    original = peer.reply_page

    async def reply(link, request):
        await original(link, request)
        peer.pause = True

    peer.reply_page = reply
    client = Client(fixture, "phone-history-owner")
    try:
        operation = client.operation(1)
        client.call("session/open", operation=operation, backend="codex", cwd=str(fixture.root),
                    target={"kind": "resume", "address": native.thread_id})
        assert client.outcome(operation)["state"]["kind"] == "accepted"
    finally:
        client.close()
    return peer


@contextmanager
def history_control(peer):
    class Release(BaseHTTPRequestHandler):
        def do_POST(self):
            if self.path != "/release" or not peer.held:
                self.send_error(409)
                return
            peer.release()
            self.send_response(200)
            self.send_header("Content-Length", "0")
            self.end_headers()

        def log_message(self, *_arguments):
            pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), Release)
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield server.server_port
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)
