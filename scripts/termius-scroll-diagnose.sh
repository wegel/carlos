#!/usr/bin/env bash
set -euo pipefail

LOG_FILE="${1:-/tmp/carlos-termius-scroll-diag.log}"
CAPTURE_SECS="${CAPTURE_SECS:-8}"
PAUSE_SECS="${PAUSE_SECS:-2}"
TTY_IN="/dev/tty"
TTY_OUT="/dev/tty"

if [[ ! -r "$TTY_IN" ]]; then
  echo "error: $TTY_IN is not readable" >&2
  exit 1
fi

mkdir -p "$(dirname "$LOG_FILE")"
: >"$LOG_FILE"

TMP_DIR="$(mktemp -d /tmp/carlos-termdiag.XXXXXX)"
ORIG_STTY="$(stty -g <"$TTY_IN")"

reset_modes() {
  # Disable mouse capture, alternate scroll, bracketed paste.
  printf '\e[?1006l\e[?1015l\e[?1003l\e[?1002l\e[?1000l\e[?1007l\e[?2004l' >"$TTY_OUT" || true
}

cleanup() {
  stty "$ORIG_STTY" <"$TTY_IN" 2>/dev/null || true
  reset_modes
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT INT TERM

count_matches() {
  local regex="$1"
  local text="$2"
  (grep -E -o "$regex" <<<"$text" || true) | wc -l | tr -d ' '
}

capture_phase() {
  local id="$1"
  local title="$2"
  local on_seq="$3"
  local off_seq="$4"

  local bin="$TMP_DIR/${id}.bin"

  printf '\n[%s] %s\n' "$id" "$title" >"$TTY_OUT"
  printf '  Preparing in %ss... then swipe/scroll for %ss.\n' "$PAUSE_SECS" "$CAPTURE_SECS" >"$TTY_OUT"

  printf '%b' "$on_seq" >"$TTY_OUT"
  sleep "$PAUSE_SECS"

  stty -echo -icanon min 0 time 1 <"$TTY_IN"
  timeout "${CAPTURE_SECS}s" cat <"$TTY_IN" >"$bin" || true
  stty "$ORIG_STTY" <"$TTY_IN"

  printf '%b' "$off_seq" >"$TTY_OUT"

  local bytes hex visible
  bytes="$(wc -c <"$bin" | tr -d ' ')"
  hex="$(od -An -tx1 -v "$bin" | tr '\n' ' ' | tr -s ' ' | sed 's/^ //; s/ $//')"
  visible="$(cat -v "$bin" | sed -e ':a' -e 'N' -e '$!ba' -e 's/\n/\\n/g')"

  local sgr_count arrow_up_count arrow_down_count
  sgr_count="$(count_matches '1b 5b 3c' "$hex")"
  arrow_up_count="$(count_matches '1b 5b 41' "$hex")"
  arrow_down_count="$(count_matches '1b 5b 42' "$hex")"

  {
    echo "=== phase ${id}: ${title} ==="
    echo "on_seq=${on_seq@Q}"
    echo "off_seq=${off_seq@Q}"
    echo "bytes=${bytes}"
    echo "mouse_sgr_sequences=${sgr_count}"
    echo "arrow_up_sequences=${arrow_up_count}"
    echo "arrow_down_sequences=${arrow_down_count}"
    echo "visible=${visible}"
    echo "hex=${hex}"
    echo
  } >>"$LOG_FILE"

  printf '  captured %s bytes (mouse_sgr=%s, up=%s, down=%s)\n' \
    "$bytes" "$sgr_count" "$arrow_up_count" "$arrow_down_count" >"$TTY_OUT"
}

{
  echo "Carlos Termius Scroll Diagnostic"
  echo "timestamp=$(date -Is)"
  echo "host=$(hostname)"
  echo "uname=$(uname -a)"
  echo "term=${TERM:-}"
  echo "term_program=${TERM_PROGRAM:-}"
  echo "ssh_tty=${SSH_TTY:-}"
  echo "ssh_connection=${SSH_CONNECTION:-}"
  echo "ssh_client=${SSH_CLIENT:-}"
  echo "stty_before=$(stty -a <"$TTY_IN" | tr '\n' ' ')"
  echo
} >>"$LOG_FILE"

reset_modes
printf '\nCarlos scroll diagnostic started.\n' >"$TTY_OUT"
printf 'Log file: %s\n' "$LOG_FILE" >"$TTY_OUT"
printf 'Use your normal swipe/scroll gestures when each phase starts.\n' >"$TTY_OUT"
printf 'Press Ctrl+C to abort early.\n' >"$TTY_OUT"

capture_phase "baseline" "No special modes" "" ""
capture_phase "mouse" "Mouse capture (1000/1002/1003/1006)" "\e[?1000h\e[?1002h\e[?1003h\e[?1006h" "\e[?1006l\e[?1003l\e[?1002l\e[?1000l"
capture_phase "alt_scroll" "Alternate scroll only (1007)" "\e[?1007h" "\e[?1007l"
capture_phase "mouse_plus_alt" "Mouse capture + alternate scroll" "\e[?1000h\e[?1002h\e[?1003h\e[?1006h\e[?1007h" "\e[?1007l\e[?1006l\e[?1003l\e[?1002l\e[?1000l"

printf '\nDone. Diagnostic log written to %s\n' "$LOG_FILE" >"$TTY_OUT"
printf 'Share: tail -n 200 %s\n\n' "$LOG_FILE" >"$TTY_OUT"
