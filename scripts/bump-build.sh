#!/usr/bin/env bash
# Run before each TestFlight push to increment the build number.
# CFBundleShortVersionString (e.g. 1.0.0) is left alone — bump that manually
# when you release a new user-visible version.
set -euo pipefail

FILE="$(dirname "$0")/../gen/apple/project.yml"

current=$(grep 'CFBundleVersion:' "$FILE" | head -1 | awk '{print $2}')
new=$((current + 1))

sed -i '' "s/CFBundleVersion: ${current}$/CFBundleVersion: ${new}/" "$FILE"

echo "Build number: ${current} → ${new}"
echo "Run xcodegen to regenerate the Xcode project before archiving."
