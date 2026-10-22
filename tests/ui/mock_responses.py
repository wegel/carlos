"""Serve fixed model replies to the real Codex harness without inference."""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import threading


def tool(call_id, namespace, name, arguments):
    return {"type": "function_call", "call_id": call_id, "namespace": namespace,
            "name": name, "arguments": json.dumps(arguments)}


def answer(text):
    return {"type": "message", "role": "assistant", "id": "message-" + text[:12],
            "phase": "final_answer", "content": [{"type": "output_text", "text": text}]}


class Model:
    def __init__(self, root, reply):
        self.root = root
        self.reply = reply
        self.requests = []
        self.errors = []
        self.lock = threading.Lock()
        self.release = threading.Event()
        model = self

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):
                try:
                    body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                    with model.lock:
                        model.requests.append(body)
                        sequence = len(model.requests)
                        with (root / "model-requests.jsonl").open("a") as log:
                            log.write(json.dumps(body) + "\n")
                    items = model.reply(body)
                    response_id = f"mock-{sequence}"
                    events = [{"type": "response.created", "response": {"id": response_id}}]
                    events += [{"type": "response.output_item.done", "item": item} for item in items]
                    events += [{"type": "response.completed", "response": {
                        "id": response_id, "usage": {"input_tokens": 0, "output_tokens": 0,
                        "total_tokens": 0, "input_tokens_details": None, "output_tokens_details": None}}}]
                    data = "".join("event: " + event["type"] + "\ndata: " + json.dumps(event) + "\n\n"
                                   for event in events).encode()
                    self.send_response(200)
                    self.send_header("Content-Type", "text/event-stream")
                    self.send_header("Content-Length", str(len(data)))
                    self.end_headers()
                    self.wfile.write(data)
                except Exception as error:
                    model.errors.append(str(error))
                    self.send_error(500, str(error))

            def log_message(self, *_args):
                pass

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        home = root / "codex-home"
        home.mkdir(parents=True, exist_ok=True)
        (home / "config.toml").write_text(f'''model = "gpt-6.1-sol"
model_provider = "fixture"
model_reasoning_effort = "low"
approval_policy = "never"
sandbox_mode = "read-only"
[model_providers.fixture]
name = "Carlos local test fixture"
base_url = "http://127.0.0.1:{self.server.server_port}/v1"
wire_api = "responses"
supports_websockets = false
requires_openai_auth = false
[features]
shell_snapshot = false
multi_agent_v2 = true
''')

    def close(self):
        self.release.set()
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=3)
        assert not self.errors, self.errors


def outputs(body):
    return {item["call_id"]: item["output"] for item in body.get("input", [])
            if item.get("type") == "function_call_output"}
