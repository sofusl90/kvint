#!/usr/bin/env bash
# Builds a universal macOS .app bundle for direct distribution.
#
# Output: dist/Kvint.app  (unsigned — run scripts/sign_macos.sh next)
#
# Requirements:
#   * Both `aarch64-apple-darwin` and `x86_64-apple-darwin` rustup targets.
#     `rustup target add x86_64-apple-darwin aarch64-apple-darwin` once.
#   * `lipo`, `sips`, `iconutil` (preinstalled on macOS).
#
# Pass `--no-x86` to skip the Intel slice if you only target Apple Silicon.

set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="$(cargo metadata --no-deps --format-version 1 \
    | sed -n 's/.*"name":"kvint","version":"\([^"]*\)".*/\1/p')"
BIN="kvint-desktop"
APP="dist/Kvint.app"

INCLUDE_X86=1
if [[ "${1:-}" == "--no-x86" ]]; then INCLUDE_X86=0; fi

echo "==> Building release for aarch64-apple-darwin"
cargo build --release --bin "$BIN" --target aarch64-apple-darwin

if (( INCLUDE_X86 )); then
    echo "==> Building release for x86_64-apple-darwin"
    cargo build --release --bin "$BIN" --target x86_64-apple-darwin
fi

echo "==> Assembling $APP"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

# Universal binary
if (( INCLUDE_X86 )); then
    lipo -create \
        "target/aarch64-apple-darwin/release/$BIN" \
        "target/x86_64-apple-darwin/release/$BIN" \
        -output "$APP/Contents/MacOS/$BIN"
else
    cp "target/aarch64-apple-darwin/release/$BIN" "$APP/Contents/MacOS/$BIN"
fi
chmod +x "$APP/Contents/MacOS/$BIN"

# Bundle assets
cp -R assets "$APP/Contents/Resources/"

# Icon (regenerate if stale)
if [[ ! -f target/icons/Kvint.icns || branding/icon.png -nt target/icons/Kvint.icns ]]; then
    echo "==> Regenerating icon"
    ./scripts/gen_app_icons.sh >/dev/null
fi
cp target/icons/Kvint.icns "$APP/Contents/Resources/Kvint.icns"

# Info.plist with version substituted in
sed "s/__VERSION__/$VERSION/g" packaging/macos/Info.plist.tmpl \
    > "$APP/Contents/Info.plist"

echo "==> Done: $APP (version $VERSION, unsigned)"
echo "    Next: scripts/sign_macos.sh"
