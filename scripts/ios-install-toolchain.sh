#!/usr/bin/env bash
# Install verified, locally relayed Rust components below the Carlos build directory.
set -euo pipefail
cd "${1:?pass the owned Carlos build directory}"
mkdir -p toolchains/unpacked
for archive in toolchains/downloads/*.tar.xz; do
    component="$(basename "$archive" .tar.xz)"
    tar -xf "$archive" -C toolchains/unpacked
    bash "toolchains/unpacked/$component/install.sh" \
        --prefix="$PWD/toolchains/rust-1.100.0" --disable-ldconfig
done
toolchains/rust-1.100.0/bin/rustc --version
