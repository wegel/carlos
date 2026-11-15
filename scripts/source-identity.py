#!/usr/bin/env python3
"""Print the length-framed FNV-1a128 source marker shared with scripts/source_fingerprint.rs."""

import argparse
from pathlib import Path

OFFSET = 0x6C62272E07BB014262B821756295C58D
PRIME = 0x0000000001000000000000000000013B
MASK = (1 << 128) - 1


def source_identity(root):
    inputs = [root / "Cargo.toml", root / "Cargo.lock", root / "build.rs",
              root / "scripts/source_fingerprint.rs"]
    inputs.extend(path for path in (root / "src").rglob("*") if path.is_file())
    for package in (root / "crates").iterdir():
        inputs.append(package / "Cargo.toml")
        if (package / "build.rs").is_file():
            inputs.append(package / "build.rs")
        inputs.extend(path for path in (package / "src").rglob("*") if path.is_file())
    helper = root / "helpers/claude-controller"
    inputs.extend(path for path in (helper / "src").rglob("*") if path.is_file())
    inputs.extend(helper / name for name in ("package.json", "bun.lock", "bunfig.toml", "tsconfig.json"))
    value = OFFSET
    for path in sorted(inputs):
        for data in (path.relative_to(root).as_posix().encode(), path.read_bytes()):
            for byte in len(data).to_bytes(8, "little") + data:
                value = ((value ^ byte) * PRIME) & MASK
    return f"{value:032x}"


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    print(source_identity(parser.parse_args().root.resolve()))
