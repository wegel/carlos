"""Stream parser-valid tables in two stages while the real terminal resizes."""
import time

FIRST = "| Name | Description |\n| :--- | :--- |\n| Alpha | CELL_START Each cell wraps its own words while the neighboring column stays aligned, through every remaining word until CELL_END. |\n"
LAST = "| Beta | BETA_DONE 日本語 👩‍💻 |\n"
NARROW = "| First | Second | Third | Fourth |\n| - | - | - | - |\n| VALUE_A | VALUE_B | VALUE_C | VALUE_D |\n"


def start_turn(server,request,params):
    thread = params["threadId"]
    turn = "table-turn"
    item = {"type":"agentMessage","id":"table-answer","phase":"final_answer","text":""}
    user = {"type":"userMessage","id":"table-user","content":params["input"]}
    server.reply(request,{"turn":{"id":turn,"status":"inProgress"}})
    server.notify("turn/started",{"threadId":thread,"turn":{"id":turn,"status":"inProgress"}})
    server.notify("item/started",{"threadId":thread,"turnId":turn,"item":item})
    if params["input"][0]["text"] == "Show narrow table":
        pieces = [NARROW]
    else:
        pieces = [FIRST,LAST]
    for index,piece in enumerate(pieces):
        server.notify("item/agentMessage/delta",{"threadId":thread,"turnId":turn,"itemId":item["id"],"delta":piece})
        if len(pieces) == 2 and index == 0:
            deadline = time.monotonic()+15
            while not (server.root/"finish-table").exists():
                if time.monotonic() > deadline: raise RuntimeError("Test did not release table stream")
                time.sleep(.01)
    item["text"] = "".join(pieces)
    complete = {"id":turn,"status":"completed","items":[user,item]}
    server.threads[thread]["turns"].append(complete)
    server.notify("item/completed",{"threadId":thread,"turnId":turn,"item":item})
    server.notify("turn/completed",{"threadId":thread,"turn":complete})
