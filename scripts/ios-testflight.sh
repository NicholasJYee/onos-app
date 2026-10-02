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

# A stale APPLE_API_KEY_PATH exported in the shell silently outranks the key id
# configured above, and pairing a key id with another key's .p8 fails as a 401
# ("Your Apple Account or password was entered incorrectly") deep in the export.
# Ignore any path that does not belong to the configured key.
if [ -n "${APPLE_API_KEY:-}" ] && [ -n "${APPLE_API_KEY_PATH:-}" ]; then
    case "$(basename "$APPLE_API_KEY_PATH")" in
        *"$APPLE_API_KEY"*) ;;
        *)
            echo "==> Ignoring APPLE_API_KEY_PATH=$APPLE_API_KEY_PATH"
            echo "    (it is not the key file for APPLE_API_KEY=$APPLE_API_KEY)"
            unset APPLE_API_KEY_PATH
            ;;
    esac
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

# --- Pin the signing for this export ----------------------------------------
#
# Automatic signing makes xcodebuild ask Apple to create the App Store profile
# at export time, and that call fails with "Cloud signing permission error"
# regardless of the credentials used. Everything needed already exists locally:
# an Apple Distribution certificate, and an App Store profile for this bundle
# id. So the export is told exactly what to use and never asks Apple anything.
#
# Both the Xcode project and ExportOptions.plist have to agree. tauri's
# merge_plist inserts later sources over earlier ones, and the CLI derives
# signingStyle from the project's CODE_SIGN_STYLE, so a "manual" written only
# into ExportOptions.plist is overwritten with "automatic" before xcodebuild
# ever sees it.
#
# Both files are restored on exit, so `pnpm build:ios` keeps signing for direct
# device installs exactly as before.

pbxproj="$frontend/src-tauri/gen/apple/onos.xcodeproj/project.pbxproj"
export_options="$frontend/src-tauri/gen/apple/ExportOptions.plist"
tauri_conf="$frontend/src-tauri/tauri.conf.json"

bundle_id="$(/usr/bin/sed -nE 's/.*"identifier"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/p' "$tauri_conf" | head -1)"
team_id="$(/usr/bin/sed -nE 's/.*"developmentTeam"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/p' "$tauri_conf" | head -1)"
profile_name="${APPLE_PROVISIONING_PROFILE:-ONOS}"
signing_certificate="${APPLE_SIGNING_CERTIFICATE:-Apple Distribution}"

if [ -z "$bundle_id" ] || [ -z "$team_id" ]; then
    echo "error: could not read identifier/developmentTeam from $tauri_conf" >&2
    exit 1
fi

echo "==> Pinning signing: $signing_certificate / profile \"$profile_name\" / $bundle_id ($team_id)"

restore_dir="$(mktemp -d)"
cp "$pbxproj" "$restore_dir/project.pbxproj"
cp "$export_options" "$restore_dir/ExportOptions.plist"

# tauri writes every export to ONOS.ipa, so this build is about to overwrite the
# device-installable one from `pnpm build:ios`. Move it out of the way first and
# put it back afterwards, otherwise a TestFlight run silently destroys it.
device_ipa="$frontend/src-tauri/gen/apple/build/arm64/ONOS.ipa"
preserved_device_ipa=""
if [ -f "$device_ipa" ]; then
    preserved_device_ipa="$restore_dir/ONOS-device.ipa"
    mv "$device_ipa" "$preserved_device_ipa"
    echo "==> Set aside the existing device .ipa while this build runs"
fi

restore_signing() {
    cp "$restore_dir/project.pbxproj" "$pbxproj"
    cp "$restore_dir/ExportOptions.plist" "$export_options"
    # Runs after the App Store .ipa has been renamed, so this cannot clobber it.
    if [ -n "$preserved_device_ipa" ] && [ -f "$preserved_device_ipa" ]; then
        mv "$preserved_device_ipa" "$device_ipa"
        echo "==> Restored the device .ipa"
    fi
    rm -rf "$restore_dir"
    echo "==> Restored project signing settings"
}
trap restore_signing EXIT

/usr/bin/sed -i '' 's/CODE_SIGN_STYLE = Automatic;/CODE_SIGN_STYLE = Manual;/g' "$pbxproj"
/usr/bin/sed -i '' 's/CODE_SIGN_IDENTITY = "Apple Development";/CODE_SIGN_IDENTITY = "Apple Distribution";/g' "$pbxproj"

cat > "$export_options" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <!-- Generated by scripts/ios-testflight.sh for one build; the committed
         version of this file is restored when the script exits. -->
    <key>method</key>
    <string>app-store-connect</string>
    <key>teamID</key>
    <string>${team_id}</string>
    <key>signingStyle</key>
    <string>manual</string>
    <key>signingCertificate</key>
    <string>${signing_certificate}</string>
    <key>provisioningProfiles</key>
    <dict>
        <key>${bundle_id}</key>
        <string>${profile_name}</string>
    </dict>
    <key>stripSwiftSymbols</key>
    <true/>
</dict>
</plist>
PLIST

echo "==> Building for App Store Connect"
cd "$frontend"
node node_modules/@tauri-apps/cli/tauri.js ios build --target aarch64 --export-method app-store-connect

# `pnpm build:ios` writes its device-installable .ipa to this same path, and
# whichever ran last wins. They are not interchangeable: an App Store build
# refuses to sideload ("Attempted to install a Beta profile without the proper
# entitlement", 0xe800801f) and a device build is rejected by App Store Connect.
# Keep this one under its own name so both survive and neither is ambiguous.
ipa="$frontend/src-tauri/gen/apple/build/arm64/ONOS-testflight-${build_number}.ipa"
if [ -f "$device_ipa" ]; then
    mv "$device_ipa" "$ipa"
fi

echo
if [ -f "$ipa" ]; then
    echo "Built $ipa"
    echo "(for installing directly on a device, use \`pnpm build:ios\` instead)"
    echo
    echo "Upload it with either:"
    echo "  - Transporter.app (free on the Mac App Store): drag the .ipa in, Deliver"
    echo "  - this command:"
    echo
    if [ -n "${APPLE_API_KEY:-}" ] && [ -n "${APPLE_API_ISSUER:-}" ]; then
        # altool finds the .p8 itself, in the same place this script looks.
        echo "      xcrun altool --upload-app -f \"$ipa\" -t ios \\"
        echo "        --apiKey $APPLE_API_KEY --apiIssuer $APPLE_API_ISSUER"
    else
        echo "      xcrun altool --upload-app -f \"$ipa\" -t ios \\"
        echo "        --apiKey <KEY_ID> --apiIssuer <ISSUER_ID>"
    fi
    echo
    echo "It appears in App Store Connect > TestFlight a few minutes after"
    echo "processing finishes."
else
    echo "warning: expected .ipa not found at $ipa" >&2
    echo "Check the build output above for the exported path." >&2
fi
