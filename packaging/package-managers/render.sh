#!/usr/bin/env bash
# Fill in the package-manager files for a release.
#   packaging/package-managers/render.sh <version> <SHA256SUMS file> <output folder>
# The SHA256SUMS file lists the release's files (`sha256sum` format). Writes, under the output
# folder: homebrew/roomtone.rb (a Homebrew cask), winget/*.yaml (a winget manifest) and
# aur/PKGBUILD (an Arch User Repository package). Nothing is published; see docs/RELEASING.md.
set -euo pipefail
version="${1:?version, e.g. 0.6.1}"
sums="${2:?SHA256SUMS file}"
out="${3:?output folder}"
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"

sum_of() {
  local s
  s="$(awk -v f="$1" '{ n = $2; sub(/^\*/, "", n); if (n == f) print $1 }' "$sums")"
  if [ -z "$s" ]; then echo "No checksum for $1 in $sums" >&2; exit 1; fi
  echo "$s"
}

sha_dmg="$(sum_of "roomtone-v${version}-macos-arm64.dmg")"
sha_exe="$(sum_of "roomtone-v${version}-windows-x64-setup.exe")"
sha_linux="$(sum_of "roomtone-v${version}-linux-x64.tar.gz")"
if command -v sha256sum >/dev/null; then
  sha_desktop="$(sha256sum "$repo/packaging/linux/roomtone.desktop" | cut -d' ' -f1)"
else
  sha_desktop="$(shasum -a 256 "$repo/packaging/linux/roomtone.desktop" | cut -d' ' -f1)"
fi
date="$(date -u +%Y-%m-%d)"

mkdir -p "$out/homebrew" "$out/winget" "$out/aur"
fill() {
  sed -e "s|@VERSION@|${version}|g" -e "s|@SHA_DMG@|${sha_dmg}|g" -e "s|@SHA_EXE@|${sha_exe}|g" \
      -e "s|@SHA_LINUX@|${sha_linux}|g" -e "s|@SHA_DESKTOP@|${sha_desktop}|g" -e "s|@DATE@|${date}|g" "$1" > "$2"
}
fill "$here/homebrew/roomtone.rb" "$out/homebrew/roomtone.rb"
for f in "$here"/winget/*.yaml; do fill "$f" "$out/winget/$(basename "$f")"; done
fill "$here/aur/PKGBUILD" "$out/aur/PKGBUILD"
if grep -rq '@[A-Z_]*@' "$out"; then echo "Some placeholders were left unfilled" >&2; grep -rn '@[A-Z_]*@' "$out" >&2; exit 1; fi
echo "Wrote package-manager files for $version to $out"
