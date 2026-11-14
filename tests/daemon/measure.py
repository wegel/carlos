#!/usr/bin/env python3
"""Measure actual socket admission with an instant native peer, including fixture/client costs."""

import argparse
import json
from pathlib import Path
import statistics
import time

import support
from support import Client, Fixture


def measure(rounds):
    fixture = Fixture("socket-measure")
    client = Client(fixture, "measured-client")
    try:
        create = client.operation(1)
        session = client.call("session/create", operation=create, backend="grok", cwd=str(fixture.root), name=None)["session"]
        client.outcome(create)
        client.call("session/subscribe", session=session, since=None)
        elapsed = []
        for counter in range(2, rounds+2):
            operation = client.operation(counter)
            client.messages.clear()
            start = time.perf_counter_ns()
            client.call("session/send", session=session, operation=operation, text="measured prompt", policy="queue")
            result = client.message(lambda message: message.get("method") == "operation/updated" and
                                    message["params"]["operation"] == operation and
                                    message["params"]["state"]["kind"] not in ("pending", "queued"))["params"]
            elapsed.append((time.perf_counter_ns()-start)/1_000_000)
            assert result["state"]["kind"] == "accepted", result
        assert len(fixture.native_requests("session/prompt")) == rounds
        fixture.memory("after-native-admissions")
        return {"rounds": rounds, "median_ms": statistics.median(elapsed),
                "p95_ms": sorted(elapsed)[(rounds-1)*95//100], "max_ms": max(elapsed),
                "samples_ms": elapsed,
                "daemon_memory": [json.loads(line) for line in (fixture.root / "daemon-memory.jsonl").read_text().splitlines()],
                "scope": "actual carlosd Unix socket, owner and real Grok adapter; deterministic instant native peer, Python client and fixture logging included; admission is not model completion"}
    finally:
        client.close()
        fixture.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=support.REPO / "target/release/carlosd")
    parser.add_argument("--artifacts", type=Path, default=support.ARTIFACTS / "measurement")
    parser.add_argument("--rounds", type=int, default=1000)
    arguments = parser.parse_args()
    assert arguments.rounds > 0
    support.BINARY = arguments.binary.resolve()
    support.ARTIFACTS = arguments.artifacts.resolve()
    result = measure(arguments.rounds)
    (support.ARTIFACTS / "roundtrips.json").write_text(json.dumps(result, indent=2))
    print(json.dumps({key: value for key, value in result.items() if key != "samples_ms"}))
