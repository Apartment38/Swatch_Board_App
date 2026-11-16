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
    for mode in board config; do
      if [[ -d $root/$mode/images ]]; then
        mkdir -p "$SWATCH_TEST_ARTIFACTS/$mode"
        cp -a "$root/$mode/images/." "$SWATCH_TEST_ARTIFACTS/$mode/"
      fi
    done
  fi
  rm -rf "$root"
}
trap cleanup EXIT
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
done
