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
    process = subprocess.Popen([os.environ["CARLOS_TEST_CODEX"], *sys.argv[1:]],
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                               stderr=(root / "codex-stderr.log").open("w"))
    lock = threading.Lock()

    def copy(source, destination, direction):
        for line in source:
            with lock, (root / "rpc.jsonl").open("a") as log:
                log.write(json.dumps({"direction": direction, "message": json.loads(line)}) + "\n")
            destination.write(line)
            destination.flush()

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
