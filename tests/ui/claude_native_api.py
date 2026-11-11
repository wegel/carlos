"""A localhost Messages endpoint for the unmodified native Claude CLI, without account calls."""
import http.server
import json
from pathlib import Path
import re
import threading
import uuid
from urllib.parse import urlsplit


class NativeClaudeAPI:
    def __init__(self, root, scenario="text"):
        self.root = root
        self.scenario = scenario
        self.requests = []
        self.count_token_requests = 0
        self.lock = threading.Lock()
        self.context_release = threading.Event()
        owner = self

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_POST(self):
                body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                path = urlsplit(self.path).path
                if path.endswith("/count_tokens"):
                    with owner.lock:
                        owner.count_token_requests += 1
                    self.send_response(200)
                    self.send_header("Content-Type", "application/json")
                    self.end_headers()
                    self.wfile.write(b'{"input_tokens":10}')
                    return
                if not path.endswith("/messages"):
                    self.send_error(404)
                    return
                with owner.lock:
                    index = len(owner.requests)
                    owner.requests.append(body)
                    (root / "api-requests.json").write_text(json.dumps(owner.requests,indent=2))
                if index >= 20:
                    self.send_error(429,"Local contract exceeded its request bound")
                    return
                try:
                    block = owner.reply(index,body)
                    self.send_response(200)
                    self.send_header("Content-Type", "text/event-stream")
                    self.end_headers()
                    def emit(kind,value):
                        self.wfile.write(("event: " + kind + "\ndata: " + json.dumps(value) + "\n\n").encode())
                        self.wfile.flush()
                    emit("message_start",{"type":"message_start","message":{"id":"msg_"+uuid.uuid4().hex,
                        "type":"message","role":"assistant","model":body["model"],"content":[],
                        "stop_reason":None,"stop_sequence":None,"usage":owner.usage(index)}})
                    if isinstance(block,str):
                        emit("content_block_start",{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}})
                        emit("content_block_delta",{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":block}})
                    else:
                        name,tool_input = block
                        emit("content_block_start",{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_"+uuid.uuid4().hex,"name":name,"input":{}}})
                        emit("content_block_delta",{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":json.dumps(tool_input)}})
                    emit("content_block_stop",{"type":"content_block_stop","index":0})
                    emit("message_delta",{"type":"message_delta","delta":{"stop_reason":"end_turn" if isinstance(block,str) else "tool_use","stop_sequence":None},"usage":{"output_tokens":5}})
                    emit("message_stop",{"type":"message_stop"})
                except (BrokenPipeError,ConnectionResetError):
                    pass
        self.server = http.server.ThreadingHTTPServer(("127.0.0.1",0),Handler)
        threading.Thread(target=self.server.serve_forever,daemon=True).start()

    @property
    def url(self):
        return f"http://127.0.0.1:{self.server.server_port}"

    def usage(self,index):
        if self.scenario == "context":
            return {"input_tokens":123 + index,"cache_read_input_tokens":32000 + index*1000,
                "cache_creation_input_tokens":8000,"output_tokens":0}
        return {"input_tokens":10,"output_tokens":0}

    def reply(self,index,body):
        if self.scenario == "rewind":
            if index == 0:
                return "Bash", {"command":"printf NATIVE_REWIND_TOOL_DONE","description":"Kept tool-rich turn"}
            if index == 2 and not self.context_release.wait(30):
                raise AssertionError("Rewind fixture was not released")
        if self.scenario == "context":
            if index == 0:
                return "Bash", {"command":"printf NATIVE_CONTEXT_TOOL_DONE","description":"Isolated context fixture"}
            if index == 1 and not self.context_release.wait(30):
                raise AssertionError("Context fixture was not released")
        if self.scenario == "question" and index == 0:
            return "AskUserQuestion", {"questions":[{"question":"Which native output?","header":"Format","multiSelect":False,
                "options":[{"label":"Summary","description":"Short"},{"label":"Detailed","description":"Long"}]}]}
        if self.scenario == "plan":
            if index == 0:
                paths = re.findall(r'/[^\s<>"\\]+\.md',json.dumps(body))
                plan = next((Path(path) for path in paths if "/plans/" in path and Path(path).is_relative_to(self.root)),None)
                if plan is None:
                    raise AssertionError("Native plan mode did not supply an isolated plan path")
                return "Write", {"file_path":str(plan),"content":"# Packaged native plan\nCheck the isolated fixture.\n"}
            if index == 1:
                return "ExitPlanMode", {}
        if self.scenario == "background" and index in (0,4):
            gate = self.root / ("release-first" if index == 0 else "release-stop")
            return "Bash", {"command":f"while test ! -e '{gate}'; do sleep 0.05; done; printf NATIVE_BACKGROUND_DONE",
                "description":"Wait for isolated native contract gate","run_in_background":True}
        return f"NATIVE_{self.scenario.upper()}_ANSWER_{index}"

    def close(self):
        self.context_release.set()
        for name in ("release-first","release-stop"):
            (self.root/name).touch()
        self.server.shutdown()
        self.server.server_close()
