#!/usr/bin/env python3
"""Check the library's resolved packages, including inactive platform dependencies."""

import json
from pathlib import Path
import subprocess
import sys


REVIEWED = {
    "anyhow", "base64", "blake3", "jsonschema", "libc", "mio", "rusqlite",
    "serde_json", "shlex", "socket2", "tempfile", "tungstenite", "url", "uuid",
}
FORBIDDEN = {
    "carlos", "crossterm", "ratatui", "termion", "tui", "ansi-to-tui",
    "cpal", "rubato", "webrtc-vad", "whisper-rs", "whisper-rs-sys", "alsa",
    "alsa-sys", "coreaudio-rs", "coreaudio-sys",
}


def check_graph(metadata):
    """Return errors for new direct packages or forbidden indirect packages."""
    packages = {package["id"]: package for package in metadata["packages"]}
    owners = [key for key in metadata["workspace_members"]
              if packages[key]["name"] == "carlos-core"]
    if len(owners) != 1:
        return ["Expected exactly one carlos-core workspace member"]
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    owner = owners[0]
    issues = []
    if packages[owner].get("features", {}).get("default", []):
        issues.append("carlos-core: default features must stay empty; fixture helpers are opt-in")
    for key in nodes[owner]["dependencies"]:
        name = packages[key]["name"]
        if name not in REVIEWED:
            issues.append(f"carlos-core: unreviewed direct package {name}")
    pending, visited = [owner], set()
    while pending:
        key = pending.pop()
        if key in visited:
            continue
        visited.add(key)
        name = packages[key]["name"]
        if name in FORBIDDEN or name.startswith(("ratatui-", "tui-")):
            issues.append(f"carlos-core: forbidden resolved package {name}")
        pending.extend(nodes[key]["dependencies"])
    return sorted(set(issues))


def check_tree(root):
    """Ask Cargo for all feature and platform branches without compiling them."""
    result = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--all-features", "--locked"],
        cwd=root, check=True, text=True, stdout=subprocess.PIPE,
    )
    return check_graph(json.loads(result.stdout))


if __name__ == "__main__":
    errors = check_tree(Path(__file__).resolve().parents[1])
    if errors:
        print("\n".join(errors), file=sys.stderr)
        sys.exit(1)
    print("Core packages: no terminal, audio or speech dependency in any target branch")
