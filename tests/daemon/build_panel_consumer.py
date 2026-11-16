#!/usr/bin/env python3
"""Print the exact Rust test executable Cargo builds for the actual-native panel fixture."""

import json
from pathlib import Path
import subprocess
import sys


def main():
    profile = sys.argv[1]
    assert profile in ("debug", "release"), "expected a stock test profile"
    command = ["cargo", "test", "-p", "carlosd", "--test", "shared_panels", "--no-run", "--message-format=json"]
    if profile == "release":
        command.append("--release")
    result = subprocess.run(command, cwd=Path(__file__).resolve().parents[2],
                            stdout=subprocess.PIPE, text=True, check=True)
    artifacts = [message["executable"] for line in result.stdout.splitlines()
                 if (message := json.loads(line)).get("reason") == "compiler-artifact"
                 and message.get("target", {}).get("name") == "shared_panels"
                 and message.get("executable")]
    assert len(artifacts) == 1, "Cargo identifies exactly one matched fixture executable"
    print(artifacts[0])


if __name__ == "__main__":
    main()
