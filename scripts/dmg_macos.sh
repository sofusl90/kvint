#!/usr/bin/env bash
# Wraps dist/Kvint.app into a signed DMG.
#
# Required env:
#   DEVELOPER_ID_APP — same cert used for the .app
#
# Output: dist/Kvint.dmg

set -euo pipefail
cd "$(dirname "$0")/.."

: "${DEVELOPER_ID_APP:?need DEVELOPER_ID_APP env var}"

APP="dist/Kvint.app"
DMG="dist/Kvint.dmg"
STAGE="dist/.dmg-stage"

[[ -d "$APP" ]] || { echo "missing $APP"; exit 1; }

echo "==> Staging DMG contents"
rm -rf "$STAGE" "$DMG"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"

echo "==> Creating DMG"
hdiutil create \
    -volname "Kvint" \
    -srcfolder "$STAGE" \
    -ov -format UDZO \
    "$DMG"

rm -rf "$STAGE"

echo "==> Signing DMG"
codesign --force --sign "$DEVELOPER_ID_APP" --timestamp "$DMG"
codesign --verify --verbose=2 "$DMG"

echo "==> Done: $DMG"
echo "    Distribute this file. Users mount it, drag Kvint.app to Applications."
