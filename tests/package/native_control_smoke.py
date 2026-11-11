"""Check native CLI startup and public SDK controls on the package's actual host, without inference."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile

from smoke import Helper, environment


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--helper",type=Path,required=True)
    parser.add_argument("--claude",type=Path,required=True)
    parser.add_argument("--artifacts",type=Path)
    options = parser.parse_args()
    root = (options.artifacts or Path(tempfile.mkdtemp(prefix="carlos-native-controls-"))).resolve()
    root.mkdir(parents=True,exist_ok=True)
    native = options.claude.resolve()
    env = {**environment(root),"CLAUDE_CONFIG_DIR":str(root/"claude"),
        "ANTHROPIC_API_KEY":"fake-local-idle-control","ANTHROPIC_BASE_URL":"http://127.0.0.1:9",
        "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC":"1","DISABLE_AUTOUPDATER":"1"}
    version = subprocess.check_output([str(native),"--version"],env=env,text=True,timeout=10).strip()
    helper = Helper(options.helper.resolve(),root,env,timeout=25)
    try:
        capabilities = helper.send("start",{"cwd":str(root),"executable":str(native),"settingSources":[]})
        assert capabilities["models"],capabilities
        helper.send("model.set",{"model":capabilities["models"][0]["value"]})
        helper.send("context")
        helper.send("interrupt")
        usage = helper.send("usage.optional")
        assert usage["available"] is False,usage
    finally:
        helper.close()
    assert not any(frame.get("message",{}).get("type") == "result" for frame in helper.frames)
    (root/"result.json").write_text(json.dumps({"passed":True,"nativeVersion":version,"modelCount":len(capabilities["models"]),
        "inferenceTurns":0,"controls":["start","model.set","context","interrupt","usage.optional","shutdown"]},indent=2)+"\n")
    print("PASS native idle controls:",version,"Artifacts:",root)


if __name__ == "__main__":
    main()
