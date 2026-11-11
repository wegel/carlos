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


def write_private_children(root):
    session, path, rows = write_session(root, ["saved main human"])
    def append(kind, content, **tags):
        row = {**rows[0], "type":kind,"uuid":str(uuid.uuid4()),"parentUuid":rows[-1]["uuid"],
               "message":{"id":str(uuid.uuid4()),"role":kind,"content":content}, **tags}
        rows.append(row)
    for tags in [{"agent_id":"child"}, {"parent_tool_use_id":"caller"}, {"parent_agent_id":"nested"}, {"isSidechain":True}]:
        identifier = str(uuid.uuid4())
        append("assistant", [{"type":"text","text":"PRIVATE_HISTORY_TEXT"},
               {"type":"thinking","thinking":"PRIVATE_HISTORY_THOUGHT"},
               {"type":"tool_use","id":identifier,"name":"Bash","input":{"command":"echo PRIVATE_HISTORY_COMMAND"}}], **tags)
        append("user", [{"type":"tool_result","tool_use_id":identifier,"content":"PRIVATE_HISTORY_RESULT"}], **tags)
    append("user", "PRIVATE_HISTORY_REPORT", origin={"kind":"task-notification","runId":"child-run"})
    append("user", "Visible cross-session peer", origin={"kind":"peer","fromSession":"another"})
    append("user", "Human <task-notification> literal", origin={"kind":"human"})
    append("assistant", [{"type":"tool_use","id":"main-call","name":"Bash","input":{"command":"echo MAIN_HISTORY_COMMAND"}}])
    append("user", [{"type":"tool_result","tool_use_id":"main-call","content":"MAIN_HISTORY_OUTPUT"}], origin={"kind":"unclassified"})
    append("assistant", [{"type":"text","text":"MAIN_HISTORY_REPORT"}])
    path.write_text("".join(json.dumps(row,ensure_ascii=False)+"\n" for row in rows))
    return session
