"""Keep a browser consent check local: a real page visit never answers the native callback."""

from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from threading import Thread


@contextmanager
def browser_page(native):
    """Expose only a fixed completion page and record whether Carlos already sent consent."""
    visits = []

    class Page(BaseHTTPRequestHandler):
        def do_GET(self):
            if self.path != "/complete":
                self.send_error(404)
                return
            visits.append({"answers_before_open": len(native.task_answers)})
            body = ("<!doctype html><meta name='viewport' content='width=device-width,initial-scale=1'>"
                    "<style>body{background:#1e1e2e;color:#cdd6f4;font:17px system-ui;padding:32px 24px}"
                    "h1{font-size:28px;color:#a6e3a1}p{line-height:1.5}</style>"
                    "<h1>Browser step complete</h1><p>Return to Carlos to choose whether to allow this request.</p>").encode()
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *_arguments):
            pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), Page)
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield server.server_port, visits
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)
