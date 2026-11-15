#!/usr/bin/env bash
# Noninteractive release gate: no GUI, D-Bus, portal, or live configuration writes.
set -euo pipefail
for validator in desktop-file-validate appstreamcli; do
    command -v "$validator" >/dev/null || {
        printf 'Missing required release validator: %s\n' "$validator" >&2
        exit 1
    }
done
root=$(dirname "$(dirname "$(realpath "${BASH_SOURCE[0]}")")")
stage=$(mktemp -d "${TMPDIR:-/tmp}/swatch-release.XXXXXX")
trap 'rm -rf "$stage"' EXIT
cd "$root"
cargo build --release --offline --workspace --locked
python3 -m unittest discover -s packaging -p 'test_*.py'
PREFIX=/usr DESTDIR="$stage" BINDIR="$root/target/release" bash packaging/install.sh
bin="$stage/usr/bin"
for name in swatch-board-picker swatch-board-tray swatch-board-history \
            swatch-board-config swatch-board swatch-board-theme-worker; do
    test -x "$bin/$name"
done
"$bin/swatch-board-picker" --version | grep -Fx "Swatch Board Picker 0.1.0"
"$bin/swatch-board-picker" --help >/dev/null
python3 - "$stage" "$root" <<'PY'
import json
from pathlib import Path
import subprocess
import sys

prefix = Path(sys.argv[1]) / "usr"
source = Path(sys.argv[2])
# Check all shipped plugin/schema bytes, not merely their directory names.
for category in ("plugins", "schemas"):
    original = source / category
    installed = prefix / "share/swatch-board" / category
    expected = {p.relative_to(original) for p in original.rglob("*") if p.is_file()}
    actual = {p.relative_to(installed) for p in installed.rglob("*") if p.is_file()}
    if actual != expected:
        raise RuntimeError(f"{category} inventory mismatch: {actual ^ expected}")
    for relative in expected:
        if (original / relative).read_bytes() != (installed / relative).read_bytes():
            raise RuntimeError(f"{category}/{relative}: installed bytes differ")
if (prefix / "share/licenses/swatch-board/LICENSE").read_bytes() != (source / "LICENSE").read_bytes():
    raise RuntimeError("installed license differs")
if (prefix / "share/licenses/swatch-board/THIRD_PARTY_NOTICES.txt").read_bytes() != (source / "THIRD_PARTY_NOTICES.txt").read_bytes():
    raise RuntimeError("installed third-party notices differ")
for name in ("ghostty", "zed", "kde", "example-terminal"):
    plugin = prefix / "share/swatch-board/plugins" / name
    manifest = json.loads((plugin / "manifest.json").read_text())
    roles = {role["id"]: role["default"] for role in json.loads((plugin / "roles.json").read_text())}
    roles.update(json.loads((plugin / "tests/sample-palette.json").read_text()))
    request = {
        "version": 1,
        "outputs": manifest["outputs"],
        "templates": {output["template"]: (plugin / output["template"]).read_text()
                      for output in manifest["outputs"]},
        "roles": roles,
    }
    response = subprocess.run(
        [str(prefix / "bin/swatch-board-theme-worker"), "--protocol-v1"],
        input=json.dumps(request), text=True, capture_output=True, check=True, timeout=5,
    )
    actual = json.loads(response.stdout)
    expected = (plugin / "tests/expected-theme.txt").read_text()
    if actual["version"] != 1 or actual["files"] != [
        {"path": manifest["outputs"][0]["path"], "content": expected}
    ]:
        raise RuntimeError(f"{name}: rendered response differs: {actual}")
    print(f"staged theme worker rendered installed {name} golden output")
PY
for entry in "$stage/usr/share/applications/"*.desktop; do
    desktop-file-validate "$entry"
done
appstreamcli validate --no-net "$stage/usr/share/metainfo/org.apartment38.SwatchBoard.Board.metainfo.xml"
printf '%s\n' 'staged release verification passed'
