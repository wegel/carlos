"""Write only owned, deterministic legacy/paginated sessions for native read tests."""

import json
from pathlib import Path
import uuid


def write_history(root, count=105, large=False, compact=False, provider="openai", commands_per_turn=0, large_indices=(1,)):
    thread, window = str(uuid.uuid4()), str(uuid.uuid4())
    stamp = "2026-11-11T00:00:00Z"
    rows = [{"type":"session_meta","payload":{
        "id":thread,"session_id":thread,"timestamp":stamp,"cwd":str(root),
        "originator":"carlos-contract-test","cli_version":"0.167.1","source":"vscode",
        "model_provider":provider,"history_mode":"paginated","multi_agent_version":"v2",
        "context_window":{"window_id":window}}}]
    turns = []
    for index in range(count):
        turn = str(uuid.uuid4())
        turns.append(turn)
        rows.append({"type":"event_msg","payload":{"type":"task_started","turn_id":turn,"root_turn_id":turn,
            "turn_attribution":{"turn_id":turn,"root_turn_id":turn,"turn_trigger":None,"parent_turn_id":None,"initiating_agent_path":None}}})
        rows.append({"type":"turn_context","payload":{"turn_id":turn,"root_turn_id":turn,
            "cwd":str(root),"approval_policy":"never","sandbox_policy":{"type":"read-only"},
            "model":"gpt-6.1-sol","effort":"low","summary":"none"}})
        for role, text in [("user",f"history human {index}"),("assistant",f"HISTORY_REPLY:{index}")]:
            if role == "assistant":
                for command in range(commands_per_turn):
                    rows.append({"type":"event_msg","payload":{"type":"item_completed","thread_id":thread,
                        "turn_id":turn,"item":{"type":"Extension","kind":"clock.sleep","id":str(uuid.uuid4()),"durationMs":1}}})
            if large and index in large_indices and role == "assistant":
                text += " x" * (3 * 1024 * 1024)
            item_id = str(uuid.uuid4())
            response_id = "msg_" + str(uuid.uuid4())
            content = [{"type":"input_text" if role == "user" else "output_text","text":text}]
            response = {"type":"message","id":response_id,"role":role,"content":content,
                "internal_chat_message_metadata_passthrough":{"turn_id":turn}}
            if role == "assistant":
                response["phase"] = "final_answer"
            rows.append({"type":"response_item","payload":response,"metadata":{
                "retained_source":{"id":{"message_id":response_id,"turn_id":turn,"role":role},
                    "revision":"retained_" + str(uuid.uuid4()),"complete":True}}})
            item = {"type":"UserMessage" if role == "user" else "AgentMessage","id":item_id,
                "content":[{"type":"text" if role == "user" else "Text","text":text}]}
            if role == "assistant":
                item["phase"] = "final_answer"
            rows.append({"type":"event_msg","payload":{"type":"item_completed","thread_id":thread,
                "turn_id":turn,"item":item}})
        rows.append({"type":"event_msg","payload":{"type":"task_complete","turn_id":turn,
            "root_turn_id":turn,"last_agent_message":f"HISTORY_REPLY:{index}"}})
        if compact and index == count // 2:
            compact_turn = str(uuid.uuid4())
            turns.append(compact_turn)
            rows.extend([
                {"type":"event_msg","payload":{"type":"task_started","turn_id":compact_turn}},
                {"type":"compacted","payload":{"message":"LOCAL_HISTORY_COMPACTION","replacement_history":[]}},
                {"type":"event_msg","payload":{"type":"item_completed","thread_id":thread,"turn_id":compact_turn,
                    "item":{"type":"ContextCompaction","id":str(uuid.uuid4())}}},
                {"type":"event_msg","payload":{"type":"task_complete","turn_id":compact_turn}},
            ])
    path = root / "codex-home/sessions" / ("rollout-2026-11-11T00-00-00-" + thread + ".jsonl")
    path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text("".join(json.dumps({"timestamp":stamp,"ordinal":index,**row}) + "\n" for index,row in enumerate(rows)))
    return thread, turns, path


def write_legacy(root, provider="openai"):
    thread = str(uuid.uuid4())
    stamp = "2026-09-01T00:00:00Z"
    header = {"timestamp":stamp,"type":"session_meta","payload":{
        "id":thread,"timestamp":stamp,"cwd":str(root),"originator":"carlos-contract-test",
        "cli_version":"0.167.1","source":"cli","model_provider":provider}}
    fixture = Path(__file__).resolve().parents[2] / "src/tests/fixtures/stale_transcript_replay.jsonl"
    path = root / "codex-home/sessions" / ("rollout-2026-09-01T00-00-00-" + thread + ".jsonl")
    path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text(json.dumps(header) + "\n" + fixture.read_text())
    return thread, path
