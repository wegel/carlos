#!/usr/bin/env python3
"""Sample only this simulator's Carlos process while its UI test waits on loopback."""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import plistlib
import secrets
import subprocess
import sys
from threading import Thread
import time


def process(simulator):
    prefix = str(Path.home() / "Library/Developer/CoreSimulator/Devices" / simulator) + "/"
    output = subprocess.check_output(["ps", "-axww", "-o", "pid=,time=,rss=,comm="], text=True)
    matches = [line.split(None, 3) for line in output.splitlines()
               if prefix in line and line.rstrip().endswith("/Carlos.app/Carlos")]
    matches = [row for row in matches
               if plistlib.loads((Path(row[3]).parent / "Info.plist").read_bytes())["CFBundleIdentifier"]
               == "app.carlos.phone.fixture"]
    if len(matches) != 1:
        raise RuntimeError("Expected exactly one Carlos process in the owned simulator")
    pid, cpu, rss, _path = matches[0]
    fields = [float(field) for field in cpu.split(":")]
    seconds = sum(value * 60 ** index for index, value in enumerate(reversed(fields)))
    return {"pid": int(pid), "cpuSeconds": seconds, "rssKiB": int(rss)}


def run(simulator, fixture, destination, command):
    token = "/idle/" + secrets.token_hex(24)

    class Idle(BaseHTTPRequestHandler):
        def do_POST(self):
            if self.path != token:
                self.send_error(404)
                return
            try:
                # Let layout and initial body reads settle before the measured window.
                time.sleep(2)
                start = time.monotonic()
                samples = [process(simulator)]
                for _index in range(20):
                    time.sleep(.5)
                    samples.append(process(simulator))
                elapsed = time.monotonic() - start
                assert len({sample["pid"] for sample in samples}) == 1
                result = {"method": "owned simulator ps CPU-time delta and resident pages",
                          "seconds": elapsed, "cpuPercent": 100 * (samples[-1]["cpuSeconds"] - samples[0]["cpuSeconds"]) / elapsed,
                          "rssPeakKiB": max(sample["rssKiB"] for sample in samples), "samples": samples}
                Path(destination).write_text(json.dumps(result, indent=2) + "\n")
                self.send_response(200)
            except Exception as error:
                print(f"Idle check failed: {error}", file=sys.stderr)
                self.send_response(500)
            self.send_header("Content-Length", "0")
            self.end_headers()

        def log_message(self, *_arguments):
            pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), Idle)
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    path = Path(fixture)
    original = path.read_text()
    try:
        config = json.loads(original)
        config["idleControlURL"] = f"http://127.0.0.1:{server.server_port}{token}"
        path.write_text(json.dumps(config))
        return subprocess.run(command).returncode
    finally:
        path.write_text(original)
        server.shutdown()
        server.server_close()
        thread.join(timeout=15)


if __name__ == "__main__":
    raise SystemExit(run(sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4:]))
