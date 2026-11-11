"""Build native-format session fixtures only inside each terminal test directory."""

import json
import re
import uuid


def session_file(root, session):
    return root / "claude-home/projects" / re.sub(r"[^a-zA-Z0-9]", "-", str(root)) / (session + ".jsonl")


def write_session(root, texts, sources=None, title=None, compact=False):
    session = str(uuid.uuid4())
    rows = []
    for index, text in enumerate(texts):
        source = (sources or ["human"] * len(texts))[index]
        rows.append({"type": "user", "uuid": str(uuid.uuid4()), "parentUuid": rows[-1]["uuid"] if rows else None,
                     "message": {"role": "user", "content": text},
                     **({"origin": {"kind": source}} if source != "unknown" else {})})
        rows.append({"type": "assistant", "uuid": str(uuid.uuid4()), "parentUuid": rows[-1]["uuid"],
                     "message": {"id": str(uuid.uuid4()), "role": "assistant", "content": [{"type": "text", "text": "Answer: " + text}]}})
    if compact:
        rows.append({"type": "system", "subtype": "compact_boundary", "uuid": str(uuid.uuid4()), "parentUuid": None,
                     "logicalParentUuid": rows[-1]["uuid"], "content": "Compacted"})
        rows.append({"type": "user", "uuid": str(uuid.uuid4()), "parentUuid": rows[-1]["uuid"],
                     "isCompactSummary": True, "isVisibleInTranscriptOnly": True,
                     "message": {"role": "user", "content": "Automatic compaction summary"}})
    if title:
        rows.append({"type": "custom-title", "customTitle": title})
    rows = [{"cwd": str(root), "sessionId": session, "entrypoint": "sdk-ts", "version": "2.1.322",
             "timestamp": "2026-11-10T00:00:00.000Z", **row} for row in rows]
    path = session_file(root, session)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(row, ensure_ascii=False) + "\n" for row in rows))
    return session, path, rows
