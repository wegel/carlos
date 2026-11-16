#!/usr/bin/env python3
"""Download checksum-verified Apple Rust components when the build Mac is offline."""

from concurrent.futures import ThreadPoolExecutor
import hashlib
from pathlib import Path
import shutil
import tomllib
import urllib.request


VERSION = "1.100.0"
ROOT = Path(__file__).resolve().parents[1] / "target/ios-toolchain"
COMPONENTS = (
    ("rustc", "x86_64-apple-darwin"),
    ("cargo", "x86_64-apple-darwin"),
    ("rust-std", "x86_64-apple-darwin"),
    ("rust-std", "x86_64-apple-ios"),
    ("rust-std", "aarch64-apple-ios"),
    ("rust-std", "aarch64-apple-ios-sim"),
    ("llvm-tools-preview", "x86_64-apple-darwin"),
)


def download(component):
    """Reuse only an archive whose bytes match the official release manifest."""
    url, checksum = component["xz_url"], component["xz_hash"]
    archive = ROOT / url.rsplit("/", 1)[1]
    if archive.exists() and hashlib.sha256(archive.read_bytes()).hexdigest() == checksum:
        return
    data = urllib.request.urlopen(url, timeout=120).read()
    if hashlib.sha256(data).hexdigest() != checksum:
        raise ValueError(f"Rust component checksum mismatch: {archive.name}")
    archive.write_bytes(data)


def main():
    """Retain the official manifest beside the verified components."""
    ROOT.mkdir(parents=True, exist_ok=True)
    manifest = urllib.request.urlopen(
        f"https://static.rust-lang.org/dist/channel-rust-{VERSION}.toml", timeout=30
    ).read()
    (ROOT / f"channel-rust-{VERSION}.toml").write_bytes(manifest)
    parsed = tomllib.loads(manifest.decode())
    with ThreadPoolExecutor(max_workers=3) as executor:
        list(executor.map(download, [parsed["pkg"][name]["target"][target]
                                     for name, target in COMPONENTS]))
    shutil.copy2(Path(__file__).with_name("ios-install-toolchain.sh"), ROOT / "install.sh")
    print(ROOT)


if __name__ == "__main__":
    main()
