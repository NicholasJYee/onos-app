#!/usr/bin/env bash
#
# Build an iOS .ipa for TestFlight.
#
# This differs from the normal `pnpm build:ios` in two ways:
#
#   1. It exports with `--export-method app-store-connect` instead of the
#      `debugging` method in gen/apple/ExportOptions.plist. The CLI merges the
#      flag over that file into a temporary copy, so the checked-in file is not
#      modified and direct-to-device builds keep working unchanged.
#
#   2. It stamps a fresh CFBundleVersion. App Store Connect rejects a build
#      whose (CFBundleShortVersionString, CFBundleVersion) pair it has already
#      seen, and this project hardcodes both to the same value, so a second
#      upload would be refused.
#
# The build number is a UTC timestamp (YYYYMMDDHHMM): always increasing, unique
# per minute, and it needs no state kept anywhere. CFBundleShortVersionString --
# the version users see -- is deliberately left alone; many builds per version
# is normal.
#
# Run from anywhere; paths are resolved relative to the repo.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
frontend="$repo_root/frontend"
info_plist="$frontend/src-tauri/gen/apple/onos_iOS/Info.plist"
project_yml="$frontend/src-tauri/gen/apple/project.yml"

if [ ! -f "$info_plist" ]; then
    echo "error: $info_plist not found" >&2
    exit 1
fi

build_number="$(date -u +%Y%m%d%H%M)"

echo "==> Stamping build number $build_number"

# The build reads Info.plist directly (tauri loads the pbxproj rather than
# re-running xcodegen), so this is what actually lands in the app...
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $build_number" "$info_plist"

# ...and project.yml is updated too so a future `xcodegen`/`tauri ios init`
# regenerates a plist with a build number no lower than this one.
if [ -f "$project_yml" ]; then
    /usr/bin/sed -i '' -E "s/^([[:space:]]*CFBundleVersion:).*/\1 \"$build_number\"/" "$project_yml"
fi

short_version="$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$info_plist")"
echo "==> Version $short_version ($build_number)"

echo "==> Building for App Store Connect"
cd "$frontend"
node node_modules/@tauri-apps/cli/tauri.js ios build --target aarch64 --export-method app-store-connect

ipa="$frontend/src-tauri/gen/apple/build/arm64/ONOS.ipa"
echo
if [ -f "$ipa" ]; then
    echo "Built $ipa"
    echo
    echo "Upload it with either:"
    echo "  - Transporter.app (free on the Mac App Store): drag the .ipa in, Deliver"
    echo "  - xcrun altool --upload-app -f \"$ipa\" -t ios \\"
    echo "        --apiKey <KEY_ID> --apiIssuer <ISSUER_ID>"
    echo
    echo "It appears in App Store Connect > TestFlight a few minutes after"
    echo "processing finishes."
else
    echo "warning: expected .ipa not found at $ipa" >&2
    echo "Check the build output above for the exported path." >&2
fi
