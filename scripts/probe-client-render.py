#!/usr/bin/env python3
"""Capture exact large source bodies through the actual daemon for the terminal renderer probe."""

import argparse
import hashlib
import json
from pathlib import Path
import sys
import time

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO / "tests/daemon"))
import support
from support import Client, Fixture, wait_for


def capture(root, name, text, tool):
    # Preserve the core's existing command heading and exit footer as well as the native bytes.
    expected = "$ printf probe\n" + text + "\nexit code: 0" if tool else text
    fixture = Fixture("render-probe-" + name, codex=True)
    client = None
    try:
        async def prompt(link, request):
            await fixture.native.notify(link, "turn/started", turn={"id": "probe-turn"})
            item = {"type": "commandExecution", "id": "probe-body", "command": "printf probe",
                    "status": "completed", "exitCode": 0, "aggregatedOutput": text} if tool else {
                    "type": "agentMessage", "id": "probe-body", "phase": "final_answer", "text": text}
            await fixture.native.notify(link, "item/completed", turnId="probe-turn", item=item)
            await fixture.native.notify(link, "turn/completed", turn={"id": "probe-turn", "status": "completed"})
        fixture.native.prompt = prompt
        client = Client(fixture, "render-source")
        create = client.operation(1)
        session = client.call("session/create", operation=create, backend="codex", cwd=str(fixture.root), name=None)["session"]
        client.outcome(create)
        client.call("session/subscribe", session=session, since=None)
        send = client.operation(2)
        client.call("session/send", session=session, operation=send, text="probe")
        client.outcome(send)

        def ready():
            entries = client.call("session/subscribe", session=session, since=None)["snapshot"]["entries"]
            return next((entry for entry in reversed(entries) if
                entry.get("source", {}).get("bytes") == str(len(text.encode())) or
                tool and entry.get("tool", {}).get("has_output")), None)
        entry = wait_for(ready, "large native source published")
        destination = root / name
        destination.mkdir(parents=True, exist_ok=True)
        (destination / "entry.json").write_text(json.dumps(entry, ensure_ascii=False))
        cursor, parts, reads, descriptor = None, [], 0, None
        fixture.memory("before-source")
        started = time.monotonic()
        with (destination / "chunks.jsonl").open("w") as output:
            while True:
                chunk = client.call("entry/source", session=session, entry=entry["id"], cursor=cursor, limit=65536)
                assert len(chunk["text"].encode()) <= 65536
                descriptor = chunk["source"]
                output.write(json.dumps(chunk, ensure_ascii=False) + "\n")
                parts.append(chunk["text"])
                reads += 1
                cursor = chunk["next"]
                if not chunk["has_more"]:
                    break
        assert "".join(parts) == expected, "wire source must preserve the complete original core body"
        fixture.memory("after-source")
        return {"name": name, "session": session, "bytes": len(expected.encode()), "reads": reads, "source": descriptor,
                "socket_read_ms": (time.monotonic() - started) * 1000,
                "sha256": hashlib.sha256(expected.encode()).hexdigest()}
    finally:
        if client:
            client.close()
        fixture.close()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=support.BINARY)
    parser.add_argument("--artifacts", type=Path, default=REPO / "target/render-probe")
    arguments = parser.parse_args()
    support.BINARY = arguments.binary.resolve()
    root = arguments.artifacts.resolve()
    support.ARTIFACTS = root / "daemon"
    root.mkdir(parents=True, exist_ok=True)
    cases = [capture(root, "markdown", "[ref]: https://example.test\n\n```text\n" +
                     "**literal 🌍** [ref]\n" * 24000 + "```\n\nTail [link][ref]\n", False),
             capture(root, "ansi", "\x1b[31m" + "red 🌍 still red\n" * 24000 + "tail stays red", True)]
    (root / "manifest.json").write_text(json.dumps({"cases": cases}, indent=2) + "\n")
    print(json.dumps({"artifacts": str(root), "cases": cases}, indent=2))


if __name__ == "__main__":
    main()
