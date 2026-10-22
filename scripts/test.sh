#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
profile=debug
check_codex=false
for argument in "$@"; do
  case "$argument" in
    --release) profile=release ;;
    --codex-contract) check_codex=true ;;
    *) printf 'usage: %s [--release] [--codex-contract]\n' "$0" >&2; exit 2 ;;
  esac
done

cargo fmt --check
cargo test --all-targets --quiet
if [[ "$profile" == release ]]; then
  cargo build --release
else
  cargo build
fi

if [[ ! -x target/ui-venv/bin/python ]]; then
  python3 -m venv target/ui-venv
fi
target/ui-venv/bin/python -m pip install --disable-pip-version-check -q \
  -r tests/ui/requirements.txt
mkdir -p target/ui-artifacts
artifacts="$(mktemp -d "$PWD/target/ui-artifacts/run-XXXXXX")"
printf 'Test artifacts: %s\n' "$artifacts"
target/ui-venv/bin/python tests/ui/test_workflows.py \
  --binary "target/$profile/carlos" --artifacts "$artifacts/ui" -v
if [[ "$check_codex" == true ]]; then
  python3 tests/ui/test_codex_contract.py --artifacts "$artifacts/codex"
  target/ui-venv/bin/python tests/ui/test_native_monitor.py \
    --binary "target/$profile/carlos" --artifacts "$artifacts/native-monitor" -v
fi
