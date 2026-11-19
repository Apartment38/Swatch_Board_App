#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --workspace --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all -- --check
python3 -m unittest discover -s packaging -p 'test_*.py'

# Each UI run gets its own bus and XDG data; neither reads nor writes the
# invoking user's palette, settings, clipboard, or running tray.
root=$(mktemp -d "${TMPDIR:-/tmp}/swatch-tests.XXXXXX")
cleanup() {
  if [[ -n ${SWATCH_TEST_ARTIFACTS:-} ]]; then
    for mode in board config wayland-board wayland-config; do
      if [[ -d $root/$mode/images ]]; then
        mkdir -p "$SWATCH_TEST_ARTIFACTS/$mode"
        cp -a "$root/$mode/images/." "$SWATCH_TEST_ARTIFACTS/$mode/"
        if [[ -f $root/$mode/weston.log ]]; then
          cp "$root/$mode/weston.log" "$SWATCH_TEST_ARTIFACTS/$mode/"
        fi
      fi
    done
  fi
  rm -rf "$root"
}
trap cleanup EXIT
# Exercise actual Qt wheel key and mouse event handlers independently of the
# model preview test. Qt's offscreen backend avoids a host desktop connection.
c++ -std=c++17 -fPIC testing/wheel-test.cpp -o "$root/wheel-test" \
  $(pkg-config --cflags --libs Qt6Widgets)
mkdir -p "$root/wheel-cache"
QT_QPA_PLATFORM=offscreen XDG_CACHE_HOME="$root/wheel-cache" "$root/wheel-test"
for mode in board config; do
  run="$root/$mode"
  mkdir -p "$run"/{config,data,state,cache,images}
  binary=swatch-board
  if [[ $mode == config ]]; then binary=swatch-board-config; fi
  timeout 60s dbus-run-session -- env \
    QT_QPA_PLATFORM=offscreen \
    XDG_CONFIG_HOME="$run/config" XDG_DATA_HOME="$run/data" \
    XDG_STATE_HOME="$run/state" XDG_CACHE_HOME="$run/cache" \
    SWATCH_BOARD_QT_SMOKE_DIR="$run/images" \
    "${CARGO_TARGET_DIR:-target}/debug/$binary"
  python3 - "$run/images/$mode-result.json" <<'PY'
import json
import pathlib
import sys
path = pathlib.Path(sys.argv[1])
result = json.loads(path.read_text())
if result.get('failures') != []:
    raise SystemExit(f'{path}: unexpected smoke result: {result!r}')
print(f'{path.name}: passed')
PY
  wayland_run="$root/wayland-$mode"
  timeout 90s dbus-run-session -- bash testing/wayland-session.sh \
    "$wayland_run" "${CARGO_TARGET_DIR:-target}/debug/$binary"
  python3 - "$wayland_run/images/$mode-result.json" <<'PY'
import json
import pathlib
import sys
path = pathlib.Path(sys.argv[1])
result = json.loads(path.read_text())
if result.get('failures') != []:
    raise SystemExit(f'{path}: unexpected Wayland smoke result: {result!r}')
print(f'Wayland {path.name}: passed')
PY
done
