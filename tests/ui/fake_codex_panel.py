"""Expose native-shaped descendant/process reads and exact controls to terminal tests."""

import threading
import time


def agent(identity,parent,nickname):
    return {"id":identity,"parentThreadId":parent,"agentNickname":nickname,"agentRole":"worker",
        "status":{"type":"active","activeFlags":[]},"canAcceptDirectInput":False,
        "source":{"subAgent":{"thread_spawn":{"parent_thread_id":parent,"depth":1}}}}


def setup(server):
    if hasattr(server,"panel_agents"):
        return
    server.panel_agents=[agent("child-a","thread-original","Alpha 日本語"),
        agent("child-b","thread-original","Beta worker"),agent("grand-child","child-a","Grandchild")]
    server.panel_processes={"thread-original":[process("101","root-process","ROOT_BACKGROUND_COMMAND")],
        "child-a":[process("201","child-process","CHILD_BACKGROUND_COMMAND")]}
    server.panel_bootstrapped=False

    def watch():
        previous=None
        while True:
            gate=server.root/"panel-step"
            value=gate.read_text() if gate.exists() else None
            if value and value!=previous:
                previous=value
                if value=="remove-alpha":
                    server.panel_agents=[row for row in server.panel_agents if row["id"] not in ("child-a","grand-child")]
                elif value=="finish-all":
                    for row in server.panel_agents:
                        row["status"]={"type":"idle"}
                        server.notify("thread/status/changed",{"threadId":row["id"],"status":row["status"]})
                    server.panel_processes={}
                    for owner,item in (("thread-original","root-process"),("child-a","child-process")):
                        server.notify("item/completed",{"threadId":owner,"turnId":"parent-done","item":{
                            "id":item,"type":"commandExecution","processId":"101" if owner=="thread-original" else "201",
                            "command":"FINISHED_BACKGROUND","status":"completed","aggregatedOutput":"","exitCode":0}})
                elif value=="child-question":
                    server.send({"id":"child-question-id","method":"item/tool/requestUserInput","params":{
                        "threadId":"child-a","turnId":"child-turn","itemId":"child-ask","isBlocking":False,
                        "questions":[{"id":"child-choice","header":"Child decision","question":"Choose for child Alpha",
                            "isOther":True,"options":[{"label":"Yes","description":"Continue child"},{"label":"No","description":"Stop child"}]}]}})
                    server.notify("turn/completed",{"threadId":"thread-original","turn":{"id":"parent-done","status":"completed"}})
                    server.notify("item/completed",{"threadId":"child-a","turnId":"child-turn","item":{
                        "id":"late-child","type":"agentMessage","text":"LATE_CHILD_HIDDEN"}})
                (server.root/"panel-applied").write_text(value)
            time.sleep(.01)
    threading.Thread(target=watch,daemon=True).start()


def process(identity,item,command):
    return {"processId":identity,"itemId":item,"command":command,"cwd":"/tmp/fixture",
        "osPid":None,"cpuPercent":None,"rssKb":None}


def handle(server,request):
    if not server.mode.startswith("codex-panel"):
        return False
    setup(server)
    method,params=request["method"],request.get("params",{})
    thread=params.get("threadId")
    if method=="thread/list" and "ancestorThreadId" in params:
        offset=int(params.get("cursor") or 0)
        server.reply(request,{"data":server.panel_agents[offset:offset+1],
            "nextCursor":str(offset+1) if offset+1<len(server.panel_agents) else None})
        if not server.panel_bootstrapped:
            server.panel_bootstrapped=True
            server.notify("item/completed",{"threadId":"thread-original","turnId":"parent-done","item":{
                "id":"main-tool","type":"commandExecution","command":"MAIN_TOOL_VISIBLE","cwd":"/tmp",
                "commandActions":[],"status":"completed","aggregatedOutput":"MAIN_TOOL_OUTPUT","exitCode":0}})
            server.notify("item/completed",{"threadId":"thread-original","turnId":"parent-done","item":{
                "id":"root-wait","type":"collabAgentToolCall","tool":"wait","status":"completed",
                "senderThreadId":"thread-original","receiverThreadIds":["child-a"],
                "agentsStates":{"child-a":{"status":"completed","message":"EMBEDDED_CHILD_REPORT_HIDDEN"}}}})
    elif method=="thread/read" and thread in [row["id"] for row in server.panel_agents]:
        server.reply(request,{"thread":next(row for row in server.panel_agents if row["id"]==thread)})
    elif method=="thread/turns/list" and thread not in server.threads:
        row=next((row for row in server.panel_agents if row["id"]==thread),None)
        server.reply(request,{"data":[{"id":"child-turn","status":"inProgress" if row and row["status"]["type"]=="active" else "completed","items":[],"itemsView":"notLoaded"}],"nextCursor":None})
    elif method=="thread/items/list" and thread not in server.threads:
        data=[{"turnId":"child-turn","item":{"id":"child-text","type":"agentMessage","text":"CHILD_DETAIL_ONLY 日本語"}},
            {"turnId":"child-turn","item":{"id":"child-command","type":"commandExecution","command":"CHILD_TOOL_ONLY",
                "aggregatedOutput":"CHILD_TOOL_RESULT_ONLY","status":"completed"}}]
        offset=int(params.get("cursor") or 0)
        server.reply(request,{"data":data[offset:offset+1],"nextCursor":str(offset+1) if offset+1<len(data) else None})
    elif method=="thread/backgroundTerminals/list":
        offset=int(params.get("cursor") or 0)
        data=server.panel_processes.get(thread,[])
        server.reply(request,{"data":data[offset:offset+1],"nextCursor":str(offset+1) if offset+1<len(data) else None})
    elif method=="thread/backgroundTerminals/terminate":
        if server.mode.endswith("stop-error"):
            server.send({"id":request["id"],"error":{"code":-32000,"message":"Selected fixture stop denied"}})
        else:
            rows=server.panel_processes.get(thread,[])
            old=len(rows)
            server.panel_processes[thread]=[row for row in rows if row["processId"]!=params["processId"]]
            server.reply(request,{"terminated":len(server.panel_processes[thread])!=old})
    elif method=="turn/interrupt" and thread not in server.threads:
        row=next(row for row in server.panel_agents if row["id"]==thread)
        row["status"]={"type":"idle"}
        server.notify("turn/completed",{"threadId":thread,"turn":{"id":params["turnId"],"status":"interrupted"}})
        server.notify("thread/status/changed",{"threadId":thread,"status":row["status"]})
        server.reply(request,{})
    else:
        return False
    return True
