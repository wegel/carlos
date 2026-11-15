#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
profile=debug
check_codex=false
check_claude=false
for argument in "$@"; do
  case "$argument" in
    --release) profile=release ;;
    --codex-contract) check_codex=true ;;
    --claude-contract) check_claude=true ;;
    *) printf 'usage: %s [--release] [--codex-contract] [--claude-contract]\n' "$0" >&2; exit 2 ;;
  esac
done

python3 -m unittest discover -s tests/architecture -p 'test_*.py'
python3 scripts/check-core-dependencies.py
cargo fmt --all --check
if [[ "$profile" == release ]]; then
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace --release --all-targets --quiet
  cargo build --release
else
  cargo test --workspace --all-targets --quiet
  cargo build
fi

# The Claude terminal fixtures exercise the compiled SDK controller, never a stub helper.
bash scripts/build-claude-controller.sh

if [[ ! -x target/ui-venv/bin/python ]]; then
  python3 -m venv target/ui-venv
fi
target/ui-venv/bin/python -m pip install --disable-pip-version-check -q \
  -r tests/ui/requirements.txt
target/ui-venv/bin/python tests/ui/test_socket_recorder.py
target/ui-venv/bin/python tests/ui/test_terminal_screen.py
mkdir -p target/ui-artifacts
artifacts="$(mktemp -d "$PWD/target/ui-artifacts/run-XXXXXX")"
printf 'Test artifacts: %s\n' "$artifacts"
target/ui-venv/bin/python tests/daemon/test_daemon.py \
  --binary "target/$profile/carlosd" --artifacts "$artifacts/daemon" -v
release_target="$(python3 -c 'import json; print(json.load(open("target/claude-controller/claude-controller-build.json"))["rustTarget"])')"
python3 scripts/package-release.py --target "$release_target" --binary "target/$profile/carlos" \
  --helper target/claude-controller/carlos-claude-controller --output "$artifacts/package"
python3 tests/package/smoke.py --binary "$artifacts/package/carlos" \
  --helper "$artifacts/package/carlos-claude-controller" --artifacts "$artifacts/package-smoke"
target/ui-venv/bin/python tests/ui/test_workflows.py \
  --binary "target/$profile/carlos" --artifacts "$artifacts/ui" -v
# The small committed oracle keeps paged selection checks in CI without an old executable.
target/ui-venv/bin/python tests/ui/test_daemon_selection_copy.py \
  --binary "target/$profile/carlos" --daemon "target/$profile/carlosd" \
  --artifacts "$artifacts/selection"
target/ui-venv/bin/python tests/ui/test_daemon_source_failures.py \
  --binary "target/$profile/carlos" --daemon "target/$profile/carlosd" \
  --artifacts "$artifacts/source-failures"
target/ui-venv/bin/python tests/ui/test_daemon_dictation.py \
  --binary "target/$profile/carlos" --daemon "target/$profile/carlosd" \
  --artifacts "$artifacts/dictation"
if [[ "$check_codex" == true ]]; then
  python3 tests/ui/test_codex_contract.py --artifacts "$artifacts/codex"
  target/ui-venv/bin/python tests/ui/test_native_monitor.py \
    --binary "target/$profile/carlos" --artifacts "$artifacts/native-monitor" -v
fi

if [[ "$check_claude" == true ]]; then
  target/ui-venv/bin/python tests/ui/test_native_claude.py \
    --binary "$artifacts/package/carlos" --helper "$artifacts/package/carlos-claude-controller" \
    --artifacts "$artifacts/native-claude" -v
fi
