#!/usr/bin/env python3
"""Fixed streamed tool, callback, reconnect snapshot and native rewind targets."""

import os
import sys

from fake_codex import Server


class IdentityPeer(Server):
    def __init__(self):
        super().__init__()
        if self.mode == "entry-hidden-users":
            self.threads["thread-original"]["turns"][0]["items"][0:0] = [
                {"type": "userMessage", "id": "attachment", "content": [{"type": "image", "imageUrl": "fixture"}]},
                {"type": "userMessage", "id": "textless", "content": []},
                {"type": "userMessage", "id": "whitespace", "content": [{"type": "text", "text": "   "}]},
            ]

    def handle(self, request):
        if "method" in request:
            super().handle(request)

    def start_turn(self, request, params):
        if params["input"][0]["text"] == "native override":
            self.notify("codex/event/exec_command_end", {"threadId": params["threadId"], "turnId": "turn-3",
                "msg": {"call_id": "summary-file", "parsed_cmd": [{"type": "read", "path": "native-label.rs"}]}})
            self.reply(request, error="native override probe completed")
            return
        if params["input"][0]["text"] == "DISCONNECT":
            self.persist()
            os._exit(0)
        mode = self.mode
        self.mode = "summary-file"
        super().start_turn(request, params)
        self.mode = mode
        if params["input"][0]["text"] == "inspect ids":
            self.send({"id": 700, "method": "item/commandExecution/requestApproval", "params": {
                "threadId": params["threadId"], "turnId": "turn-3", "itemId": "summary-file",
                "command": "echo identity", "availableDecisions": ["accept", "decline"]}})


if __name__ == "__main__":
    if sys.argv[1:] == ["features", "list"]:
        print("step_model_switching experimental false", flush=True)
    else:
        IdentityPeer().run()
