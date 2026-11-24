#!/bin/sh
# Installs Larik Text for the current user so it shows up in "Open With".
# Usage: ./install.sh [--default] [--uninstall]
set -eu

here=$(cd "$(dirname "$0")" && pwd)
prefix="${XDG_DATA_HOME:-$HOME/.local/share}"
bin="$HOME/.local/bin"
apps="$prefix/applications"
icons="$prefix/icons/hicolor/scalable/apps"

if [ "${1:-}" = "--uninstall" ]; then
    rm -f "$bin/larik-text" "$apps/larik-text.desktop" "$icons/larik-text.svg"
    command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$apps" || true
    printf '%s\n' 'Larik Text was removed.'
    exit 0
fi

mkdir -p "$bin" "$apps" "$icons"
install -m 755 "$here/larik-text" "$bin/larik-text"
install -m 644 "$here/app-icon.svg" "$icons/larik-text.svg"
sed "s|^Exec=larik-text|Exec=$bin/larik-text|" "$here/larik-text.desktop" > "$apps/larik-text.desktop"
chmod 644 "$apps/larik-text.desktop"
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$apps" || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t "$prefix/icons/hicolor" 2>/dev/null || true

if [ "${1:-}" = "--default" ]; then
    for type in text/plain text/markdown text/x-python application/json text/x-shellscript; do
        xdg-mime default larik-text.desktop "$type"
    done
    printf '%s\n' 'Larik Text is now the default for plain text, Markdown, Python, JSON, and shell scripts.'
else
    printf '%s\n' 'Larik Text now appears in Open With. Run ./install.sh --default to make it the default for common text files.'
fi
