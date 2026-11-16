#!/usr/bin/env python3
"""Read identical large original source through the actual daemon over either socket type."""

import argparse
import hashlib
import json
from pathlib import Path
import statistics
import time

from measure import connect
import support
from support import Fixture, wait_for


def measure(transport):
    fixture = Fixture("source-measure-" + transport, codex=True,
                      listen="127.0.0.1:0" if transport == "websocket" else None)
    client = connect(fixture, transport)
    try:
        operation = client.operation(1)
        session = client.call("session/create", operation=operation, backend="codex",
                              cwd=str(fixture.root), name=None)["session"]
        assert client.outcome(operation)["state"]["kind"] == "accepted"
        client.call("session/subscribe", session=session, since=None)
        operation = client.operation(2)
        client.call("session/send", session=session, operation=operation, text="large-plain-source")
        assert client.outcome(operation)["state"]["kind"] == "accepted"
        expected = ("\0🌍\x1b[31m\n" * (4096 * 224)).encode()

        def ready():
            snapshot = client.call("session/subscribe", session=session, since=None)["snapshot"]
            return next((row for row in snapshot["entries"] if row.get("source", {}).get("bytes") == str(len(expected))
                         and row["done"]), None)

        entry = wait_for(ready, "large original source is ready", timeout=15)
        samples, count, digest, cursor = [], 0, hashlib.sha256(), None
        start = time.perf_counter_ns()
        while True:
            client.messages.clear()
            began = time.perf_counter_ns()
            chunk = client.call("entry/source", session=session, entry=entry["id"], cursor=cursor, limit=65536)
            samples.append((time.perf_counter_ns() - began) / 1_000_000)
            data = chunk["text"].encode()
            assert len(data) <= 65536
            digest.update(data)
            count += len(data)
            cursor = chunk["next"]
            if not chunk["has_more"]:
                assert chunk["done"]
                break
        elapsed = (time.perf_counter_ns() - start) / 1_000_000
        assert count == len(expected) and digest.digest() == hashlib.sha256(expected).digest()
        fixture.memory("after-source-transfer")
        return {"transport": transport, "source_bytes": count, "sha256": digest.hexdigest(),
                "chunks": len(samples), "wall_ms": elapsed, "median_ms": statistics.median(samples),
                "p95_ms": sorted(samples)[(len(samples) * 95 + 99) // 100 - 1], "max_ms": max(samples),
                "samples_ms": samples, "daemon_memory": [json.loads(line) for line in
                    (fixture.root / "daemon-memory.jsonl").read_text().splitlines()],
                "scope": "actual daemon, deterministic attached Codex fixture; original source with NUL, Unicode and ANSI; Python decode/hash and fixture logging included; source preparation excluded"}
    finally:
        client.close()
        fixture.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=support.REPO / "target/release/carlosd")
    parser.add_argument("--artifacts", type=Path, default=support.ARTIFACTS / "source-measurement")
    parser.add_argument("--transport", choices=("unix", "websocket"), default="unix")
    arguments = parser.parse_args()
    support.BINARY = arguments.binary.resolve()
    support.ARTIFACTS = arguments.artifacts.resolve()
    result = measure(arguments.transport)
    (support.ARTIFACTS / "source-transfer.json").write_text(json.dumps(result, indent=2))
    print(json.dumps({key: value for key, value in result.items() if key != "samples_ms"}))
