"""Persist isolated native-format sessions so the official SDK owns all test reads and forks."""

import json
import os
import re
from pathlib import Path
import uuid


class Sessions:
    def __init__(self, argv, cwd):
        self.cwd = str(cwd)
        self.session_id = self.argument(argv, "--resume") or self.argument(argv, "--session-id") or str(uuid.uuid4())
        folder = Path(os.environ["CLAUDE_CONFIG_DIR"]) / "projects" / re.sub(r"[^a-zA-Z0-9]", "-", self.cwd)
        folder.mkdir(parents=True, exist_ok=True)
        self.path = folder / (self.session_id + ".jsonl")
        self.parent = None
        self.prompts = {}
        if self.path.exists():
            rows = [json.loads(line) for line in self.path.read_text().splitlines() if line.strip()]
            self.parent = next((row["uuid"] for row in reversed(rows) if row.get("uuid")), None)

    @staticmethod
    def argument(argv, key):
        for index, value in enumerate(argv):
            if value == key and index + 1 < len(argv):
                return argv[index + 1]
            if value.startswith(key + "="):
                return value[len(key) + 1:]
        return None

    def append(self, row):
        row = {"cwd": self.cwd, "sessionId": self.session_id, "entrypoint": "sdk-ts", "version": "2.1.322",
               "timestamp": "2026-11-10T00:00:00.000Z", "parentUuid": self.parent, **row}
        with self.path.open("a") as output:
            output.write(json.dumps(row, ensure_ascii=False) + "\n")
        self.parent = row.get("uuid", self.parent)

    def prompt(self, message):
        self.prompts[message["uuid"]] = message["message"]["content"]
        self.append({"type": "user", "uuid": message["uuid"], "message": message["message"],
                     **({"origin": message["origin"]} if message.get("origin", {}).get("kind") == "human" else {})})

    def result(self, prompt):
        text = self.prompts.get(prompt, "")
        self.append({"type": "assistant", "uuid": str(uuid.uuid4()), "message": {"id": "message-" + prompt,
                     "role": "assistant", "content": [{"type": "text", "text": "Answer: " + text}]}})
