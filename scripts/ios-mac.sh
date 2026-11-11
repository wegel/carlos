#!/usr/bin/env bash
# Runs commands on the remote Mac that builds the carlos iPhone app.
#
# Settings come from etc/mac-build.env (ignored by git). See docs/ios-build.md.
#
# Usage:
#   scripts/ios-mac.sh check              show macOS, Xcode, simulator, free disk
#   scripts/ios-mac.sh ssh <command...>   run one command on the Mac
#   scripts/ios-mac.sh sim-create         create the carlos simulator if missing
#   scripts/ios-mac.sh sim-udid           print the carlos simulator UDID
#   scripts/ios-mac.sh sync <local-dir>   copy an app source directory to the Mac
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="${MAC_BUILD_ENV:-$REPO_ROOT/etc/mac-build.env}"
if [[ ! -f "$ENV_FILE" ]]; then
    echo "missing $ENV_FILE; see docs/ios-build.md" >&2
    exit 2
fi
# shellcheck disable=SC1090
source "$ENV_FILE"

: "${MAC_BUILD_HOST:?set MAC_BUILD_HOST in $ENV_FILE}"
: "${MAC_BUILD_DIR:=carlos-ios-build}"
: "${IOS_SIMULATOR_NAME:=carlos iPhone}"
: "${IOS_SIMULATOR_TYPE:=iPhone 17 Pro}"

# ControlMaster=no: a stale shared SSH connection makes rsync and xcodebuild hang.
SSH_OPTS=(-o ControlMaster=no -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null
    -o LogLevel=ERROR -o ConnectTimeout=10)
if [[ -n "${MAC_SSH_KEY:-}" ]]; then
    SSH_OPTS+=(-i "$MAC_SSH_KEY")
fi

# Standard input is closed so a remote command cannot swallow the caller's input.
mac() { ssh "${SSH_OPTS[@]}" "$MAC_BUILD_HOST" "$@" </dev/null; }

sim_udid() {
    mac "xcrun simctl list devices available" |
        awk -v name="$IOS_SIMULATOR_NAME" 'index($0, name " (") { match($0, /\([0-9A-F-]{36}\)/); print substr($0, RSTART + 1, RLENGTH - 2); exit }'
}

case "${1:-}" in
check)
    mac "sw_vers -productVersion; uname -m; xcodebuild -version | head -1; df -h ~ | tail -1"
    echo "simulator: $IOS_SIMULATOR_NAME $(sim_udid || true)"
    ;;
ssh)
    shift
    mac "$@"
    ;;
sim-create)
    udid="$(sim_udid)"
    if [[ -z "$udid" ]]; then
        udid="$(mac "xcrun simctl create '$IOS_SIMULATOR_NAME' '$IOS_SIMULATOR_TYPE'")"
    fi
    echo "$udid"
    ;;
sim-udid)
    sim_udid
    ;;
sync)
    src="${2:?usage: ios-mac.sh sync <local-dir>}"
    mac "mkdir -p ~/$MAC_BUILD_DIR"
    # No --delete: Xcode writes generated files into the build directory on the Mac.
    rsync -a --exclude build --exclude DerivedData --exclude .git \
        -e "ssh ${SSH_OPTS[*]}" "$src/" "$MAC_BUILD_HOST:$MAC_BUILD_DIR/"
    ;;
*)
    sed -n '2,12p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
    exit 2
    ;;
esac
