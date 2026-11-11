#!/usr/bin/env python3
"""Offer one local MCP form; never contact an account or execute user input."""

import json
import os
from pathlib import Path
import sys


root=Path(os.environ["CARLOS_TEST_ROOT"])
call_id=None


def send(frame):
    frame={"jsonrpc":"2.0",**frame}
    print(json.dumps(frame),flush=True)
    with (root/"mcp-fixture.jsonl").open("a") as log:
        log.write(json.dumps({"direction":"server","message":frame})+"\n")


for line in sys.stdin:
    request=json.loads(line)
    with (root/"mcp-fixture.jsonl").open("a") as log:
        log.write(json.dumps({"direction":"client","message":request})+"\n")
    method=request.get("method")
    if method=="initialize":
        send({"id":request["id"],"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{}},
            "serverInfo":{"name":"carlos-local-form","version":"1"}}})
    elif method=="tools/list":
        send({"id":request["id"],"result":{"tools":[{"name":"ask","description":"Ask the isolated native form",
            "inputSchema":{"type":"object","properties":{}},"annotations":{"readOnlyHint":True,"destructiveHint":False,"openWorldHint":False}}]}})
    elif method=="tools/call":
        call_id=request["id"]
        send({"id":"fixture-form","method":"elicitation/create","params":{"mode":"form","message":"Native MCP form",
            "requestedSchema":{"type":"object","required":["age","choice","enabled"],"properties":{
                "age":{"type":"integer","minimum":18,"maximum":65,"default":24},
                "choice":{"type":"string","oneOf":[{"const":"jp","title":"日本語"},{"const":"fr","title":"Français"}],"default":"jp"},
                "enabled":{"type":"boolean","default":False},"optional":{"type":"string"}}}}})
    elif method=="ping":
        send({"id":request["id"],"result":{}})
    elif method is None and request.get("id")=="fixture-form":
        result=request.get("result",{})
        marker="MCP_FORM_ACCEPTED" if result.get("action")=="accept" else "MCP_FORM_DECLINED"
        send({"id":call_id,"result":{"content":[{"type":"text","text":marker}],"isError":False}})
    elif "id" in request:
        send({"id":request["id"],"error":{"code":-32601,"message":"Unknown fixture method"}})
