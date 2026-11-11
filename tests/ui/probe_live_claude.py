"""Make one explicitly invoked subscribed UI proof; normal test runners never call this script."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess

from terminal import Terminal

PROMPT = "Reply with exactly CARLOS_NATIVE_UI_OK. Do not use tools."
MARKER = "CARLOS_NATIVE_UI_OK"
REPO = Path(__file__).resolve().parents[2]


def frames(root,direction):
    path = root/"claude-controller.jsonl"
    if not path.exists(): return []
    return [record["message"] for line in path.read_bytes().splitlines(keepends=True) if line.endswith(b"\n")
            if (record := json.loads(line))["direction"] == direction]


def preflight(native,root):
    present = {key:bool(os.environ.get(key)) for key in ("ANTHROPIC_API_KEY","ANTHROPIC_BASE_URL","ANTHROPIC_AUTH_TOKEN","CLAUDE_CODE_OAUTH_TOKEN")}
    if present["ANTHROPIC_API_KEY"] or present["ANTHROPIC_BASE_URL"]:
        raise RuntimeError("Live subscription proof requires the existing native login without an API-key/base-URL override")
    run = subprocess.run([str(native),"auth","status","--json"],capture_output=True,text=True,timeout=15)
    status = json.loads(run.stdout)
    summary = {key:status.get(key) for key in ("loggedIn","authMethod","apiProvider","subscriptionType")}
    (root/"auth-summary.json").write_text(json.dumps({"auth":summary,"environmentPresent":present},indent=2)+"\n")
    if not summary["loggedIn"] or summary["authMethod"] != "claude.ai":
        raise RuntimeError("Native login did not report the expected existing Claude account; no live prompt sent")
    return summary


def isolate_project(root,native_config):
    bun = shutil.which(os.environ.get("BUN_BINARY","bun"))
    if not bun: raise RuntimeError("This developer-only probe needs the pinned build's Bun to read public tool names")
    tools = json.loads(subprocess.check_output([bun,"-e",'import {BUILTIN_TOOL_NAMES} from "./helpers/claude-controller/node_modules/@anthropic-ai/claude-agent-sdk"; console.log(JSON.stringify(BUILTIN_TOOL_NAMES))'],cwd=REPO,text=True))
    plugins = {}
    settings = native_config/"settings.json"
    if settings.exists():
        configured = json.loads(settings.read_text()).get("enabledPlugins",{})
        plugins = {name:False for name in configured}
    local = root/".claude/settings.local.json"
    local.parent.mkdir()
    local.write_text(json.dumps({"disableAllHooks":True,"enabledPlugins":plugins,
        "permissions":{"deny":[*tools,"mcp__*"]}},indent=2)+"\n")


def run_once(options,root,auth):
    native_home = Path(os.environ.get("HOME",str(Path.home())))
    native_config = Path(os.environ.get("CLAUDE_CONFIG_DIR",str(native_home/".claude")))
    isolate_project(root,native_config)
    env = {key:value for key,value in os.environ.items() if key.startswith(("ANTHROPIC_","CLAUDE_"))}
    env.update({"HOME":str(native_home),"USERPROFILE":str(native_home),"CLAUDE_CONFIG_DIR":str(native_config),
        "CARLOS_TOOL_SUMMARIES":"off","CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC":"1","DISABLE_AUTOUPDATER":"1"})
    terminal = Terminal(options.binary.absolute(),root,"live-subscribed-proof",None,160,"claude",
                        claude=options.claude.resolve(),extra_env=env)
    try:
        terminal.wait(lambda:terminal.input_is(""),"native composer rendered")
        terminal.wait(lambda: any(frame.get("type") == "reply" and frame.get("ok") and isinstance(frame.get("result"),dict) and "models" in frame["result"] for frame in frames(root,"server")),"subscribed startup",timeout=30)
        for _ in range(4): terminal.pump(.05)
        with (root/"live-turn-started").open("x") as output: output.write("One authorized UI turn; do not replay.\n")
        terminal.paste(PROMPT)
        terminal.send(b"\r")
        terminal.wait(lambda: any(frame.get("message",{}).get("type") == "result" for frame in frames(root,"server")),"single subscribed native result",timeout=60)
        messages = [frame["message"] for frame in frames(root,"server") if frame.get("event") == "sdk"]
        results = [message for message in messages if message.get("type") == "result"]
        result = results[-1]
        text = [part.get("text") for message in messages if message.get("type") == "assistant" for part in message.get("message",{}).get("content",[]) if part.get("type") == "text"]
        tools = [part for message in messages if message.get("type") == "assistant" for part in message.get("message",{}).get("content",[]) if part.get("type") == "tool_use"]
        prompts = [frame for frame in frames(root,"client") if frame.get("method") == "prompt"]
        passed = result.get("subtype") == "success" and result.get("is_error") is False and result.get("result") == MARKER and MARKER in text and len(prompts) == 1 and not tools
        proof = {"passed":False,"resultValid":passed,"screenHasAssistantRow":False,"auth":auth,"promptCount":len(prompts),"nativeToolCalls":len(tools),
            "nativeResult":{key:result.get(key) for key in ("subtype","is_error","result","session_id","user_message_uuid","errors")},"assistantText":text}
        (root/"proof.json").write_text(json.dumps(proof,indent=2)+"\n")
        if not passed: raise RuntimeError("The single native result did not satisfy the proof; inspect the private artifact without sending another turn")
        terminal.contains("✓ "+MARKER)
        proof.update({"passed":True,"screenHasAssistantRow":True})
        (root/"proof.json").write_text(json.dumps(proof,indent=2)+"\n")
        terminal.snapshot("subscribed-native-ui")
    finally:
        terminal.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary",type=Path,required=True)
    parser.add_argument("--claude",type=Path,required=True)
    parser.add_argument("--artifacts",type=Path,required=True)
    options = parser.parse_args()
    os.umask(0o077)
    root = options.artifacts.resolve()
    if root.exists(): raise RuntimeError("Use one fresh private artifact folder; an existing live proof must not be replayed")
    root.mkdir(parents=True,mode=0o700)
    auth = preflight(options.claude.resolve(),root)
    run_once(options,root,auth)
    print("PASS single subscribed native UI turn; private evidence:",root/"proof.json")


if __name__ == "__main__":
    main()
