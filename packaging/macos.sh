#!/bin/sh
set -eu

cd "$(dirname "$0")/.."
if [ "$(uname -s)" != Darwin ]; then
    printf '%s\n' 'Run this script on macOS.' >&2
    exit 1
fi

case "${1:-}" in
    '') profile=release; cargo build --locked --release ;;
    --debug) profile=debug; cargo build --locked ;;
    *) printf '%s\n' 'Usage: packaging/macos.sh [--debug]' >&2; exit 2 ;;
esac

target_dir=$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
mkdir -p dist
stage=$(mktemp -d "$PWD/dist/.larik.XXXXXX")
trap 'rm -rf "$stage"' EXIT HUP INT TERM
app="$stage/Larik Text.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$stage/LarikText.iconset"
cp "$target_dir/$profile/larik-text" "$app/Contents/MacOS/larik-text"
sed "s/@VERSION@/$version/g" packaging/macos-info.plist > "$app/Contents/Info.plist"

sed 's/viewBox="0 0 96 96"/width="1024" height="1024" viewBox="0 0 96 96"/' assets/app-icon.svg > "$stage/raster.svg"
sips -s format png "$stage/raster.svg" --out "$stage/1024.png" >/dev/null
for points in 16 32 128 256 512; do
    sips -z "$points" "$points" "$stage/1024.png" --out "$stage/LarikText.iconset/icon_${points}x${points}.png" >/dev/null
    pixels=$((points * 2))
    sips -z "$pixels" "$pixels" "$stage/1024.png" --out "$stage/LarikText.iconset/icon_${points}x${points}@2x.png" >/dev/null
done
iconutil -c icns "$stage/LarikText.iconset" -o "$app/Contents/Resources/LarikText.icns"

# File icon shown in Finder for documents this app is set to open.
mkdir -p "$stage/LarikDocument.iconset"
sed 's/viewBox="0 0 96 96"/width="1024" height="1024" viewBox="0 0 96 96"/' assets/document-icon.svg > "$stage/document.svg"
sips -s format png "$stage/document.svg" --out "$stage/document-1024.png" >/dev/null
for points in 16 32 128 256 512; do
    sips -z "$points" "$points" "$stage/document-1024.png" --out "$stage/LarikDocument.iconset/icon_${points}x${points}.png" >/dev/null
    pixels=$((points * 2))
    sips -z "$pixels" "$pixels" "$stage/document-1024.png" --out "$stage/LarikDocument.iconset/icon_${points}x${points}@2x.png" >/dev/null
done
iconutil -c icns "$stage/LarikDocument.iconset" -o "$app/Contents/Resources/LarikDocument.icns"
for license in assets/fonts/*-OFL.txt; do cp "$license" "$app/Contents/Resources/"; done
plutil -lint "$app/Contents/Info.plist"
# LARIK_SIGN_IDENTITY="Developer ID Application: Name (TEAMID)" signs for distribution
# (hardened runtime plus a secure timestamp, both required for notarization).
# Without it the app gets an ad-hoc signature that only runs on this Mac.
if [ -n "${LARIK_SIGN_IDENTITY:-}" ]; then
    codesign --force --options runtime --timestamp --sign "$LARIK_SIGN_IDENTITY" "$app"
else
    codesign --force --sign - "$app"
fi
codesign --verify --strict --verbose=2 "$app"

destination="dist/Larik Text.app"
if [ "$profile" = debug ]; then destination="dist/debug/Larik Text.app"; fi
mkdir -p "$(dirname "$destination")"
ditto "$app" "$destination"
printf '%s\n' "$PWD/$destination"
