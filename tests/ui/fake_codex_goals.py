"""Persist native goal facts and require explicit user API calls for every change."""

import json
import threading
import time


def new_goal(thread,objective,status="paused"):
    return {"threadId":thread,"objective":objective,"status":status,"tokenBudget":1000,
        "tokensUsed":0,"timeUsedSeconds":0,"createdAt":100,"updatedAt":101}


def save(server):
    path=server.root/"goal-state.json"
    temporary=path.with_suffix(".tmp")
    temporary.write_text(json.dumps(server.goal_state))
    temporary.replace(path)


def update(server,thread):
    goal=server.goal_state.get(thread)
    if goal is None:
        server.notify("thread/goal/cleared",{"threadId":thread})
    else:
        server.notify("thread/goal/updated",{"threadId":thread,"turnId":None,"goal":goal})


def setup(server):
    if hasattr(server,"goal_state"):
        return
    path=server.root/"goal-state.json"
    server.goal_state=json.loads(path.read_text()) if path.exists() else {"thread-original":
        None if server.mode.endswith("empty") else new_goal("thread-original","Fixture native objective")}
    if server.goal_state.get("thread-original"):
        server.goal_state["thread-original"]["tokensUsed"]=17
        server.goal_state["thread-original"]["timeUsedSeconds"]=3
    server.goal_pending=None

    def watch():
        previous=None
        while True:
            gate=server.root/"goal-step"
            value=gate.read_text() if gate.exists() else None
            if value and value!=previous:
                previous=value
                if value=="release-activate" and server.goal_pending:
                    request,goal=server.goal_pending
                    server.reply(request,{"goal":goal})
                    update(server,goal["threadId"])
                    server.goal_pending=None
                elif value in ("active","paused","blocked","usageLimited","budgetLimited","complete"):
                    thread="thread-original"
                    goal=server.goal_state.get(thread) or new_goal(thread,"External goal")
                    goal["status"]=value
                    server.goal_state[thread]=goal
                    save(server)
                    update(server,thread)
                elif value=="cleared":
                    server.goal_state["thread-original"]=None
                    save(server)
                    update(server,"thread-original")
                (server.root/"goal-applied").write_text(value)
            time.sleep(.01)
    threading.Thread(target=watch,daemon=True).start()


def handle(server,request):
    method=request["method"]
    if method not in ("thread/goal/get","thread/goal/set","thread/goal/clear"):
        return False
    params=request.get("params",{})
    thread=params["threadId"]
    if not server.mode.startswith("codex-goal"):
        if method=="thread/goal/get":
            server.reply(request,{"goal":None})
            return True
        return False
    setup(server)
    if server.mode.endswith("disabled"):
        server.send({"id":request["id"],"error":{"code":-32600,"message":"goals feature is disabled"}})
        return True
    if method=="thread/goal/get":
        server.reply(request,{"goal":server.goal_state.get(thread)})
    elif params.get("origin")!="user":
        server.send({"id":request["id"],"error":{"code":-32602,"message":"Expected explicit user origin"}})
    elif method=="thread/goal/clear":
        old=server.goal_state.pop(thread,None)
        save(server)
        server.reply(request,{"cleared":old is not None})
        if old:
            update(server,thread)
    elif server.mode.endswith("reject"):
        server.send({"id":request["id"],"error":{"code":-32600,"message":"Managed goal policy rejected this objective"}})
    else:
        goal=server.goal_state.get(thread) or new_goal(thread,params.get("objective",""),"active")
        for key in ["objective","status","tokenBudget"]:
            if params.get(key) is not None:
                goal[key]=params[key]
        server.goal_state[thread]=goal
        save(server)
        if server.mode.endswith("delayed") and goal["status"]=="active":
            server.goal_pending=request,goal
        else:
            server.reply(request,{"goal":goal})
            update(server,thread)
    return True
