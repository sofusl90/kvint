#!/usr/bin/env bash
# Generates platform app-icon assets from `branding/icon.png`.
#
#   * iOS:    gen/apple/Assets.xcassets/AppIcon.appiconset/{Contents.json, *.png}
#             (committed — Xcode build needs it on a fresh checkout)
#   * macOS:  target/icons/Kvint.icns
#             (gitignored — build_macos.sh regenerates as needed)
#
# Run from the repo root:    ./scripts/gen_app_icons.sh
#
# Requires `sips` and `iconutil` (preinstalled on macOS).

set -euo pipefail

cd "$(dirname "$0")/.."

SRC="branding/icon.png"
[[ -f "$SRC" ]] || { echo "missing $SRC"; exit 1; }

# ---------- iOS Asset Catalog ---------------------------------------------
IOS_DIR="gen/apple/Assets.xcassets/AppIcon.appiconset"
mkdir -p "$IOS_DIR"

# Sizes Apple still wants for iOS 13+ (idiom, size in pt, scale)
# Format below is "filename:px-size"
declare -a IOS_TARGETS=(
  "icon-20@2x.png:40"        # 20pt @2x — notification iPhone
  "icon-20@3x.png:60"        # 20pt @3x — notification iPhone
  "icon-29@2x.png:58"        # 29pt @2x — settings iPhone
  "icon-29@3x.png:87"        # 29pt @3x — settings iPhone
  "icon-40@2x.png:80"        # 40pt @2x — spotlight iPhone
  "icon-40@3x.png:120"       # 40pt @3x — spotlight iPhone
  "icon-60@2x.png:120"       # 60pt @2x — app iPhone
  "icon-60@3x.png:180"       # 60pt @3x — app iPhone
  "icon-20.png:20"           # iPad notification @1x
  "icon-29.png:29"           # iPad settings @1x
  "icon-40.png:40"           # iPad spotlight @1x
  "icon-76.png:76"           # iPad app @1x
  "icon-76@2x.png:152"       # iPad app @2x
  "icon-83.5@2x.png:167"     # iPad Pro app @2x
  "icon-1024.png:1024"       # App Store
)

for target in "${IOS_TARGETS[@]}"; do
  name="${target%%:*}"
  size="${target##*:}"
  sips -z "$size" "$size" "$SRC" --out "$IOS_DIR/$name" >/dev/null
done

cat > "$IOS_DIR/Contents.json" <<'JSON'
{
  "images" : [
    { "size" : "20x20",   "idiom" : "iphone", "filename" : "icon-20@2x.png", "scale" : "2x" },
    { "size" : "20x20",   "idiom" : "iphone", "filename" : "icon-20@3x.png", "scale" : "3x" },
    { "size" : "29x29",   "idiom" : "iphone", "filename" : "icon-29@2x.png", "scale" : "2x" },
    { "size" : "29x29",   "idiom" : "iphone", "filename" : "icon-29@3x.png", "scale" : "3x" },
    { "size" : "40x40",   "idiom" : "iphone", "filename" : "icon-40@2x.png", "scale" : "2x" },
    { "size" : "40x40",   "idiom" : "iphone", "filename" : "icon-40@3x.png", "scale" : "3x" },
    { "size" : "60x60",   "idiom" : "iphone", "filename" : "icon-60@2x.png", "scale" : "2x" },
    { "size" : "60x60",   "idiom" : "iphone", "filename" : "icon-60@3x.png", "scale" : "3x" },
    { "size" : "20x20",   "idiom" : "ipad",   "filename" : "icon-20.png",    "scale" : "1x" },
    { "size" : "20x20",   "idiom" : "ipad",   "filename" : "icon-20@2x.png", "scale" : "2x" },
    { "size" : "29x29",   "idiom" : "ipad",   "filename" : "icon-29.png",    "scale" : "1x" },
    { "size" : "29x29",   "idiom" : "ipad",   "filename" : "icon-29@2x.png", "scale" : "2x" },
    { "size" : "40x40",   "idiom" : "ipad",   "filename" : "icon-40.png",    "scale" : "1x" },
    { "size" : "40x40",   "idiom" : "ipad",   "filename" : "icon-40@2x.png", "scale" : "2x" },
    { "size" : "76x76",   "idiom" : "ipad",   "filename" : "icon-76.png",    "scale" : "1x" },
    { "size" : "76x76",   "idiom" : "ipad",   "filename" : "icon-76@2x.png", "scale" : "2x" },
    { "size" : "83.5x83.5","idiom" : "ipad",  "filename" : "icon-83.5@2x.png", "scale" : "2x" },
    { "size" : "1024x1024","idiom" : "ios-marketing","filename" : "icon-1024.png", "scale" : "1x" }
  ],
  "info" : { "author" : "xcode", "version" : 1 }
}
JSON

# Asset catalog needs a top-level Contents.json too
cat > "gen/apple/Assets.xcassets/Contents.json" <<'JSON'
{
  "info" : { "author" : "xcode", "version" : 1 }
}
JSON

echo "iOS:    wrote $IOS_DIR (15 sizes + Contents.json)"

# ---------- iOS Launch Screen ---------------------------------------------
# Keep the launch-screen icon in sync.
LAUNCH_DIR="gen/apple/Assets.xcassets/LaunchIcon.imageset"
mkdir -p "$LAUNCH_DIR"
sips -z 1024 1024 "$SRC" --out "$LAUNCH_DIR/icon-1024.png" >/dev/null
echo "iOS:    wrote $LAUNCH_DIR/icon-1024.png"

# ---------- macOS .icns ---------------------------------------------------
ICONSET="target/icons/Kvint.iconset"
ICNS="target/icons/Kvint.icns"
mkdir -p target/icons
rm -rf "$ICONSET"
mkdir -p "$ICONSET"

declare -a MAC_TARGETS=(
  "icon_16x16.png:16"
  "icon_16x16@2x.png:32"
  "icon_32x32.png:32"
  "icon_32x32@2x.png:64"
  "icon_128x128.png:128"
  "icon_128x128@2x.png:256"
  "icon_256x256.png:256"
  "icon_256x256@2x.png:512"
  "icon_512x512.png:512"
  "icon_512x512@2x.png:1024"
)
for target in "${MAC_TARGETS[@]}"; do
  name="${target%%:*}"
  size="${target##*:}"
  sips -z "$size" "$size" "$SRC" --out "$ICONSET/$name" >/dev/null
done

iconutil -c icns "$ICONSET" -o "$ICNS"
echo "macOS:  wrote $ICNS"
