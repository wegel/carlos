#!/usr/bin/env python3
"""Record Carlos traffic while forwarding it to the installed Codex binary."""

import json
import os
from pathlib import Path
import subprocess
import sys
import threading


def main():
    root = Path(os.environ["CARLOS_TEST_ROOT"])
    if sys.argv[1:]==["features","list"]:
        os.execv(os.environ["CARLOS_TEST_CODEX"],[os.environ["CARLOS_TEST_CODEX"],*sys.argv[1:]])
    with (root/"native-launches.jsonl").open("a") as log:
        log.write(json.dumps({"args":sys.argv[1:]})+"\n")
    process = subprocess.Popen([os.environ["CARLOS_TEST_CODEX"], *sys.argv[1:]],
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                               stderr=(root / "codex-stderr.log").open("w"))
    with (root/"native-processes.jsonl").open("a") as log:
        log.write(json.dumps({"pid":process.pid,"args":sys.argv[1:]})+"\n")
    lock = threading.Lock()

    def copy(source, destination, direction):
        for line in source:
            with lock, (root / "rpc.jsonl").open("a") as log:
                log.write(json.dumps({"direction": direction, "message": json.loads(line)}) + "\n")
            destination.write(line)
            destination.flush()
        if direction == "server":
            os.close(sys.stdout.fileno())  # Propagate native EOF even while stdin is quiet.

    reader = threading.Thread(target=copy, args=(process.stdout, sys.stdout.buffer, "server"), daemon=True)
    reader.start()
    try:
        copy(sys.stdin.buffer, process.stdin, "client")
    finally:
        process.stdin.close()
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            process.terminate()
            process.wait(timeout=3)
        reader.join(timeout=3)


if __name__ == "__main__":
    main()
