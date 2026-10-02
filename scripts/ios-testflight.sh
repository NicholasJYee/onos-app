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
# Signing: xcodebuild is run from a terminal here, not from the Xcode GUI, so it
# cannot always reach the interactive developer-portal session. When it cannot,
# it fails with "Cloud signing permission error" and "No profiles for
# '<bundle id>' were found" even though the account is an Admin and the
# distribution certificate exists.
#
# The fix is an App Store Connect API key. Tauri forwards it to xcodebuild as
# authentication credentials when all three of these are set:
#
#   APPLE_API_KEY       the Key ID, e.g. A1B2C3D4E5
#   APPLE_API_ISSUER    the Issuer ID (a UUID, shown above the key list)
#   APPLE_API_KEY_PATH  path to the downloaded AuthKey_<KEYID>.p8
#
# Rather than exporting those by hand each time, put them in
# scripts/ios-signing.env (gitignored; see the .example alongside it). This
# script sources that file, and will find the key automatically if it lives in
# ~/.appstoreconnect/private_keys/.
#
# Create one at App Store Connect > Users and Access > Integrations > App Store
# Connect API, with the "App Manager" role. The .p8 downloads once and cannot be
# downloaded again, so keep it somewhere safe and out of this repo. The same key
# also works for uploading with `xcrun altool`.
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

# Local, untracked credentials, so they do not have to be exported by hand every
# time. See scripts/ios-signing.env.example.
signing_env="$repo_root/scripts/ios-signing.env"
if [ -f "$signing_env" ]; then
    # shellcheck disable=SC1090
    . "$signing_env"
    echo "==> Loaded signing config from scripts/ios-signing.env"
fi

# With a key id but no explicit path, look where Apple's own tools keep keys.
if [ -n "${APPLE_API_KEY:-}" ] && [ -z "${APPLE_API_KEY_PATH:-}" ]; then
    for candidate in \
        "$HOME/.appstoreconnect/private_keys/AuthKey_${APPLE_API_KEY}.p8" \
        "$HOME/private_keys/AuthKey_${APPLE_API_KEY}.p8"; do
        if [ -f "$candidate" ]; then
            export APPLE_API_KEY_PATH="$candidate"
            echo "==> Found signing key at $candidate"
            break
        fi
    done
fi

# Report which signing route this run will take, since the failure mode when
# credentials are missing is an opaque permissions error much later.
if [ -n "${APPLE_API_KEY:-}" ] && [ -n "${APPLE_API_ISSUER:-}" ] && [ -n "${APPLE_API_KEY_PATH:-}" ]; then
    if [ ! -f "$APPLE_API_KEY_PATH" ]; then
        echo "error: APPLE_API_KEY_PATH is set but no file at $APPLE_API_KEY_PATH" >&2
        exit 1
    fi
    echo "==> Signing with App Store Connect API key ${APPLE_API_KEY}"
else
    echo "==> No App Store Connect API key set; relying on Xcode's signing session."
    echo "    If this fails with \"Cloud signing permission error\" or \"No profiles"
    echo "    for 'com.onos.ai' were found\", set APPLE_API_KEY, APPLE_API_ISSUER and"
    echo "    APPLE_API_KEY_PATH (see the comments at the top of this script)."
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
