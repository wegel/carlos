#!/usr/bin/env python3
"""Check the library's resolved packages, including inactive platform dependencies."""

import json
from pathlib import Path
import subprocess
import sys


REVIEWED = {
    "carlos-proto", "anyhow", "base64", "blake3", "jsonschema", "libc", "mio", "rusqlite",
    "serde_json", "shlex", "socket2", "tempfile", "tungstenite", "url", "uuid",
}
DAEMON_REVIEWED = {
    "carlos-core", "carlos-proto", "carlos-client", "anyhow", "getrandom", "libc", "rusqlite",
    "serde", "serde_json", "signal-hook", "socket2", "uuid", "tempfile", "blake3", "tungstenite",
}
PHONE_REVIEWED = {"carlos-client", "carlos-proto", "serde_json", "syntect", "uniffi"}
PHONE_FORBIDDEN = {
    "carlos-core", "carlosd", "tokio", "async-std", "reqwest", "tungstenite",
    "socket2", "mio", "rustls",
}
FORBIDDEN = {
    "carlos", "crossterm", "ratatui", "termion", "tui", "ansi-to-tui",
    "cpal", "rubato", "webrtc-vad", "whisper-rs", "whisper-rs-sys", "alsa",
    "alsa-sys", "coreaudio-rs", "coreaudio-sys",
}


PROTO_PACKAGES = {
    "carlos-proto", "serde", "serde_core", "serde_derive", "serde_json", "itoa",
    "memchr", "zmij", "proc-macro2", "quote", "syn", "unicode-ident",
}


def check_pure_package(packages, nodes, owner):
    """Keep wire types and shared client free of transport and application packages."""
    pending = [key for key, package in packages.items() if package["name"] == owner]
    allowed = PROTO_PACKAGES | {owner}
    if owner == "carlos-client":
        # Cargo unifies optional HTML/CLI features with the terminal in --all-features.
        # All of these packages are pure parsers/formatters; the phone disables defaults.
        allowed |= {"pulldown-cmark", "pulldown-cmark-escape", "bitflags", "unicase",
                    "getopts", "unicode-width"}
    visited, issues = set(), []
    while pending:
        key = pending.pop()
        if key in visited:
            continue
        visited.add(key)
        name = packages[key]["name"]
        if name not in allowed:
            issues.append(f"{owner}: unreviewed resolved package {name}")
        pending.extend(nodes[key]["dependencies"])
    return issues


def check_graph(metadata):
    """Return errors for new direct packages or forbidden indirect packages."""
    packages = {package["id"]: package for package in metadata["packages"]}
    owners = [key for key in metadata["workspace_members"]
              if packages[key]["name"] == "carlos-core"]
    if len(owners) != 1:
        return ["Expected exactly one carlos-core workspace member"]
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    issues = check_pure_package(packages, nodes, "carlos-proto")
    issues.extend(check_pure_package(packages, nodes, "carlos-client"))
    selected = [owners[0]] + [key for key in metadata["workspace_members"]
                              if packages[key]["name"] in {"carlosd", "carlos-phone"}]
    reviewed_by_owner = {"carlos-core": REVIEWED, "carlosd": DAEMON_REVIEWED,
                         "carlos-phone": PHONE_REVIEWED}
    for owner in selected:
        name = packages[owner]["name"]
        issues.extend(check_owner(packages, nodes, owner, reviewed_by_owner[name]))
    return sorted(set(issues))


def check_owner(packages, nodes, owner, reviewed):
    """Check both native-service crates without letting the UI graph enter either."""
    label = packages[owner]["name"]
    issues = []
    forbidden = FORBIDDEN | (PHONE_FORBIDDEN if label == "carlos-phone" else set())
    if packages[owner].get("features", {}).get("default", []):
        issues.append(f"{label}: default features must stay empty; fixture helpers are opt-in")
    for key in nodes[owner]["dependencies"]:
        name = packages[key]["name"]
        if name not in reviewed:
            issues.append(f"{label}: unreviewed direct package {name}")
    pending, visited = [owner], set()
    while pending:
        key = pending.pop()
        if key in visited:
            continue
        visited.add(key)
        name = packages[key]["name"]
        if name in forbidden or name.startswith(("ratatui-", "tui-")):
            issues.append(f"{label}: forbidden resolved package {name}")
        pending.extend(nodes[key]["dependencies"])
    return issues


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
    print("Core/daemon/phone: no terminal/audio/speech; phone: no backend/transport runtime; protocol/client: reviewed data only")
