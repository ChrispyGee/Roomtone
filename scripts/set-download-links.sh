#!/usr/bin/env bash
# Point the README's download links at a release (do this in the release pull request).
#   scripts/set-download-links.sh 0.7.2
# A unit test (help.rs) fails if the README's links and Cargo.toml's version disagree.
set -euo pipefail
v="${1:?version, e.g. 0.7.2}"
case "$v" in [0-9]*.[0-9]*.[0-9]*) ;; *) echo "That doesn't look like a version: $v" >&2; exit 2 ;; esac
readme="$(cd "$(dirname "$0")/.." && pwd)/README.md"
tmp="$(mktemp)"
sed -E \
  -e "s#(releases/download/v)[0-9]+\.[0-9]+\.[0-9]+#\1${v}#g" \
  -e "s#(roomtone-v)[0-9]+\.[0-9]+\.[0-9]+(-)#\1${v}\2#g" \
  -e "s#(Download Roomtone )[0-9]+\.[0-9]+\.[0-9]+#\1${v}#" \
  "$readme" > "$tmp"
mv -- "$tmp" "$readme"
echo "README download links now point at v${v}"
