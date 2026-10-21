#!/usr/bin/env bash
# Signs dist/Kvint.app for direct distribution (Hardened Runtime).
#
# Required env:
#   DEVELOPER_ID_APP — e.g. "Developer ID Application: Sofus Laubel (XXXXXXXXXX)"
#                       Run `security find-identity -v -p codesigning` to list yours.
#
# Output: dist/Kvint.app, signed in place.

set -euo pipefail
cd "$(dirname "$0")/.."

: "${DEVELOPER_ID_APP:?need DEVELOPER_ID_APP env var}"
APP="dist/Kvint.app"
ENT="packaging/macos/entitlements.plist"

[[ -d "$APP" ]] || { echo "missing $APP — run scripts/build_macos.sh first"; exit 1; }
[[ -f "$ENT" ]] || { echo "missing $ENT"; exit 1; }

echo "==> Signing $APP with Hardened Runtime"
# Sign the inner binary first, then the bundle.
codesign --force --options runtime --timestamp \
    --entitlements "$ENT" \
    --sign "$DEVELOPER_ID_APP" \
    "$APP/Contents/MacOS/kvint-desktop"

codesign --force --options runtime --timestamp \
    --entitlements "$ENT" \
    --sign "$DEVELOPER_ID_APP" \
    "$APP"

echo "==> Verifying signature"
codesign --verify --deep --strict --verbose=2 "$APP"
spctl -a -vv -t exec "$APP" || {
    echo "spctl rejected the signature — that's expected pre-notarization;"
    echo "submit via scripts/notarize_macos.sh next."
}

echo "==> Done. Next: scripts/notarize_macos.sh"
