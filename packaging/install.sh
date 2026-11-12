#!/usr/bin/env bash
# Install already-built binaries; DESTDIR is a staging root, never part of Exec.
set -euo pipefail

if [[ ${1:-} == --help ]]; then
    printf '%s\n' 'Usage: PREFIX=/usr/local DESTDIR=/tmp/stage BINDIR=target/release bash packaging/install.sh' \
        'PREFIX is the absolute installation prefix; DESTDIR is an optional absolute staging root.' \
        'BINDIR is the directory containing the six already-built executables (default: target/release).'
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

install -d "$destdir$prefix/bin" "$destdir$prefix/share/applications" \
    "$destdir$prefix/share/icons/hicolor/scalable/apps" "$destdir$prefix/share/metainfo" \
    "$destdir$prefix/share/licenses/swatch-board"
for binary in swatch-board-picker swatch-board-tray swatch-board-history swatch-board-config swatch-board swatch-board-theme-worker; do
    install -m 755 "$bindir/$binary" "$destdir$prefix/bin/$binary"
done
install -m 644 "$root/share/icons/hicolor/scalable/apps/org.apartment38.SwatchBoard.svg" \
    "$destdir$prefix/share/icons/hicolor/scalable/apps/org.apartment38.SwatchBoard.svg"
install -m 644 "$root/share/metainfo/org.apartment38.SwatchBoard.Board.metainfo.xml" \
    "$destdir$prefix/share/metainfo/org.apartment38.SwatchBoard.Board.metainfo.xml"
install -m 644 "$root/LICENSE" "$destdir$prefix/share/licenses/swatch-board/LICENSE"
install -d "$destdir$prefix/share/swatch-board/plugins" "$destdir$prefix/share/swatch-board/schemas"
cp -R "$root/plugins/." "$destdir$prefix/share/swatch-board/plugins/"
cp -R "$root/schemas/." "$destdir$prefix/share/swatch-board/schemas/"

install_entry() {
    local source=$1 binary=$2 line
    local destination="$destdir$prefix/share/applications/${source##*/}"
    # The portal matches this entry's executable to the picker process. Using
    # the final absolute path also works when PREFIX/bin is not on PATH.
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
