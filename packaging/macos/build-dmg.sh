#!/usr/bin/env bash
# Builds Roomtone.app and a .dmg from target/release/roomtone (run `cargo build --release` first).
# Usage: packaging/macos/build-dmg.sh <version> <output.dmg>
# Not signed or notarised: macOS asks you to confirm the first launch (right-click › Open).
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

# An ad-hoc signature keeps Apple-silicon Macs from calling the app damaged.
codesign --force --deep --sign - "$app"

ln -s /Applications "$work/dmg/Applications"
rm -f "$out"
hdiutil create -volname "Roomtone $version" -srcfolder "$work/dmg" -ov -format UDZO "$out" > /dev/null
echo "Wrote $out"
