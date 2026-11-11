"""Keep multiple human inputs in one native turn so rewind must check item identity."""


def shared_history():
    from fake_codex import completed_turn, user_item
    first = completed_turn("shared-turn", "same prompt 日本語")
    first["items"].insert(1, {"type":"commandExecution","id":"before-steer-tool",
        "command":"echo before steer","aggregatedOutput":"BEFORE_STEER_TOOL","exitCode":0})
    first["items"] += [user_item("shared-steer", "same prompt 日本語"),
        {"type":"agentMessage","id":"shared-after","text":"AFTER_STEER_REPORT","phase":"final_answer"}]
    return {"id":"thread-original","turns":[first,completed_turn("later-turn","later prompt")]}


def list_items(server, request, params):
    if server.mode == "legacy":
        server.send({"id":request["id"],"error":{"code":-32601,"message":"Unknown method: thread/items/list"}})
        return
    turns=server.threads[params["threadId"]]["turns"]
    entries=[{"turnId":turn["id"],"item":item} for turn in turns if turn["id"]==params["turnId"] for item in turn["items"]]
    # The selected steer arrives after a page break and an overlapping first user.
    if server.mode == "codex-shared-turn" and params["turnId"]=="shared-turn" and not params.get("cursor"):
        server.reply(request,{"data":entries[:2],"nextCursor":"user-page-2"})
    else:
        server.reply(request,{"data":entries,"nextCursor":None})
