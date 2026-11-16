#!/usr/bin/env python3
"""Stage only the phone sources and their locked crates for an offline Mac build."""

import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
CRATES = ("carlos-phone", "carlos-client", "carlos-proto")


def metadata(root, *arguments):
    """Read Cargo's resolved graph without writing it to a terminal log."""
    command = ["cargo", "metadata", "--format-version", "1", *arguments]
    return json.loads(subprocess.check_output(command, cwd=root))


def phone_packages(graph):
    """Follow all feature/platform edges from the actual phone crate."""
    packages = {package["id"]: package for package in graph["packages"]}
    nodes = {node["id"]: node for node in graph["resolve"]["nodes"]}
    pending = [key for key, package in packages.items() if package["name"] == "carlos-phone"]
    visited = set()
    while pending:
        key = pending.pop()
        if key not in visited:
            visited.add(key)
            pending.extend(nodes[key]["dependencies"])
    return [packages[key] for key in sorted(visited)]


def copy_sources(destination):
    """Keep private settings, daemon code and build products off the Mac."""
    for name in CRATES:
        shutil.copytree(ROOT / "crates" / name, destination / "crates" / name)
    shutil.copy2(ROOT / "LICENSE", destination / "LICENSE")
    shutil.copy2(ROOT / "Cargo.lock", destination / "Cargo.lock")
    app = ROOT / "apps/ios"
    if app.exists():
        shutil.copytree(app, destination / "apps/ios")
    script = ROOT / "scripts/ios-remote.sh"
    if script.exists():
        shutil.copy2(script, destination / script.name)
    members = ", ".join(json.dumps(f"crates/{name}") for name in CRATES)
    (destination / "Cargo.toml").write_text(
        f'[workspace]\nmembers = [{members}]\nresolver = "2"\n\n'
        '[profile.release]\nlto = "thin"\ncodegen-units = 1\n'
    )


def vendor(destination):
    """Keep verified third-party sources at a stable path across app edits."""
    result = subprocess.run(
        ["cargo", "vendor", "--offline", "--locked", "--versioned-dirs", "vendor"],
        cwd=destination, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True,
    )
    if result.returncode:
        raise RuntimeError(result.stderr)
    identity = fingerprint(destination / "vendor")
    shared = ROOT / "target/ios-dependencies" / identity
    shared.mkdir(parents=True, exist_ok=True)
    if not (shared / "vendor").exists():
        shutil.move(destination / "vendor", shared / "vendor")
    else:
        shutil.rmtree(destination / "vendor")
    (destination / "dependencies-id").write_text(identity + "\n")
    config = destination / ".cargo"
    config.mkdir()
    (config / "config.toml").write_text(
        '[source.crates-io]\nreplace-with = "vendored-sources"\n'
        f'[source.vendored-sources]\ndirectory = "../../ios-dependencies/{identity}/vendor"\n'
    )


def fingerprint(directory):
    """Name immutable source bundles by every staged byte and relative path."""
    digest = hashlib.sha256()
    for path in sorted(directory.rglob("*")):
        if path.is_file():
            digest.update(str(path.relative_to(directory)).encode() + b"\0")
            digest.update(path.read_bytes())
    return digest.hexdigest()[:20]


def main():
    """Print the safe source bundle path for ios-build.sh."""
    graph = metadata(ROOT, "--all-features", "--locked")
    packages = phone_packages(graph)
    parent = ROOT / "target/ios-source"
    parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="stage-", dir=parent) as temporary:
        stage = Path(temporary)
        copy_sources(stage)
        narrowed = metadata(stage, "--all-features", "--offline")
        expected = {(p["name"], p["version"]) for p in packages}
        actual = {(p["name"], p["version"]) for p in narrowed["packages"]}
        # Removing unrelated workspace members can remove unified optional features.
        # It must never choose a new dependency or version beyond the checked lock.
        if not actual <= expected:
            raise ValueError(f"phone dependency graph changed: {actual - expected}")
        vendor(stage)
        destination = parent / fingerprint(stage)
        if not destination.exists():
            shutil.copytree(stage, destination)
        print(destination)


if __name__ == "__main__":
    main()
