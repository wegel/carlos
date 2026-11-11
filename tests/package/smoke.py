"""Check a standalone package from a hostile cwd without Node, Bun, or package tools in PATH."""
import argparse
import json
import os
from pathlib import Path
import queue
import re
import subprocess
import sys
import tempfile
import threading
import uuid


class Helper:
    def __init__(self, executable, cwd, env, timeout=8):
        self.timeout = timeout
        self.frames = []
        self.queue = queue.Queue()
        self.process = subprocess.Popen([str(executable)], cwd=cwd, env=env,
                                        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        def read():
            for line in self.process.stdout:
                frame = json.loads(line)
                self.frames.append(frame)
                self.queue.put(frame)
        threading.Thread(target=read, daemon=True).start()
        self.sequence = 0

    def wait(self, predicate):
        for frame in self.frames:
            if predicate(frame):
                return frame
        while True:
            frame = self.queue.get(timeout=self.timeout)
            if predicate(frame):
                return frame

    def send(self, method, params=None):
        self.sequence += 1
        identifier = str(self.sequence)
        self.process.stdin.write(json.dumps({"v":1,"id":identifier,"method":method,"params":params or {}}) + "\n")
        self.process.stdin.flush()
        reply = self.wait(lambda frame: frame.get("type") == "reply" and frame.get("id") == identifier)
        assert reply["ok"], reply
        return reply.get("result")

    def close(self):
        try:
            if self.process.poll() is None:
                self.send("shutdown")
                self.process.wait(timeout=6)
        finally:
            if self.process.poll() is None:
                self.process.kill()
                self.process.wait(timeout=3)
        assert self.process.returncode == 0, self.process.stderr.read()


def session(config, cwd, identifier, title):
    folder = config / "projects" / re.sub(r"[^a-zA-Z0-9]", "-", str(cwd))
    folder.mkdir(parents=True)
    rows = [{"type":"user","uuid":str(uuid.uuid4()),"parentUuid":None,"message":{"role":"user","content":"Local package fixture"}},
            {"type":"custom-title","customTitle":title}]
    (folder / (identifier + ".jsonl")).write_text("".join(json.dumps({"cwd":str(cwd),"sessionId":identifier,
        "timestamp":"2026-11-10T00:00:00Z",**row}) + "\n" for row in rows))


def environment(root):
    env = {"HOME":str(root / "home"),"USERPROFILE":str(root / "home"),"PATH":str(root / "empty-path"),
           "XDG_STATE_HOME":str(root / "state"),"XDG_CONFIG_HOME":str(root / "config")}
    for key in ("SystemRoot","WINDIR","TEMP","TMP"):
        if key in os.environ:
            env[key] = os.environ[key]
    (root / "home").mkdir()
    (root / "empty-path").mkdir()
    return env


def worker(executable, root, env):
    payload = {"kind":"elicitation","payload":{"mode":"form","requestedSchema":{"type":"object",
        "properties":{"value":{"type":"string","pattern":"^ok$"}},"required":["value"]}},
        "answer":{"action":"accept","content":{"value":"ok"}}}
    run = subprocess.run([str(executable),"--validate-native-answer"],cwd=root,env=env,
                         input=json.dumps(payload),text=True,capture_output=True,timeout=4,check=True)
    assert json.loads(run.stdout) == {"ok":True}, run.stdout


def callback_worker(executable, root, env):
    """Exercise parent-to-self validator spawning with no external JS runtime or PATH tools."""
    cli = root / "fixture-claude"
    cli.write_text("#!" + sys.executable + "\n" + r'''
import json, os, sys
with open(os.environ['PACKAGE_ENV_LOG'],'w') as output:
    json.dump({key:os.environ.get(key) for key in ['ANTHROPIC_API_KEY','ANTHROPIC_BASE_URL','ANTHROPIC_AUTH_TOKEN','CLAUDE_CONFIG_DIR','PACKAGE_MARKER']},output)
def send(value): print(json.dumps(value),flush=True)
for line in sys.stdin:
    frame=json.loads(line)
    if frame['type']=='control_request':
        send({'type':'control_response','response':{'subtype':'success','request_id':frame['request_id'],
             'response':{'commands':[],'models':[],'agents':[],'account':{},'output_style':'default'}}})
    elif frame['type']=='user':
        send({'type':'control_request','request_id':'package-form','request':{'subtype':'elicitation',
              'mcp_server_name':'package-fixture','mode':'form','message':'Package form',
              'requested_schema':{'type':'object','properties':{'value':{'type':'string','pattern':'^ok$'}},'required':['value']}}})
''')
    cli.chmod(0o755)
    helper = Helper(executable,root,{**env,"PACKAGE_ENV_LOG":str(root / "native-env.json")})
    try:
        helper.send("start",{"executable":str(cli),"cwd":str(root),"settingSources":[]})
        helper.send("prompt",{"uuid":str(uuid.uuid4()),"text":"Package form","origin":{"kind":"unclassified"},"priority":"later"})
        helper.wait(lambda frame: frame.get("callbackId") == "package-form")
        helper.send("callback.respond",{"callbackId":"package-form","answer":{"action":"accept","content":{"value":"ok"}}})
        actual = json.loads((root / "native-env.json").read_text())
        assert actual["ANTHROPIC_API_KEY"] == env.get("ANTHROPIC_API_KEY"), actual
        assert actual["ANTHROPIC_BASE_URL"] == env.get("ANTHROPIC_BASE_URL"), actual
        assert actual["ANTHROPIC_AUTH_TOKEN"] is None, actual
        assert actual["PACKAGE_MARKER"] is None, actual
    finally:
        helper.close()


def check(executable, root):
    env = environment(root)
    poisoned = root / "poison-config"
    explicit = root / "explicit-config"
    identifier = str(uuid.uuid4())
    session(poisoned,root,identifier,"ENV_FILE_WAS_LOADED")
    session(explicit,root,identifier,"EXPLICIT_PROCESS_ENV")
    (root / ".env").write_text("\n".join(key + "=" + json.dumps(value) for key,value in {
        "CLAUDE_CONFIG_DIR":str(poisoned),"ANTHROPIC_API_KEY":"poison-fake-key",
        "ANTHROPIC_BASE_URL":"http://127.0.0.1:9/poison","ANTHROPIC_AUTH_TOKEN":"poison-fake-token",
        "PACKAGE_MARKER":"ENV_FILE_WAS_LOADED"}.items()) + "\n")
    (root / "poison.ts").write_text('throw new Error("BUNFIG_PRELOAD_WAS_LOADED");\n')
    (root / "bunfig.toml").write_text('preload = ["./poison.ts"]\n[install]\nauto = "force"\n')
    (root / "tsconfig.json").write_text('{"compilerOptions":{"paths":{"*": ["./poison.ts"]}}}\n')
    (root / "package.json").write_text('{"type":"commonjs","dependencies":{"must-not-download-at-runtime":"0.0.0"}}\n')
    for process_env, expected in [(env,None),({**env,"CLAUDE_CONFIG_DIR":str(explicit),
          "ANTHROPIC_API_KEY":"explicit-fake-key","ANTHROPIC_BASE_URL":"http://127.0.0.1:9/explicit"},"EXPLICIT_PROCESS_ENV")]:
        helper = Helper(executable,root,process_env)
        try:
            assert helper.send("hello")["sdkVersion"] == "0.3.322"
            info = helper.send("sessions.info",{"cwd":str(root),"sessionId":identifier})
            assert (info or {}).get("customTitle") == expected, info
        finally:
            helper.close()
        worker(executable,root,process_env)
        if os.name != "nt":
            callback_worker(executable,root,process_env)
    assert not (root / "node_modules").exists(), "Runtime attempted to install a package"
    (root / "result.json").write_text(json.dumps({"passed":True,"platform":sys.platform,
        "helper":str(executable),"pathContainsNodeOrBun":False,"hostileCwd":True,"callbackSelfWorker":os.name != "nt"},indent=2)+"\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--helper",type=Path,required=True)
    parser.add_argument("--binary",type=Path)
    parser.add_argument("--artifacts",type=Path)
    options = parser.parse_args()
    root = (options.artifacts or Path(tempfile.mkdtemp(prefix="carlos-package-"))).resolve()
    root.mkdir(parents=True,exist_ok=True)
    if options.binary:
        run = subprocess.run([str(options.binary.resolve()),"--help"],env={**os.environ,"CARLOS_TOOL_SUMMARIES":"off"},capture_output=True,text=True,timeout=5,check=True)
        assert "carlos" in (run.stdout+run.stderr).lower(),run.stdout+run.stderr
    check(options.helper.resolve(),root)
    print("PASS standalone hostile-cwd, explicit environment, sessions, and validator; artifacts:",root)


if __name__ == "__main__":
    main()
