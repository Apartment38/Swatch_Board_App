#!/usr/bin/env bash
# Run inside a private D-Bus session with a private headless compositor.
set -euo pipefail
run=$1
binary=$2
mkdir -p "$run"/{runtime,config,data,state,cache,images}
chmod 700 "$run/runtime"
export XDG_RUNTIME_DIR="$run/runtime" WAYLAND_DISPLAY=swatch-test
export XDG_CONFIG_HOME="$run/config" XDG_DATA_HOME="$run/data"
export XDG_STATE_HOME="$run/state" XDG_CACHE_HOME="$run/cache"
weston --backend=headless-backend.so --socket="$WAYLAND_DISPLAY" \
  --idle-time=0 --width=960 --height=640 --no-config >"$run/weston.log" 2>&1 &
weston_pid=$!
cleanup() { kill "$weston_pid" 2>/dev/null || true; wait "$weston_pid" 2>/dev/null || true; }
trap cleanup EXIT
for _ in {1..100}; do
  if [[ -S $XDG_RUNTIME_DIR/$WAYLAND_DISPLAY ]]; then break; fi
  if ! kill -0 "$weston_pid" 2>/dev/null; then cat "$run/weston.log" >&2; exit 1; fi
  sleep .1
done
if [[ ! -S $XDG_RUNTIME_DIR/$WAYLAND_DISPLAY ]]; then
  cat "$run/weston.log" >&2
  echo 'Timed out waiting for the private Wayland compositor' >&2
  exit 1
fi
QT_QPA_PLATFORM=wayland SWATCH_BOARD_QT_SMOKE_DIR="$run/images" "$binary"
