#!/bin/sh
# Installs KaKa from a release tarball (or a source checkout after `cargo build --release`).
#
#   ./install.sh                     # for the current user, into ~/.local
#   sudo ./install.sh --prefix /usr/local
#   ./install.sh --uninstall         # same --prefix as used for installing
#
# Needs GTK 4.14+ and libadwaita 1.5+ on the system (Arch: gtk4 libadwaita,
# Debian 13 / Ubuntu 24.04+: libgtk-4-1 libadwaita-1-0, Fedora: gtk4 libadwaita).
set -eu

APP_ID=io.github.spaghetticodez.kaka
PREFIX="${HOME}/.local"
ACTION=install

while [ $# -gt 0 ]; do
    case "$1" in
        --prefix) PREFIX="$2"; shift 2 ;;
        --prefix=*) PREFIX="${1#*=}"; shift ;;
        --uninstall) ACTION=uninstall; shift ;;
        -h | --help) sed -n '2,10p' "$0"; exit 0 ;;
        *) echo "unknown option: $1" >&2; exit 2 ;;
    esac
done

HERE=$(cd "$(dirname "$0")" && pwd)
BIN="$PREFIX/bin/kaka"
DESKTOP="$PREFIX/share/applications/$APP_ID.desktop"
ICON="$PREFIX/share/icons/hicolor/scalable/apps/$APP_ID.svg"

if [ "$ACTION" = uninstall ]; then
    rm -f "$BIN" "$DESKTOP" "$ICON"
    echo "Removed KaKa from $PREFIX. Your library and logs are kept:"
    echo "  ${XDG_DATA_HOME:-$HOME/.local/share}/$APP_ID/  and  ${XDG_STATE_HOME:-$HOME/.local/state}/kaka/"
    exit 0
fi

# Release tarball layout first, then a source checkout.
if [ -f "$HERE/kaka" ]; then
    SRC_BIN="$HERE/kaka" SRC_DESKTOP="$HERE/$APP_ID.desktop" SRC_ICON="$HERE/$APP_ID.svg"
else
    ROOT=$(cd "$HERE/.." && pwd)
    SRC_BIN="$ROOT/target/release/kaka" SRC_DESKTOP="$ROOT/data/$APP_ID.desktop" SRC_ICON="$ROOT/data/icons/$APP_ID.svg"
fi
for f in "$SRC_BIN" "$SRC_DESKTOP" "$SRC_ICON"; do
    [ -f "$f" ] || { echo "missing $f (in a source checkout, run: cargo build --release --locked)" >&2; exit 1; }
done

install -Dm755 "$SRC_BIN" "$BIN"
install -Dm644 "$SRC_ICON" "$ICON"
# Absolute Exec: the menu entry must work even when $PREFIX/bin is not on the session PATH.
install -d "$(dirname "$DESKTOP")"
sed "s|^Exec=kaka\$|Exec=$BIN|" "$SRC_DESKTOP" > "$DESKTOP"
chmod 644 "$DESKTOP"

# Refresh an existing icon cache only: creating one in a user prefix makes GTK miss icons other
# apps add later. The desktop file has no MimeType, so no desktop database update is needed.
CACHE="$PREFIX/share/icons/hicolor/icon-theme.cache"
if [ -f "$CACHE" ] && command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q "$PREFIX/share/icons/hicolor" 2>/dev/null || true
fi

echo "Installed KaKa to $BIN"
