#!/usr/bin/env bash
# Notarizes dist/Kvint.app with notarytool and staples the ticket (takes 1–3 min).
#
# Required env:
#   APPLE_ID         — Apple Developer account email
#   APPLE_PASSWORD   — an app-specific password, not your iCloud password
#   TEAM_ID          — 10-char team identifier

set -euo pipefail
cd "$(dirname "$0")/.."

: "${APPLE_ID:?need APPLE_ID env var}"
: "${APPLE_PASSWORD:?need APPLE_PASSWORD env var (app-specific password, not iCloud)}"
: "${TEAM_ID:?need TEAM_ID env var}"

APP="dist/Kvint.app"
ZIP="dist/Kvint-notarize.zip"

[[ -d "$APP" ]] || { echo "missing $APP — sign first"; exit 1; }

echo "==> Zipping $APP for upload"
rm -f "$ZIP"
ditto -c -k --keepParent "$APP" "$ZIP"

echo "==> Submitting to Apple notary service"
xcrun notarytool submit "$ZIP" \
    --apple-id "$APPLE_ID" \
    --password "$APPLE_PASSWORD" \
    --team-id "$TEAM_ID" \
    --wait

echo "==> Stapling ticket to $APP"
xcrun stapler staple "$APP"
xcrun stapler validate "$APP"

rm -f "$ZIP"
echo "==> Notarized. Next: scripts/dmg_macos.sh"
