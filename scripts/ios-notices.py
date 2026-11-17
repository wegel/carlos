#!/usr/bin/env python3
"""Bundle original notices from the locked phone crates, excluding unrelated workspace features."""

import hashlib
import json
from pathlib import Path
import subprocess
import sys


def write_notices(root):
    graph = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--offline"], cwd=root))
    tree = subprocess.check_output(
        ["cargo", "tree", "-p", "carlos-phone", "--target", "aarch64-apple-ios", "-e", "normal",
         "--prefix", "none", "--format", "{p}"], cwd=root, text=True)
    selected = {(line.split()[0], line.split()[1].removeprefix("v")) for line in tree.splitlines()}
    notices = ["Original license texts for the locked iPhone Rust libraries.\n\nCarlos\n", (root / "LICENSE").read_text()]
    seen = set()
    for package in sorted(graph["packages"], key=lambda package: (package["name"], package["version"])):
        if (package["name"], package["version"]) not in selected or package["source"] is None:
            continue
        notices.append(f"\n{package['name']} {package['version']} ({package['license']})\n")
        folder = Path(package["manifest_path"]).parent
        files = sorted(path for path in folder.iterdir() if path.is_file()
                       and any(name in path.name.lower() for name in ("license", "copying", "copyright")))
        if package["name"].startswith("uniffi"):
            if package["version"] != "0.32.2":
                raise RuntimeError("Refresh the bundled UniFFI notice for the new locked version")
            notices.append("Source: https://github.com/mozilla/uniffi-rs/tree/v0.32.2\nSee the bundled UniFFI-MPL-2.0.txt.\n")
        elif not files:
            raise RuntimeError(f"No original license text found for {package['name']}")
        for path in files:
            text = path.read_text()
            identity = hashlib.sha256(text.encode()).hexdigest()
            notices.append(f"\n{path.name}, text SHA256 {identity}\n")
            if identity not in seen:
                notices.append(text + "\n")
                seen.add(identity)
            else:
                notices.append("Identical license text is included above.\n")
    notices.append("\nBundled syntax data notices are in syntect-MIT.txt.\n")
    destination = root / "apps/ios/Resources/Licenses/Rust-notices.txt"
    destination.write_text("".join(notices))


if __name__ == "__main__":
    write_notices(Path(sys.argv[1]).resolve())
