#!/usr/bin/env bash
# Install already-built binaries; DESTDIR is a staging root, never part of Exec.
set -euo pipefail

if [[ ${1:-} == --help ]]; then
    printf '%s\n' 'Usage: PREFIX=/usr/local DESTDIR=/tmp/stage BINDIR=target/release bash packaging/install.sh' \
        'PREFIX is the absolute installation prefix; DESTDIR is an optional absolute staging root.' \
        'BINDIR is the directory containing the six already-built executables (default: target/release).' \
        'RUST_LICENSE_DIR is the build toolchain share/doc/rust directory; defaults to the active rustc sysroot.' \
        'EFFECT_LIB is the built KWin effect (default: target/kwin/lib/swatchboardpicker.so).' \
        'QT_PLUGIN_DIR is the absolute Qt plugin root; otherwise discover it from Qt.'
    exit 0
fi
if (( $# != 0 )); then
    printf '%s\n' 'Use --help for usage.' >&2
    exit 2
fi

root=$(dirname "$(dirname "$(realpath "${BASH_SOURCE[0]}")")")
prefix=${PREFIX:-/usr/local}
destdir=${DESTDIR:-}
bindir=${BINDIR:-"$root/target/release"}
effect_lib=${EFFECT_LIB:-"$root/target/kwin/lib/swatchboardpicker.so"}
[[ $prefix == /* && ( -z $destdir || $destdir == /* ) ]] || {
    printf '%s\n' 'PREFIX and nonempty DESTDIR must be absolute paths.' >&2
    exit 2
}
# Desktop Exec values have a separate quoting language. Reject its special
# characters rather than write an entry that might execute a different path.
case "$prefix" in
    *$'\n'*|*$'\r'*|*$'\t'*|*'"'*|*'\'*|*'`'*|*'$'*|*'%'*)
        printf '%s\n' 'PREFIX must not contain control characters, quotes, backslashes, backticks, dollar signs or percent signs.' >&2
        exit 2 ;;
esac
while [[ $prefix != / && $prefix == */ ]]; do prefix=${prefix%/}; done
[[ $prefix == / ]] && prefix=''
while [[ $destdir != / && $destdir == */ ]]; do destdir=${destdir%/}; done
[[ $destdir == / ]] && destdir=''

for binary in swatch-board-picker swatch-board-tray swatch-board-history swatch-board-config swatch-board swatch-board-theme-worker; do
    [[ -f $bindir/$binary && -x $bindir/$binary ]] || {
        printf 'Missing executable: %s\n' "$bindir/$binary" >&2
        exit 1
    }
done
[[ -f $effect_lib ]] || { printf 'Missing KWin effect: %s\n' "$effect_lib" >&2; exit 1; }
qt_plugin_dir=${QT_PLUGIN_DIR:-}
if [[ -z $qt_plugin_dir ]]; then
    if command -v qtpaths6 >/dev/null; then
        qt_plugin_dir=$(qtpaths6 --plugin-dir)
    elif command -v qmake6 >/dev/null; then
        qt_plugin_dir=$(qmake6 -query QT_INSTALL_PLUGINS)
    elif command -v pkg-config >/dev/null; then
        qt_bindir=$(pkg-config --variable=bindir Qt6Core)
        if [[ -n $qt_bindir && -x $qt_bindir/qtpaths6 ]]; then
            qt_plugin_dir=$("$qt_bindir/qtpaths6" --plugin-dir)
        elif [[ -n $qt_bindir && -x $qt_bindir/qtpaths ]]; then
            qt_plugin_dir=$("$qt_bindir/qtpaths" --plugin-dir)
        fi
    fi
fi
[[ $qt_plugin_dir == /* ]] || {
    printf '%s\n' 'Cannot discover an absolute Qt plugin directory; set QT_PLUGIN_DIR.' >&2
    exit 1
}


# Rust's standard library is linked into the executables and has notices outside
# Cargo.lock. Use the documentation from the *same* toolchain used to build.
rust_license_dir=${RUST_LICENSE_DIR:-}
if [[ -z $rust_license_dir ]]; then
    command -v rustc >/dev/null || { printf '%s\n' 'Set RUST_LICENSE_DIR to the build toolchain share/doc/rust directory.' >&2; exit 1; }
    rust_license_dir="$(rustc --print sysroot)/share/doc/rust"
fi
[[ -s $rust_license_dir/COPYRIGHT-library.html && -s $rust_license_dir/licenses/MIT.txt && -s $rust_license_dir/licenses/Apache-2.0.txt ]] || {
    printf 'Missing Rust standard library notices in %s\n' "$rust_license_dir" >&2
    exit 1
}

install -d "$destdir$prefix/bin" "$destdir$prefix/share/applications" \
    "$destdir$prefix/share/icons/hicolor/scalable/apps" "$destdir$prefix/share/metainfo" \
    "$destdir$prefix/share/licenses/swatch-board"
install -d "$destdir$qt_plugin_dir/kwin/effects/plugins"
install -m 755 "$effect_lib" "$destdir$qt_plugin_dir/kwin/effects/plugins/swatchboardpicker.so"
install -m 644 "$root/kwin/LICENSE" "$destdir$prefix/share/licenses/swatch-board/KWIN-LICENSE"
for binary in swatch-board-picker swatch-board-tray swatch-board-history swatch-board-config swatch-board swatch-board-theme-worker; do
    install -m 755 "$bindir/$binary" "$destdir$prefix/bin/$binary"
done
install -m 644 "$root/share/icons/hicolor/scalable/apps/org.apartment38.SwatchBoard.svg" \
    "$destdir$prefix/share/icons/hicolor/scalable/apps/org.apartment38.SwatchBoard.svg"
install -m 644 "$root/share/metainfo/org.apartment38.SwatchBoard.Board.metainfo.xml" \
    "$destdir$prefix/share/metainfo/org.apartment38.SwatchBoard.Board.metainfo.xml"
install -m 644 "$root/LICENSE" "$destdir$prefix/share/licenses/swatch-board/LICENSE"
install -m 644 "$root/THIRD_PARTY_NOTICES.txt" "$destdir$prefix/share/licenses/swatch-board/THIRD_PARTY_NOTICES.txt"
install -d "$destdir$prefix/share/licenses/swatch-board/rust-standard-library"
install -m 644 "$rust_license_dir/COPYRIGHT-library.html" "$destdir$prefix/share/licenses/swatch-board/rust-standard-library/COPYRIGHT-library.html"
cp -R "$rust_license_dir/licenses" "$destdir$prefix/share/licenses/swatch-board/rust-standard-library/"
chmod -R a+rX "$destdir$prefix/share/licenses/swatch-board/rust-standard-library"
install -d "$destdir$prefix/share/swatch-board/plugins" "$destdir$prefix/share/swatch-board/schemas"
cp -R "$root/plugins/." "$destdir$prefix/share/swatch-board/plugins/"
cp -R "$root/schemas/." "$destdir$prefix/share/swatch-board/schemas/"

install_entry() {
    local source=$1 binary=$2 line
    local destination="$destdir$prefix/share/applications/${source##*/}"
    # Use the final absolute path even when PREFIX/bin is not on PATH.
    while IFS= read -r line || [[ -n $line ]]; do
        if [[ $line == Exec=* ]]; then
            printf 'Exec="%s/bin/%s"\n' "$prefix" "$binary"
        else
            printf '%s\n' "$line"
        fi
    done < "$source" > "$destination"
    chmod 644 "$destination"
}
install_entry "$root/share/applications/org.apartment38.SwatchBoard.desktop" swatch-board-picker
install_entry "$root/packaging/org.apartment38.SwatchBoard.Tray.desktop" swatch-board-tray
install_entry "$root/packaging/org.apartment38.SwatchBoard.History.desktop" swatch-board-history
install_entry "$root/packaging/org.apartment38.SwatchBoard.Config.desktop" swatch-board-config
install_entry "$root/packaging/org.apartment38.SwatchBoard.Board.desktop" swatch-board
