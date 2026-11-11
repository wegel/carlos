"""Send native Codex callbacks on explicit fixture gates, never through model prompts."""

import json
import threading
import time


def question(request_id, thread="child-a", blocking=True, secret=False, options=None):
    return {"id":request_id,"method":"item/tool/requestUserInput","params":{
        "threadId":thread,"turnId":"child-turn","itemId":"question-tool","isBlocking":blocking,
        "questions":[{"id":"native-answer-key","header":"Secret answer" if secret else "Choose a route",
            "question":"Native question text, not the answer key","isSecret":secret,"isOther":True,
            "options":options}]}}


def form(request_id="form", schema=None, mode="form"):
    return {"id":request_id,"method":"mcpServer/elicitation/request","params":{
        "threadId":"child-form","turnId":None,"serverName":"forms.test","message":"Native form",
        "mode":mode,"requestedSchema":schema or {"type":"object","required":["age","mode","notify"],"properties":{
            "age":{"type":"integer","minimum":18,"maximum":65,"default":24},
            "mode":{"type":"string","default":"jp","oneOf":[{"const":"jp","title":"日本語"},{"const":"fr","title":"Français"}]},
            "notify":{"type":"boolean","default":False},"optional":{"type":"string","default":None}}}}}


def bootstrap(server):
    if getattr(server,"question_watcher",False):
        return
    server.question_watcher=True
    server.native_callbacks={}

    def watch():
        previous=None
        while True:
            path=server.root/"callback-step"
            current=path.read_text() if path.exists() else None
            if current and current!=previous:
                previous=current
                dispatch(server,current)
            time.sleep(.01)
    threading.Thread(target=watch,daemon=True).start()


def emit(server, request):
    server.native_callbacks[json.dumps(request["id"])]=request
    server.send(request)


def dispatch(server,step):
    if step=="questions":
        server.notify("turn/started",{"threadId":"thread-original","turn":{"id":"root-working","status":"inProgress"}})
        first=question(9,options=[{"label":"Red","description":"The red route"},{"label":"Blue","description":"The blue route"}])
        first["params"]["questions"].append({"id":"unicode-key","header":"Unicode text","question":"Type your note","options":None})
        emit(server,first)
        emit(server,question("9",thread="child-b",secret=True))
        server.notify("item/completed",{"threadId":"child-a","turnId":"child-turn","item":{"type":"agentMessage","id":"child-output","text":"PRIVATE_CHILD_TEXT"}})
        server.notify("item/completed",{"threadId":"child-a","turnId":"child-turn","item":{"type":"commandExecution","id":"child-tool","command":"PRIVATE_CHILD_TOOL","aggregatedOutput":"PRIVATE_CHILD_OUTPUT","exitCode":0}})
        server.notify("turn/completed",{"threadId":"thread-original","turn":{"id":"root-working","status":"completed"}})
    elif step=="nonblocking":
        server.notify("turn/started",{"threadId":"thread-original","turn":{"id":"root-busy","status":"inProgress"}})
        emit(server,question("nonblocking",blocking=False))
    elif step=="secret-nonblocking":
        request=question("secret-nonblocking",blocking=False,secret=True)
        request["params"]["questions"].append({"id":"last","header":"Public note","question":"Optional note","options":None})
        emit(server,request)
    elif step=="quiet":
        server.notify("turn/completed",{"threadId":"thread-original","turn":{"id":"root-busy","status":"completed"}})
    elif step=="old-replay":
        emit(server,question("nonblocking",blocking=False))
        server.notify("carlos/codex/formChecked",{"requestId":"nonblocking","revision":1,"ok":True})
        server.notify("serverRequest/resolved",{"threadId":"child-a","requestId":"nonblocking"})
    elif step=="approvals":
        common={"threadId":"child-approvals","turnId":"child-turn","itemId":"native-item"}
        emit(server,{"id":"command","method":"item/commandExecution/requestApproval","params":{
            **common,"command":"echo command-checkpoint","availableDecisions":["accept","decline","cancel"]}})
        emit(server,{"id":"permissions","method":"item/permissions/requestApproval","params":{
            **common,"permissions":{"network":{"enabled":True}},"reason":"Allow this network request"}})
        emit(server,{"id":"legacy-patch","method":"applyPatchApproval","params":{
            "conversationId":"child-legacy","callId":"patch-call","fileChanges":{"note.txt":{"type":"add","content":"visible patch content"}}}})
        emit(server,{"id":"file","method":"item/fileChange/requestApproval","params":{
            **common,"reason":"Update the generated file","grantRoot":"/tmp/owned-fixture"}})
    elif step=="blocking":
        emit(server,{"id":"human-command","method":"item/commandExecution/requestApproval","params":{
            "threadId":"child-human","turnId":"child-turn","itemId":"checkpoint","kind":"humanCommand",
            "command":None,"reason":"Approve this human checkpoint once","availableDecisions":["accept","acceptForSession","decline","cancel"]}})
    elif step=="form":
        emit(server,form())
    elif step=="bad-form":
        emit(server,form("bad",{"type":"object","required":["missing"],"properties":{"x":{"type":"string","unknownConstraint":True}}}))
    elif step=="url":
        request=form("url",mode="url")
        request["params"].update(url="https://login.example.test/device",elicitationId="browser-id")
        request["params"].pop("requestedSchema")
        emit(server,request)
    elif step=="verification":
        request=form("verify",mode="openai/userVerification")
        request["params"].update(challenge="unsupported-proof",title="Verify",description="Device verification")
        emit(server,request)
    elif step=="resolve-numeric":
        server.notify("serverRequest/resolved",{"threadId":"child-a","requestId":9})
    elif step=="resolve-string":
        server.notify("serverRequest/resolved",{"threadId":"child-b","requestId":"9"})
    elif step=="replay":
        emit(server,question(9))
        emit(server,question("9",thread="child-b",secret=True))


def answered(server,reply):
    original=server.native_callbacks.get(json.dumps(reply["id"]))
    if original:
        params=original["params"]
        server.notify("serverRequest/resolved",{"threadId":params.get("threadId",params.get("conversationId")),"requestId":reply["id"]})
