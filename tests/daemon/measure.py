#!/usr/bin/env python3
"""Measure actual socket admission with an instant native peer, including fixture/client costs."""

import argparse
import json
from pathlib import Path
import statistics
import time

import support
from support import Client, Fixture


def connect(fixture, transport):
    if transport == "unix":
        return Client(fixture, "measured-client")
    from network_support import grant, NetworkClient
    client = NetworkClient(fixture, "measured-client", grant(fixture, "measured-device"))
    fixture.memory("hello:measured-client")
    return client


def idle(fixture):
    def counters():
        values = Path(f"/proc/{fixture.process.pid}/stat").read_text().split(")", 1)[1].split()
        switches = {}
        for task in Path(f"/proc/{fixture.process.pid}/task").iterdir():
            try:
                for line in (task / "status").read_text().splitlines():
                    if line.startswith(("voluntary_ctxt_switches:", "nonvoluntary_ctxt_switches:")):
                        key = task.name + ":" + line.split(":")[0]
                        switches[key] = int(line.split()[1])
            except FileNotFoundError:
                pass
        return {"cpu_ticks": int(values[11]) + int(values[12]), **switches}
    before = counters()
    start = time.monotonic()
    time.sleep(2)
    after = counters()
    return {"seconds": time.monotonic() - start, "process_cpu_ticks": after["cpu_ticks"] - before["cpu_ticks"],
            "thread_context_switch_deltas": {key: after[key] - before[key] for key in before
                                              if key != "cpu_ticks" and key in after},
            "scope": "idle daemon after native startup; each surviving thread context switches, process CPU ticks"}



def measure(rounds, transport="unix"):
    start = time.monotonic()
    fixture = Fixture("socket-measure-" + transport, listen="127.0.0.1:0" if transport == "websocket" else None)
    startup_ms = (time.monotonic() - start) * 1000
    start = time.monotonic()
    client = connect(fixture, transport)
    greeting_ms = (time.monotonic() - start) * 1000
    try:
        create = client.operation(1)
        session = client.call("session/create", operation=create, backend="grok", cwd=str(fixture.root), name=None)["session"]
        client.outcome(create)
        client.call("session/subscribe", session=session, since=None)
        idle_metrics = idle(fixture)
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
        return {"rounds": rounds, "transport": transport, "startup_ms": startup_ms,
                "provision_and_greeting_ms": greeting_ms, "idle": idle_metrics, "median_ms": statistics.median(elapsed),
                "p95_ms": sorted(elapsed)[(rounds*95+99)//100-1], "max_ms": max(elapsed),
                "samples_ms": elapsed,
                "daemon_memory": [json.loads(line) for line in (fixture.root / "daemon-memory.jsonl").read_text().splitlines()],
                "scope": "actual carlosd " + transport + " socket, owner and real Grok adapter; deterministic instant native peer, Python client and fixture logging included; admission is not model completion"}
    finally:
        client.close()
        fixture.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=support.REPO / "target/release/carlosd")
    parser.add_argument("--artifacts", type=Path, default=support.ARTIFACTS / "measurement")
    parser.add_argument("--rounds", type=int, default=1000)
    parser.add_argument("--transport", choices=("unix", "websocket"), default="unix")
    arguments = parser.parse_args()
    assert arguments.rounds > 0
    support.BINARY = arguments.binary.resolve()
    support.ARTIFACTS = arguments.artifacts.resolve()
    result = measure(arguments.rounds, arguments.transport)
    (support.ARTIFACTS / "roundtrips.json").write_text(json.dumps(result, indent=2))
    print(json.dumps({key: value for key, value in result.items() if key != "samples_ms"}))
