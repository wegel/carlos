#!/usr/bin/env bash
# Stage a phone-only offline source bundle, then build/test on the owned Mac.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
# shellcheck disable=SC1090
source "${MAC_BUILD_ENV:-$ROOT/etc/mac-build.env}"
: "${MAC_BUILD_DIR:=carlos-ios-build}"
if [[ "$MAC_BUILD_DIR" == /* || "$MAC_BUILD_DIR" == *..* || ! "$MAC_BUILD_DIR" =~ ^[a-zA-Z0-9/_-]+$ ]]; then
    echo "MAC_BUILD_DIR must stay below the Mac home" >&2
    exit 2
fi
MAC=(bash scripts/ios-mac.sh)
if [[ -n "${CARLOS_IOS_SOURCE:-}" ]] && \
    { [[ ! "$CARLOS_IOS_SOURCE" =~ ^[a-f0-9]{20}$ ]] || [[ ! -d "target/ios-source/$CARLOS_IOS_SOURCE" ]]; }; then
    echo "CARLOS_IOS_SOURCE must name an existing 20-hex source snapshot" >&2
    exit 2
fi
mkdir -p target/ios-artifacts
# Every build/test/launch shares the owned simulator and Mac compiler cache.
exec 9> target/ios-artifacts/mac-build.lock

toolchain() {
    flock 9
    python3 scripts/ios-toolchain.py
    "${MAC[@]}" sync target/ios-toolchain toolchains/downloads
    "${MAC[@]}" ssh "bash ~/$MAC_BUILD_DIR/toolchains/downloads/install.sh ~/$MAC_BUILD_DIR"
}

run() {
    # Wait inside a parsed function, so queued builds cannot resume reading an edited script mid-file.
    flock 9
    local directory identity simulator dependencies
    if [[ -n "${CARLOS_IOS_SOURCE:-}" ]]; then directory="$ROOT/target/ios-source/$CARLOS_IOS_SOURCE"
    else directory="$(python3 scripts/ios-prepare.py)"; fi
    identity="$(basename "$directory")"
    printf 'source: %s\n' "$identity"
    dependencies="$(cat "$directory/dependencies-id")"
    [[ "$dependencies" =~ ^[a-f0-9]{20}$ ]] || { echo "invalid dependency bundle" >&2; exit 2; }
    "${MAC[@]}" sync "target/ios-dependencies/$dependencies" "ios-dependencies/$dependencies"
    "${MAC[@]}" sync "$directory" "sources/$identity"
    local remote="bash ~/$MAC_BUILD_DIR/sources/$identity/ios-remote.sh"
    printf '%s\n' "$identity" > target/ios-artifacts/current-source
    "${MAC[@]}" ssh "$remote framework"
    [[ "$1" == framework ]] && return
    "${MAC[@]}" ssh "$remote build"
    [[ "$1" == build ]] && return
    simulator="$("${MAC[@]}" sim-create)"
    [[ "$simulator" =~ ^[A-F0-9-]{36}$ ]] || { echo "invalid Carlos simulator UUID" >&2; exit 2; }
    "${MAC[@]}" ssh "$remote $1 $simulator ${2:-}"
}

case "${1:-}" in
check) "${MAC[@]}" check ;;
toolchain) toolchain ;;
framework|build|test|launch|ui-test)
    fixture="${2:-}"
    if [[ "$1" == ui-test && ( ! "$fixture" =~ ^fixture/[a-zA-Z0-9_-]+$ || "$fixture" == *..* ) ]]; then
        echo "ui-test needs an owned fixture/<run> directory" >&2
        exit 2
    fi
    run "$1" "$fixture" 2>&1 | tee "target/ios-artifacts/$1-$(date -u +%Y%m%dT%H%M%S).log"
    ;;
*) echo "usage: ios-build.sh check|toolchain|framework|build|test|launch" >&2; exit 2 ;;
esac
