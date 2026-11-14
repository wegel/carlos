#!/usr/bin/env python3
"""Run each captured-source formatter case alone and verify private copied bytes."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import uuid

REPO = Path(__file__).resolve().parents[1]


def private_environment(root):
    private = root / "formatter-env"
    programs = private / "bin"
    programs.mkdir(parents=True, exist_ok=True)
    shim = ("#!/usr/bin/env python3\nimport os,sys\nfrom pathlib import Path\n"
            "Path(os.environ['CARLOS_LARGE_COPY_PATH']).write_bytes(sys.stdin.buffer.read())\n")
    for name in ("wl-copy", "xclip", "xsel", "pbcopy"):
        program = programs / name
        program.write_text(shim)
        program.chmod(0o755)
    environment = dict(os.environ)
    environment["PATH"] = str(programs) + os.pathsep + environment["PATH"]
    for key in ("XDG_CONFIG_HOME", "XDG_STATE_HOME", "CODEX_HOME", "CLAUDE_CONFIG_DIR", "GROK_HOME"):
        directory = private / key.lower()
        directory.mkdir(exist_ok=True, mode=0o700)
        environment[key] = str(directory)
    for key in ("SSH_TTY", "SSH_CONNECTION", "SSH_CLIENT"):
        environment[key] = ""
    environment.update(CARLOS_LARGE_PROBE=str(root), CARLOS_LARGE_COPY="1")
    return environment


def verify_copy(path, expected):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024*1024):
            digest.update(chunk)
    assert digest.hexdigest() == expected, "clipboard must preserve every original byte"


def run_case(executable, root, environment, case, width, samples, checkpoints):
    environment = dict(environment)
    prefix = "checkpoint" if checkpoints else "width"
    run_id = uuid.uuid4().hex
    copied = root / case["name"] / f"clipboard-{prefix}-{width}-{run_id}.txt"
    environment.update(CARLOS_LARGE_CASE=case["name"], CARLOS_LARGE_WIDTH=str(width),
                       CARLOS_LARGE_SAMPLES=str(samples), CARLOS_LARGE_COPY_PATH=str(copied),
                       CARLOS_LARGE_RUN_ID=run_id)
    probe = "large_ansi_checkpoint_probe" if checkpoints else "large_source_layout_probe"
    with (root / case["name"] / f"{prefix}-{width}.log").open("w") as log:
        result = subprocess.run(["rtk", "proxy", "taskset", "-c", "6", executable,
            probe, "--ignored", "--nocapture", "--test-threads=1"],
            cwd=REPO, env=environment, stdout=log, stderr=subprocess.STDOUT)
    assert result.returncode == 0, (case["name"], width, result.returncode)
    metrics = json.loads((root / case["name"] / f"{prefix}-{width}.json").read_text())
    assert metrics.get("run_id") == run_id, "the selected probe must produce fresh metrics for this run"
    assert metrics["width"] == width and metrics["source_bytes"] == case["bytes"]
    assert metrics["clipboard"]["backend"] in ("wl-copy", "xclip", "xsel", "pbcopy"), "private clipboard program must succeed"
    assert metrics["clipboard"]["path"] == str(copied), "this run must write its own clipboard file"
    verify_copy(copied, case["sha256"])
    if checkpoints:
        return {"case":case["name"], "width":width, "bytes":case["bytes"], "copy_digest_verified":True,
            "rows":metrics["rows"], "index_ms":metrics["index_ms"],
            "index_capacity_bytes":metrics["index_capacity_bytes"],
            "indexed_memory":metrics["indexed_memory"], "viewport":metrics["viewport"],
            "append_p95":metrics["append"]["p95_ms"], "resize_p95":metrics["resize"]["p95_ms"]}
    return {"case":case["name"], "width":width, "bytes":case["bytes"], "copy_digest_verified":True,
        "rows":metrics["baseline"]["rows"], "count_ms":metrics["baseline"]["count_ms"],
        "materialize_ms":metrics["baseline"]["materialize_ms"], "source_memory":metrics["pinned"],
        "raster_memory":metrics["baseline"]["after_raster"],
        "raster_capacity":metrics["baseline"]["raster_vec_string_capacities_bytes"],
        "append_p95":metrics["append"]["p95_ms"], "resize_p95":metrics["resize"]["p95_ms"]}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root", type=Path)
    parser.add_argument("--build-json", type=Path, required=True,
                        help="cargo test --no-run --message-format=json output")
    parser.add_argument("--samples", type=int, default=16)
    parser.add_argument("--widths", type=int, nargs="+", default=[28,120])
    parser.add_argument("--ansi-checkpoints", action="store_true",
                        help="measure the private ANSI checkpoint candidate against saved full-raster rows")
    args = parser.parse_args()
    if not 1 <= args.samples <= 32 or any(width < 8 for width in args.widths):
        parser.error("use 1..32 samples and widths of at least 8 columns")
    records = (json.loads(line) for line in args.build_json.read_text().splitlines())
    artifact = next(record for record in records if record.get("reason") == "compiler-artifact"
        and record.get("target",{}).get("name") == "carlos" and record.get("profile",{}).get("test"))
    executable = artifact["executable"]
    root = args.root.resolve()
    manifest = json.loads((root / "manifest.json").read_text())
    identity = {"path":executable, "sha256":hashlib.sha256(Path(executable).read_bytes()).hexdigest(),
                "profile":artifact["profile"], "features":artifact["features"],
                "source":artifact["target"]["src_path"]}
    name = "checkpoint-identity.json" if args.ansi_checkpoints else "formatter-identity.json"
    (root / name).write_text(json.dumps(identity,indent=2)+"\n")
    environment = private_environment(root)
    for case in manifest["cases"]:
        if args.ansi_checkpoints and case["name"] != "ansi":
            continue
        for width in args.widths:
            print(json.dumps(run_case(executable, root, environment, case, width, args.samples,
                                      args.ansi_checkpoints)), flush=True)


if __name__ == "__main__":
    main()
