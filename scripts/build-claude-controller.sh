#!/usr/bin/env bash
# Build only when explicitly requested; the installed helper never downloads dependencies.
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec python3 "$repo_root/scripts/build-claude-controller.py" "$@"
