"""Current Codex metadata, item pages and terminal pipe failures without a model."""

import copy
import json
import os


def handle(server, request):
    method, params = request["method"], request.get("params", {})
    if method in ("thread/read", "thread/resume"):
        thread = copy.deepcopy(server.threads[params["threadId"]])
        if method == "thread/resume" or not params.get("includeTurns"):
            thread["turns"] = []
        thread["historyMode"] = "paginated"
        server.reply(request, {"thread":thread})
    elif method == "thread/turns/list":
        if server.mode == "codex-history-error":
            server.send({"id":request["id"],"error":{"code":-32000,"data":"irrelevant-large-error-data"*10000,"message":"Fixture history denied: " + "日本語\n"*20000}})
            return True
        turns = copy.deepcopy(server.threads[params["threadId"]]["turns"])
        for turn in turns:
            if server.mode == "codex-current-full":
                turn["itemsView"] = "full"
            else:
                turn["items"], turn["itemsView"] = [], "notLoaded"
        server.reply(request, {"data":turns,"nextCursor":None})
    elif method == "thread/items/list":
        turns = server.threads[params["threadId"]]["turns"]
        items = next(turn["items"] for turn in turns if turn["id"] == params["turnId"])
        entries = [{"turnId":params["turnId"],"item":item} for item in items]
        if not params.get("cursor"):
            server.reply(request,{"data":entries[:1],"nextCursor":"items-page-2"})
        else:
            server.reply(request,{"data":entries,"nextCursor":None})
    elif method == "turn/start" and server.mode == "codex-pipe-eof":
        os._exit(0)
    elif method == "turn/start" and server.mode == "codex-pipe-oversize":
        print(json.dumps({"method":"future/output","params":{"text":"x" * (8*1024*1024)}}),flush=True)
        os._exit(0)
    else:
        return False
    return True
