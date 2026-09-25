#!/usr/bin/env bash
# End-to-end check against a running compositor that speaks
# ext-data-control-v1 ($WAYLAND_DISPLAY). Needs wl-clipboard. Not run by cargo.
set -euo pipefail
: "${WAYLAND_DISPLAY:?start a compositor and export WAYLAND_DISPLAY first}"
cd "$(dirname "$0")/.."

cargo build --quiet
log=$(mktemp)
RUST_LOG=${RUST_LOG:-debug} target/debug/crownos-clipboard >"$log" 2>&1 &
daemon=$!
trap 'kill "$daemon" 2>/dev/null || true; rm -f "$log"' EXIT

fail() {
    echo "FAIL: $*"
    cat "$log"
    exit 1
}

sleep 0.3
kill -0 "$daemon" 2>/dev/null || fail "the daemon did not start"

copy_then_exit() {
    wl-copy --foreground "$1" &
    local copier=$!
    sleep 0.3
    kill "$copier"
    wait "$copier" 2>/dev/null || true
    sleep 0.2
}

for text in hello world; do
    copy_then_exit "$text"
    pasted=$(timeout 2 wl-paste) || fail "wl-paste found no clipboard after '$text'"
    [[ $pasted == "$text" ]] || fail "expected '$text', pasted '$pasted'"
    echo "ok: '$text' outlived the client that copied it"
done

kill -0 "$daemon" 2>/dev/null || fail "the daemon died"
echo "PASS"
