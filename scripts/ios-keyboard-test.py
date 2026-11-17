#!/usr/bin/env python3
"""Send raw HID Return only to this test's owned simulator, outside the app product."""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import secrets
import subprocess
import sys
from threading import Thread


def run(binary, simulator, fixture, command):
    token = secrets.token_hex(32)

    class Keys(BaseHTTPRequestHandler):
        def do_POST(self):
            prefix = f"/{token}/"
            modifier = self.path.removeprefix(prefix)
            if not self.path.startswith(prefix) or modifier not in ("plain", "shift", "option", "x"):
                self.send_error(404)
                return
            result = subprocess.run([binary, simulator, modifier], capture_output=True, timeout=15)
            self.send_response(200 if result.returncode == 0 else 500)
            self.send_header("Content-Length", "0")
            self.end_headers()
            if result.returncode:
                sys.stderr.write(result.stderr.decode(errors="replace"))

        def log_message(self, *_arguments):
            pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), Keys)
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    path = Path(fixture)
    original = path.read_text()
    try:
        data = json.loads(original)
        data["keyboardControlURL"] = f"http://127.0.0.1:{server.server_port}/{token}/"
        path.write_text(json.dumps(data))
        return subprocess.run(command).returncode
    finally:
        path.write_text(original)
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)


if __name__ == "__main__":
    raise SystemExit(run(sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4:]))
