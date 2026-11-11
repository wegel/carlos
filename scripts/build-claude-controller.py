#!/usr/bin/env python3
"""Build the pinned standalone helper for an explicit matching release target."""
import argparse
import json
import os
from pathlib import Path
import platform
import shlex
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
PACKAGE = ROOT / "helpers/claude-controller"
TARGETS = {
    "x86_64-unknown-linux-gnu": "bun-linux-x64-baseline",
    "x86_64-apple-darwin": "bun-darwin-x64",
    "aarch64-apple-darwin": "bun-darwin-arm64",
    "x86_64-pc-windows-msvc": "bun-windows-x64-baseline",
}


def host_target():
    machine = platform.machine().lower()
    if sys.platform == "darwin":
        return "aarch64-apple-darwin" if machine in {"arm64", "aarch64"} else "x86_64-apple-darwin"
    if sys.platform == "win32":
        return "x86_64-pc-windows-msvc"
    if sys.platform.startswith("linux") and machine in {"x86_64", "amd64"}:
        return "x86_64-unknown-linux-gnu"
    raise SystemExit("This host is not a supported release target; choose --target explicitly")


def run(arguments, **kwargs):
    subprocess.run(arguments, cwd=PACKAGE, check=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--skip-tests", action="store_true", help="Build only; native package smoke remains required")
    options = parser.parse_args()
    target = options.target or host_target()
    extension = ".exe" if "windows" in target else ""
    output = (options.output or ROOT / f"target/claude-controller/carlos-claude-controller{extension}").resolve()
    bun = shutil.which(os.environ.get("BUN_BINARY", "bun"))
    if not bun:
        raise SystemExit("Build needs Bun 1.3.10; set BUN_BINARY to its executable")
    version = subprocess.check_output([bun, "--version"], text=True).strip()
    if version != "1.3.10":
        raise SystemExit(f"Build requires pinned Bun 1.3.10; found {version}")
    run([bun, "install", "--frozen-lockfile", "--ignore-scripts", "--omit", "optional", "--omit", "peer"])
    run([bun, "run", "check"])
    native_tests = not options.skip_tests and target == host_target()
    if native_tests:
        run([bun, "run", "test"])
    output.parent.mkdir(parents=True, exist_ok=True)
    command = shlex.split(json.loads((PACKAGE / "package.json").read_text())["scripts"]["build"])
    command[0] = bun
    command[command.index("--outfile") + 1] = str(output)
    command.append("--target=" + TARGETS[target])
    command.append("--metafile=" + str(output.parent / "controller-metafile.json"))
    run(command)
    relink = output.parent / "relink"
    relink.mkdir(exist_ok=True)
    run([bun,"build","--target=bun","--minify","--define","CARLOS_STANDALONE=true",
         "src/main.ts","--outfile",str(relink / "controller.js")])
    (output.parent / "claude-controller-build.json").write_text(json.dumps({
        "sdk": "0.3.322", "bun": version, "rustTarget": target, "bunTarget": TARGETS[target],
        "runtimeAutoload": {"dotenv":False,"bunfig":False,"tsconfig":False,"packageJson":False},
    }, indent=2) + "\n")
    if native_tests:
        run([bun,"run","test"], env={**os.environ,"CARLOS_CONTROLLER_TEST_EXECUTABLE":str(output)})
    print(f"Built {output} ({output.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
