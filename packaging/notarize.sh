#!/bin/sh
# Signs, notarizes, and staples Larik Text for distribution outside the App Store.
#
# One-time setup (needs a paid Apple Developer Program account):
#   1. Create a "Developer ID Application" certificate and install it in your keychain.
#   2. Store notarization credentials under a profile name:
#        xcrun notarytool store-credentials larik-notary \
#          --apple-id you@example.com --team-id TEAMID --password <app-specific-password>
#
# Usage:
#   LARIK_SIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" packaging/notarize.sh
# Optional: LARIK_NOTARY_PROFILE (default larik-notary).
# In CI, set APPLE_ID, APPLE_TEAM_ID and APPLE_APP_PASSWORD instead of a stored profile.
set -eu

cd "$(dirname "$0")/.."
: "${LARIK_SIGN_IDENTITY:?Set LARIK_SIGN_IDENTITY to your Developer ID Application identity.}"
profile="${LARIK_NOTARY_PROFILE:-larik-notary}"

packaging/macos.sh
app="dist/Larik Text.app"
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
zip="dist/Larik-Text-$version-macos-arm64.zip"

# Notarization takes a zip (or dmg/pkg); ditto keeps the bundle's metadata intact.
ditto -c -k --keepParent "$app" "$zip"
if [ -n "${APPLE_ID:-}" ]; then
    xcrun notarytool submit "$zip" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" \
        --password "$APPLE_APP_PASSWORD" --wait
else
    xcrun notarytool submit "$zip" --keychain-profile "$profile" --wait
fi

# Staple the ticket to the app so it opens offline, then rebuild the zip around it.
xcrun stapler staple "$app"
xcrun stapler validate "$app"
spctl --assess --type execute --verbose=2 "$app"
rm -f "$zip"
ditto -c -k --keepParent "$app" "$zip"
printf '%s\n' "Notarized: $PWD/$zip"
