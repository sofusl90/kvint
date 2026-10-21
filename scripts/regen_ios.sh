#!/usr/bin/env bash
# Regenerates gen/apple/kvint.xcodeproj from project.yml, the source of truth.
# Run it after editing project.yml. Needs `xcodegen` (`brew install xcodegen`).

set -euo pipefail

if ! command -v xcodegen >/dev/null 2>&1; then
    echo "error: xcodegen not found — install via 'brew install xcodegen'" >&2
    exit 1
fi

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT/gen/apple"

xcodegen generate
echo "regenerated kvint.xcodeproj"
