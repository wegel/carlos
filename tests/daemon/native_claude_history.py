"""Create read-only SDK history fixtures and inspect the helper's actual page requests."""

import json
import re
import uuid
from pathlib import Path


class NativeClaudeHistory:
    def __init__(self, fixture, count=150, hidden=(), boundary_tool=False, sources=None):
        self.fixture = fixture
        self.session = str(uuid.uuid4())
        self.rows = []
        for index in range(count):
            for role in ("user", "assistant"):
                text = f"native Claude {role} {index} 🌍"
                content = [{"type":"text", "text":text}] if role == "assistant" else text
                self.rows.append({"type":role, "uuid":str(uuid.uuid4()),
                    "parentUuid":self.rows[-1]["uuid"] if self.rows else None,
                    "cwd":str(fixture.root), "sessionId":self.session, "entrypoint":"sdk-ts",
                    "version":"2.1.322", "timestamp":"2026-11-16T00:00:00.000Z",
                    "origin":{"kind":"human"}, "message":{"role":role, "content":content}})
        for index in hidden:
            self.rows[index].update(agent_id="private-child", parent_tool_use_id="child-call")
        for index, source in (sources or {}).items():
            self.rows[index*2]["origin"] = {"kind":source}
        if boundary_tool:
            self.rows[-65]["message"]["content"].append({"type":"tool_use", "id":"across-window",
                "name":"Read", "input":{"file_path":"src/é.rs"}})
            self.rows[-64]["message"]["content"] = [
                {"type":"text", "text":f"native Claude user {count-32} 🌍"},
                {"type":"tool_result", "tool_use_id":"across-window", "content":"exact native cross-window source 🌍"}]
        folder = Path(fixture.env["CLAUDE_CONFIG_DIR"]) / "projects" / re.sub(r"[^a-zA-Z0-9]", "-", str(fixture.root))
        folder.mkdir(parents=True)
        self.path = folder / (self.session + ".jsonl")
        self.original = "".join(json.dumps(row, ensure_ascii=False) + "\n" for row in self.rows)
        self.path.write_text(self.original)

    def requests(self):
        trace = self.fixture.root / "claude-controller.jsonl"
        return [row["message"] for line in trace.read_text().splitlines()
                if (row := json.loads(line))["direction"] == "client"
                and row["message"].get("method") == "sessions.read"] if trace.exists() else []

    def expected_text(self):
        expected = []
        for row in self.rows:
            if row.get("agent_id"):
                continue
            content = row["message"]["content"]
            expected.append(content if isinstance(content, str) else next(part["text"] for part in content if part["type"] == "text"))
        return expected
