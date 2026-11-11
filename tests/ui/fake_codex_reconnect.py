"""Force reconnect races through real pipes without native inference or user sessions."""

import copy
import json
import os
import time


def handle(server,request):
    if not server.mode.startswith("codex-reconnect"): return False
    method=request.get("method")
    params=request.get("params",{})
    if method is None:
        if request.get("id")==9 and hasattr(server,"waiting_initialize"):
            server.reply(server.waiting_initialize,{})
            del server.waiting_initialize
        return True
    if method=="initialize":
        path=server.root/"connections"
        server.connection=int(path.read_text())+1 if path.exists() else 1
        path.write_text(str(server.connection))
        (server.root/f"connection-pid-{server.connection}").write_text(str(os.getpid()))
        if server.connection>1:
            server.waiting_initialize=request
            server.send({"id":9,"method":"mcpServer/elicitation/request","params":{
                "threadId":"thread-original","serverName":"reconnect-fixture","mode":"form",
                "message":"Reconnect question","requestedSchema":{"type":"object","properties":{
                    "count":{"type":"integer","title":"Count","minimum":1,"maximum":9}},"required":["count"]}}})
        else: server.reply(request,{})
        return True
    if method=="model/list":
        names=["fixture-b","fixture-a"] if server.connection>1 else ["fixture-a","fixture-b"]
        page=int(params.get("cursor") or 0)
        name=names[page]
        server.reply(request,{"data":[{"id":name,"model":name,"displayName":name,"isDefault":page==0,
            "defaultReasoningEffort":"low","supportedReasoningEfforts":[{"reasoningEffort":"low","description":"Low"}]}],
            "nextCursor":"1" if page==0 else None})
        return True
    if method=="turn/start" and any(part.get("text")=="DISCONNECT" for part in params.get("input",[])):
        server.persist()
        os._exit(0)
    if server.connection==1: return False
    if method=="thread/resume":
        server.selected=params["threadId"]
        thread=server.threads[server.selected]
        if not any(turn["id"]=="reconnect-running" for turn in thread["turns"]):
            thread["turns"].append({"id":"reconnect-running","status":"inProgress","items":[
                {"id":"same-reconnect","type":"agentMessage","text":"prefix"}]})
        server.reply(request,{"thread":{**thread,"turns":[],"historyMode":"paginated"},"model":"fixture-a","reasoningEffort":"low"})
    elif method=="thread/turns/list":
        deadline=time.monotonic()+10
        while not (server.root/"release-history").exists() and time.monotonic()<deadline: time.sleep(.01)
        if (server.root/"fail-history").exists(): server.reply(request,error="Isolated history failure")
        else:
            turns=copy.deepcopy(server.threads[params["threadId"]]["turns"])
            for turn in turns: turn["items"],turn["itemsView"]=[],"notLoaded"
            server.reply(request,{"data":turns,"nextCursor":None})
    elif method=="thread/items/list":
        turn=next(turn for turn in server.threads[params["threadId"]]["turns"] if turn["id"]==params["turnId"])
        old=copy.deepcopy(turn["items"])
        if turn["id"]=="reconnect-running":
            scope={"threadId":params["threadId"],"turnId":turn["id"]}
            server.notify("item/agentMessage/delta",{**scope,"itemId":"same-reconnect","delta":" suffix"})
            turn["items"][0]["text"]="prefix suffix FINAL"
            turn["status"]="completed"
            server.notify("item/completed",{**scope,"item":turn["items"][0]})
            server.notify("turn/completed",{"threadId":params["threadId"],"turn":turn})
        server.reply(request,{"data":[{"turnId":turn["id"],"item":item} for item in old],"nextCursor":None})
    elif method=="thread/read":
        thread=server.threads[params["threadId"]]
        server.notify("thread/settings/updated",{"threadId":thread["id"],"threadSettings":{"model":"fixture-a","effort":"low"}})
        server.reply(request,{"thread":{**thread,"turns":[],"status":{"type":"idle"},"model":"fixture-b","reasoningEffort":"low"}})
    elif method=="thread/queue/list":
        server.reply(request,{"data":[{"id":"queued-native","clientUserMessageId":"queued-client","input":[{"type":"text","text":"PERSISTED_QUEUE"}]}],"nextCursor":None})
    elif method=="thread/goal/get":
        server.send({"id":request["id"],"error":{"code":-32600,"message":"goals feature is disabled"}})
    elif method=="thread/list": server.reply(request,{"data":[],"nextCursor":None})
    elif method=="thread/backgroundTerminals/list": server.reply(request,{"data":[],"nextCursor":None})
    else: return False
    return True
