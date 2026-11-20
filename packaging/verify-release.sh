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
cmake -S kwin -B target/kwin -DCMAKE_BUILD_TYPE=Release
cmake --build target/kwin
python3 -m unittest discover -s packaging -p 'test_*.py'
PREFIX=/usr DESTDIR="$stage" BINDIR="$root/target/release" QT_PLUGIN_DIR=/usr/lib/qt6/plugins bash packaging/install.sh
cmp "$root/target/kwin/lib/swatchboardpicker.so" "$stage/usr/lib/qt6/plugins/kwin/effects/plugins/swatchboardpicker.so"
cmp "$root/kwin/LICENSE" "$stage/usr/share/licenses/swatch-board/KWIN-LICENSE"
bin="$stage/usr/bin"
for name in swatch-board-picker swatch-board-tray swatch-board-history \
            swatch-board-config swatch-board swatch-board-theme-worker; do
    test -x "$bin/$name"
done
"$bin/swatch-board-picker" --version | grep -Fx "Swatch Board Picker 0.1.0"
"$bin/swatch-board-picker" --help >/dev/null
rust_license_dir=${RUST_LICENSE_DIR:-}
rust_copyright=''
rust_licenses_dir=''
if [[ -n $rust_license_dir ]]; then
    rust_copyright="$rust_license_dir/COPYRIGHT-library.html"
    rust_licenses_dir="$rust_license_dir/licenses"
else
    rust_sysroot=$(rustc --print sysroot)
    while [[ $rust_sysroot != / && $rust_sysroot == */ ]]; do rust_sysroot=${rust_sysroot%/}; done
    if [[ -s $rust_sysroot/share/doc/rust/COPYRIGHT-library.html && -s $rust_sysroot/share/doc/rust/licenses/MIT.txt && -s $rust_sysroot/share/doc/rust/licenses/Apache-2.0.txt ]]; then
        rust_copyright="$rust_sysroot/share/doc/rust/COPYRIGHT-library.html"
        rust_licenses_dir="$rust_sysroot/share/doc/rust/licenses"
    else
        if [[ -s $rust_sysroot/share/licenses/rust/COPYRIGHT-library.html.rustc ]]; then
            rust_copyright="$rust_sysroot/share/licenses/rust/COPYRIGHT-library.html.rustc"
        elif [[ -s $rust_sysroot/share/licenses/rust/COPYRIGHT-library.html ]]; then
            rust_copyright="$rust_sysroot/share/licenses/rust/COPYRIGHT-library.html"
        fi
        rust_licenses_dir="$rust_sysroot/share/doc/rustc/licenses"
    fi
fi
[[ -n $rust_copyright && -s $rust_copyright && -s $rust_licenses_dir/MIT.txt && -s $rust_licenses_dir/Apache-2.0.txt ]] || {
    printf '%s\n' 'Missing Rust standard library notices for release verification.' >&2
    exit 1
}
python3 - "$stage" "$root" "$rust_copyright" "$rust_licenses_dir" <<'PY'
import json
from pathlib import Path
import subprocess
import sys

prefix = Path(sys.argv[1]) / "usr"
source = Path(sys.argv[2])
rust_copyright = Path(sys.argv[3])
rust_licenses = Path(sys.argv[4])
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
rust_installed = prefix / "share/licenses/swatch-board/rust-standard-library"
if (rust_installed / "COPYRIGHT-library.html").read_bytes() != rust_copyright.read_bytes():
    raise RuntimeError("installed Rust notice differs: COPYRIGHT-library.html")
expected_rust = {p.relative_to(rust_licenses) for p in rust_licenses.rglob("*") if p.is_file()}
actual_rust = {p.relative_to(rust_installed / "licenses") for p in (rust_installed / "licenses").rglob("*") if p.is_file()}
if actual_rust != expected_rust:
    raise RuntimeError(f"installed Rust licenses inventory mismatch: {actual_rust ^ expected_rust}")
for relative in expected_rust:
    if (rust_installed / "licenses" / relative).read_bytes() != (rust_licenses / relative).read_bytes():
        raise RuntimeError(f"installed Rust notice differs: licenses/{relative}")
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
