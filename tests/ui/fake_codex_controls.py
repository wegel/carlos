"""Persist native queues and publish live settings without treating controls as prompts."""

import json
import threading
import time


def setup(server):
    if hasattr(server,"control_queues"):
        return
    path=server.root/"queue-state.json"
    server.control_queues=json.loads(path.read_text()) if path.exists() else {}
    server.control_serial=100
    server.control_active=None
    if server.mode.startswith("codex-controls"):
        server.control_queues.setdefault("thread-original",[
            entry("queue-a","queued Alpha"),entry("queue-b","queued Beta")])
        if "busy" in server.mode:
            server.control_active="control-running"
            server.notify("turn/started",{"threadId":"thread-original","turn":{"id":server.control_active,"status":"inProgress"}})

        def watch():
            previous=None
            while True:
                gate=server.root/"controls-step"
                value=gate.read_text() if gate.exists() else None
                if value and value!=previous:
                    previous=value
                    if value=="external-edit":
                        queue=server.control_queues["thread-original"]
                        queue[0]["input"][0]["text"]="Changed by another client"
                        changed(server,"thread-original")
                    elif value=="external-remove":
                        server.control_queues["thread-original"].pop(0)
                        changed(server,"thread-original")
                time.sleep(.01)
        threading.Thread(target=watch,daemon=True).start()


def entry(identity,text,client=None):
    return {"id":identity,"clientUserMessageId":client or "client-"+identity,
        "input":[{"type":"text","text":text,"text_elements":[]}]}


def changed(server,thread):
    path=server.root/"queue-state.json"
    temporary=path.with_suffix(".tmp")
    temporary.write_text(json.dumps(server.control_queues))
    temporary.replace(path)
    server.notify("thread/queue/changed",{"threadId":thread})


def consume(server,thread,item,answer_text=None):
    server.control_serial+=1
    turn="queued-turn-"+str(server.control_serial)
    user={"id":turn+"-user","clientId":item["clientUserMessageId"],"type":"userMessage","content":item["input"]}
    text=item["input"][0].get("text","attachment")
    assistant={"id":turn+"-answer","type":"agentMessage","phase":"final_answer","text":answer_text or "Consumed: "+text}
    native={"id":turn,"status":"completed","items":[user,assistant]}
    server.threads[thread]["turns"].append(native)
    server.notify("turn/started",{"threadId":thread,"turn":{"id":turn,"status":"inProgress"}})
    for message in native["items"]:
        server.notify("item/completed",{"threadId":thread,"turnId":turn,"item":message})
    server.notify("turn/completed",{"threadId":thread,"turn":native})
    changed(server,thread)


def handle(server,request):
    method,params=request["method"],request.get("params",{})
    controls=server.mode.startswith("codex-controls")
    if not controls and method in ("thread/settings/update","turn/settings/update"):
        server.send({"id":request["id"],"error":{"code":-32601,"message":"Unknown method: "+method}})
        return True
    if not (method.startswith("thread/queue/") or controls and method in (
        "model/list","thread/settings/update","turn/settings/update","turn/interrupt","turn/steer")
        or method=="turn/start" and server.mode=="codex-controls-ralph"):
        return False
    setup(server)
    thread=params.get("threadId","thread-original")
    queue=server.control_queues.setdefault(thread,[])
    if method=="turn/start":
        server.reply(request,{"turn":{"id":"automatic-native-turn","status":"inProgress"}})
        consume(server,thread,{"clientUserMessageId":"automatic-fixture","input":params["input"]},answer_text="@@COMPLETE@@")
        return True
    if method=="model/list":
        server.reply(request,{"data":[{"model":"fixture-a","displayName":"Fixture A","isDefault":True,
            "defaultReasoningEffort":"low","supportedReasoningEfforts":[{"reasoningEffort":"low"},{"reasoningEffort":"high"}]},
            {"model":"fixture-b","displayName":"Fixture B","isDefault":False,
            "defaultReasoningEffort":"low","supportedReasoningEfforts":[{"reasoningEffort":"low"}]}],"nextCursor":None})
    elif method=="thread/queue/list":
        if server.mode=="codex-controls-unsupported":
            server.send({"id":request["id"],"error":{"code":-32601,"message":"Unknown method: thread/queue/list"}})
        else:
            offset=int(params.get("cursor") or 0)
            data=queue[offset:offset+1]
            server.reply(request,{"data":data,"nextCursor":str(offset+1) if offset+1<len(queue) else None})
    elif method=="thread/queue/add":
        text=params["input"][0]["text"]
        if text=="REJECT_QUEUE":
            server.reply(request,error="Queue rejected the isolated fixture input")
        else:
            server.control_serial+=1
            item=entry("native-queue-"+str(server.control_serial),text,params["clientUserMessageId"])
            queue.append(item)
            changed(server,thread)
            server.reply(request,{"queuedSubmission":item})
    elif method=="thread/queue/update":
        item=next((item for item in queue if item["id"]==params["queuedSubmissionId"]),None)
        if item is None:
            server.reply(request,error="Queued message disappeared")
        else:
            item["input"]=params["input"]
            changed(server,thread)
            server.reply(request,{"queuedSubmission":item})
    elif method=="thread/queue/delete":
        item=next((item for item in queue if item["id"]==params["queuedSubmissionId"]),None)
        if item:
            queue.remove(item)
            changed(server,thread)
        server.reply(request,{"deleted":item is not None})
    elif method=="thread/queue/reorder":
        ids=params["queuedSubmissionIds"]
        if set(ids)!=set(item["id"] for item in queue):
            server.reply(request,error="Queue changed; reorder requires every current ID")
        else:
            queue[:]=[next(item for item in queue if item["id"]==key) for key in ids]
            changed(server,thread)
            server.reply(request,{})
    elif method=="thread/queue/start":
        if server.control_active:
            server.reply(request,error="Thread already has an active or pending turn")
        else:
            item=next(item for item in queue if item["id"]==params["queuedSubmissionId"])
            queue.remove(item)
            server.reply(request,{"turn":{"id":"native-queued-start","status":"inProgress","items":[]}})
            consume(server,thread,item)
    elif method=="turn/interrupt":
        server.reply(request,{})
        server.notify("turn/completed",{"threadId":thread,"turn":{"id":params["turnId"],"status":"interrupted"}})
        server.control_active=None
    elif method=="turn/steer":
        server.reply(request,{"turnId":params["expectedTurnId"]})
        server.notify("item/completed",{"threadId":thread,"turnId":params["expectedTurnId"],
            "item":{"id":"native-steer","type":"userMessage","content":params["input"]}})
    elif method=="thread/settings/update":
        if "settings-rejected" in server.mode and params.get("model")=="fixture-b":
            server.send({"id":request["id"],"error":{"code":-32600,"message":"Managed fixture policy rejected this model","data":{"policy":"managed-fixture"}}})
            return True
        if "settings-unsupported" in server.mode:
            server.send({"id":request["id"],"error":{"code":-32601,"message":"Unknown method: thread/settings/update"}})
            return True
        server.notify("thread/settings/updated",{"threadId":thread,"threadSettings":{
            key:value for key,value in params.items() if key!="threadId"}})
        server.reply(request,{})
    elif method=="turn/settings/update":
        if "partial" in server.mode:
            server.reply(request,error="Active settings fixture failed")
        else:
            server.reply(request,{"status":"applied"})
    return True
