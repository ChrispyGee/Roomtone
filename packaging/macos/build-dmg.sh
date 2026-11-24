#!/usr/bin/env bash
# Builds Roomtone.app and a .dmg from target/release/roomtone (run `cargo build --release` first).
# Usage: packaging/macos/build-dmg.sh <version> <output.dmg>
# Without the variables below it is signed ad hoc only (not notarised): macOS asks you to confirm
# the first launch (System Settings › Privacy & Security › Open Anyway; docs/FIRST_RUN.md).
# With ROOMTONE_SIGN_IDENTITY (the name of a "Developer ID Application" certificate in the
# keychain) the app and the .dmg are signed with the hardened runtime and a secure timestamp; with
# ROOMTONE_NOTARY_APPLE_ID, ROOMTONE_NOTARY_PASSWORD (an app-specific password) and
# ROOMTONE_NOTARY_TEAM as well, the .dmg is also notarised by Apple and stapled, so it opens
# without a warning.
set -euo pipefail

version="${1:?version, e.g. 0.5.1}"
out="${2:?output .dmg path}"
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
exe="$root/target/release/roomtone"
[ -x "$exe" ] || { echo "Build first: cargo build --release" >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
app="$work/dmg/Roomtone.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"

cp "$exe" "$app/Contents/MacOS/roomtone"
sed "s/@VERSION@/$version/g" "$here/Info.plist.in" > "$app/Contents/Info.plist"
printf 'APPL????' > "$app/Contents/PkgInfo"

# The icon is drawn in code; ask the program to save it, then build the .icns.
iconset="$work/Roomtone.iconset"
mkdir "$iconset"
for s in 16 32 128 256 512; do
  "$exe" --write-icon "$iconset/icon_${s}x${s}.png" "$s"
  "$exe" --write-icon "$iconset/icon_${s}x${s}@2x.png" "$((s * 2))"
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/Roomtone.icns"

identity="${ROOMTONE_SIGN_IDENTITY:-}"
if [ -n "$identity" ]; then
  # Developer ID: hardened runtime and a secure timestamp are required for notarisation.
  codesign --force --options runtime --timestamp --sign "$identity" "$app"
  codesign --verify --strict --verbose=2 "$app"
else
  # An ad-hoc signature keeps Apple-silicon Macs from calling the app damaged.
  codesign --force --deep --sign - "$app"
fi

ln -s /Applications "$work/dmg/Applications"
rm -f "$out"
hdiutil create -volname "Roomtone $version" -srcfolder "$work/dmg" -ov -format UDZO "$out" > /dev/null

if [ -n "$identity" ]; then
  codesign --force --timestamp --sign "$identity" "$out"
  if [ -n "${ROOMTONE_NOTARY_APPLE_ID:-}" ] && [ -n "${ROOMTONE_NOTARY_PASSWORD:-}" ] && [ -n "${ROOMTONE_NOTARY_TEAM:-}" ]; then
    log="$work/notary.txt"
    xcrun notarytool submit "$out" --apple-id "$ROOMTONE_NOTARY_APPLE_ID" --password "$ROOMTONE_NOTARY_PASSWORD" \
      --team-id "$ROOMTONE_NOTARY_TEAM" --wait | tee "$log"
    if ! grep -q "status: Accepted" "$log"; then
      echo "Apple did not accept the .dmg; see the log above." >&2
      exit 1
    fi
    xcrun stapler staple "$out"
    xcrun stapler validate "$out"
  fi
fi
echo "Wrote $out"
