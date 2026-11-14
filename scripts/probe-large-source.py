#!/usr/bin/env python3
"""Capture large original bodies from the real daemon without enlarging any client cache."""

import argparse
import hashlib
import json
from pathlib import Path
import sys
import time

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO / "tests/daemon"))
import support


def fill_body(size, prefix, line, suffix):
    available = size - len(prefix.encode()) - len(suffix.encode())
    count, padding = divmod(available, len(line.encode()))
    body = prefix + line * count + "x" * padding + suffix
    assert len(body.encode()) == size
    return body


def capture(root, name, body, tool):
    body_bytes = body.encode()
    fixture = support.Fixture("large-source-" + name, codex=True)
    fixture.native.history = lambda: {"id":"native-root", "status":{"type":"idle"},
        "model":"fixture-model", "reasoningEffort":"low", "turns":[]}
    client = None
    try:
        async def prompt(link, request):
            await fixture.native.notify(link, "turn/started", turn={"id":"large-probe"})
            if tool:
                await fixture.native.notify(link, "item/started", turnId="large-probe", item={
                    "type":"commandExecution", "id":"large-body", "command":"printf probe", "status":"inProgress"})
            for offset in range(0, len(body), 32768):
                delta = body[offset:offset+32768]
                assert len(delta.encode()) <= 65536
                await fixture.native.notify(link,
                    "item/commandExecution/outputDelta" if tool else "item/agentMessage/delta",
                    turnId="large-probe", itemId="large-body", delta=delta)
            # Leave the turn open with a quiescent, fully received current body. A completed
            # command item must carry its final output, which would exceed the native frame cap.
        fixture.native.prompt = prompt
        client = support.Client(fixture, "large-source-capture")
        operation = client.operation(1)
        session = client.call("session/create", operation=operation, backend="codex",
                              cwd=str(fixture.root), name=None)["session"]
        client.outcome(operation)
        support.wait_for(lambda:client.call("session/subscribe", session=session, since=None)
                         ["snapshot"]["info"]["availability"] == "ready", "empty native session ready")
        fixture.memory("before-native-source")
        operation = client.operation(2)
        client.call("session/send", session=session, operation=operation, text="large original source probe")
        client.outcome(operation)

        def completed():
            snapshot = client.call("session/subscribe", session=session, since=None)["snapshot"]
            if tool:
                entry = next((entry for entry in reversed(snapshot["entries"]) if
                    (entry.get("tool") or {}).get("has_output")), None)
                if entry is None:
                    return None
                chunk = client.call("entry/source", session=session, entry=entry["id"], cursor=None, limit=64)
                if int(chunk["source"]["bytes"]) != len(body_bytes):
                    return None
                return entry
            return next((entry for entry in reversed(snapshot["entries"]) if
                int((entry.get("source") or {}).get("bytes",0)) == len(body_bytes)), None)

        entry = support.wait_for(completed, "all expected current source bytes published", timeout=60)
        destination = root / name
        destination.mkdir(parents=True, exist_ok=True)
        (destination / "entry.json").write_text(json.dumps(entry, ensure_ascii=False))
        expected = hashlib.sha256()
        expected.update(body_bytes)
        expected_digest = expected.hexdigest()
        digest = hashlib.sha256()
        cursor, reads, offset = None, 0, 0
        fixture.memory("complete-native-source")
        started = time.monotonic()
        with (destination / "chunks.jsonl").open("w") as output:
            while True:
                chunk = client.call("entry/source", session=session, entry=entry["id"], cursor=cursor, limit=65536)
                data = chunk["text"].encode()
                assert int(chunk["offset"]) == offset and len(data) <= 65536
                offset += len(data)
                digest.update(data)
                output.write(json.dumps(chunk, ensure_ascii=False) + "\n")
                reads += 1
                cursor = chunk["next"]
                if not chunk["has_more"]:
                    break
        assert digest.hexdigest() == expected_digest, "source reads preserve every original byte"
        fixture.memory("after-source-reads")
        record = {"name":name, "session":session, "source":chunk["source"], "bytes":offset,
                  "reads":reads, "sha256":expected_digest, "running":True, "socket_read_ms":(time.monotonic()-started)*1000}
        print(json.dumps(record), flush=True)
        return record
    finally:
        if client:
            client.close()
        fixture.close()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=REPO / "target/release/carlosd")
    parser.add_argument("--artifacts", type=Path, default=REPO / "target/large-source-probe")
    parser.add_argument("--case", choices=("ansi","markdown","both"), default="both")
    parser.add_argument("--mib", type=int, help="smaller smoke body; defaults to64MiB ANSI/16MiB Markdown")
    args = parser.parse_args()
    support.BINARY = args.binary.resolve()
    root = args.artifacts.resolve()
    root.mkdir(parents=True, exist_ok=True)
    support.ARTIFACTS = root / "daemon"
    cases = []
    if args.case in ("ansi","both"):
        body = fill_body((args.mib or 64)*1024*1024, "\x1b[31m",
                         "red " + "sample "*10 + "stays red\n", "large ansi tail stays red")
        cases.append(capture(root,"ansi",body,True))
    if args.case in ("markdown","both"):
        body = fill_body((args.mib or 16)*1024*1024, "[ref]: https://example.test\n\n```text\n",
                         "**literal 🌍** [ref] " + "sample "*8 + "\n", "\n```\n\nLarge Markdown tail [link][ref]\n")
        cases.append(capture(root,"markdown",body,False))
    identity = {"path":str(support.BINARY), "sha256":hashlib.sha256(support.BINARY.read_bytes()).hexdigest()}
    (root / "manifest.json").write_text(json.dumps({"daemon":identity,"cases":cases},indent=2)+"\n")


if __name__ == "__main__":
    main()
