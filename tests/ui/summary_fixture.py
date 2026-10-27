"""Commands are fixture data; neither the peer nor the summarizer executes them."""


def summary_items():
    script = "python3 - <<'PY'\n" + "\n".join([
        "from pathlib import Path", "path = Path('result.txt')", "text = path.read_text()",
        "text = text.strip()", "text = text.lower()", "text = text.replace('old', 'new')",
        "text = text.replace('before', 'after')", "text = text.rstrip()", "text += '\\n'",
        "path.write_text(text)", "print('original output marker')", "PY"])
    chain = "git add " + "source/path/" * 20 + "file.rs && git commit -m 'update source'"
    return [{"type": "commandExecution", "id": "summary-script", "command": script,
             "aggregatedOutput": "original output marker", "exitCode": 0},
            {"type": "commandExecution", "id": "summary-chain", "command": chain,
             "aggregatedOutput": "", "exitCode": 0},
            {"type": "commandExecution", "id": "summary-simple", "command": "cp source.txt target.txt",
             "aggregatedOutput": "", "exitCode": 0}]


def ep130_items():
    """Match the short but substantial heredocs that EP130 previously skipped."""
    import shlex
    local = "python3 - <<'PY'\nfrom pathlib import Path\nimport json\np=next(Path('out/ep130-furious-finish/final268-screen').glob('*.jsonl'))\nfor line in p.read_text().splitlines():\n s=json.loads(line);print({k:s.get(k) for k in ['state','position_ms','last_command_id']},s.get('cut_playback',{}).get('position_ms'))\nPY"
    remote = "python3 - <<'PY'\nimport subprocess,pathlib,plistlib\nu='simulator-id'\np=pathlib.Path(subprocess.check_output(['xcrun','simctl','get_app_container',u,'com.example.app','data'],text=True).strip())\nx=plistlib.loads((p/'Library/Preferences/com.example.app.plist').read_bytes())\nprint({k:v for k,v in x.items() if k.startswith('tmdb.') and ('language' in k or 'locale' in k)})\nPY"
    commands = [
        'rtk proxy ' + local,
        'rtk proxy ssh -o BatchMode=yes user@host ' + shlex.quote(remote),
        "rtk proxy python3 - <<'PY'\nprint(1)\nPY",
        "rtk proxy ssh user@host 'python3 process.py'",
    ]
    return [{'type': 'commandExecution', 'id': f'ep130-{index}', 'command': command,
             'aggregatedOutput': 'EP130 output marker', 'exitCode': 0}
            for index, command in enumerate(commands)]
