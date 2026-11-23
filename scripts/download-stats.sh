#!/usr/bin/env bash
# Download counts for Roomtone's GitHub releases, by platform and by general kind.
#
#   scripts/download-stats.sh               every release added together, then the latest release
#   scripts/download-stats.sh v0.9.4        one release
#   scripts/download-stats.sh --by-release  one line per release as well
#
# Read-only: it asks GitHub for the release list (needs the `gh` command, signed in, and `jq`).
# GitHub counts every download of a file, including the maintainer's own checks, package-manager
# installs (Homebrew downloads the .dmg), and anything automated, so read the numbers as an
# upper bound on people, not a head count.
set -euo pipefail

REPO="${ROOMTONE_REPO:-ChrispyGee/Roomtone}"
ONE=""; BY_RELEASE=0
for a in "$@"; do
  case "$a" in
    --by-release) BY_RELEASE=1 ;;
    -h|--help) sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    v*) ONE="$a" ;;
    *) echo "Unknown option: $a" >&2; exit 2 ;;
  esac
done
command -v gh >/dev/null || { echo "This needs the GitHub command line tool (gh)." >&2; exit 1; }
command -v jq >/dev/null || { echo "This needs jq." >&2; exit 1; }

releases=$(gh api --paginate "repos/$REPO/releases" --jq '.[] | {tag: .tag_name, assets: [.assets[] | {name, n: .download_count}]}' | jq -s '.')

# What each file is: its system, its kind and, on Linux, the family of systems it suits.
classify='
def kind:
  if   test("macos-arm64\\.dmg$")       then {os:"macOS",   what:"Installer (.dmg, Apple silicon)", flavour:"Installer"}
  elif test("macos-arm64\\.tar\\.gz$")  then {os:"macOS",   what:"Archive (.tar.gz, Apple silicon)", flavour:"Archive"}
  elif test("windows-x64-setup\\.exe$") then {os:"Windows", what:"Installer (setup .exe)",           flavour:"Installer"}
  elif test("windows-x64\\.zip$")       then {os:"Windows", what:"Archive (.zip)",                   flavour:"Archive"}
  elif test("linux-x64\\.deb$")         then {os:"Linux",   what:"Debian, Ubuntu and similar (.deb)", flavour:"Installer"}
  elif test("linux-x64\\.rpm$")         then {os:"Linux",   what:"Fedora, openSUSE, RHEL and similar (.rpm)", flavour:"Installer"}
  elif test("linux-x64\\.tar\\.gz$")    then {os:"Linux",   what:"Any Linux (.tar.gz archive)",      flavour:"Archive"}
  else {os:"Other", what:"Checksums and package-manager files", flavour:"Other"} end;
'

report() {  # $1 = title, $2 = JSON array of releases
  echo "$2" | jq -r --arg title "$1" "$classify"'
    ([.[].assets[] | . + (.name | kind)]) as $a
    | def total($xs): ($xs | map(.n) | add // 0);
      def pad($s; $w): ($s | tostring) as $t | $t + (" " * ([$w - ($t | length), 1] | max));
      def lpad($s; $w): ($s | tostring) as $t | (" " * ([$w - ($t | length), 0] | max)) + $t;
      def line($label; $n; $of): pad($label; 52) + lpad($n; 6) + (if $of > 0 then "  " + lpad((($n * 100 / $of) | round); 3) + "%" else "" end);
      ($a | map(select(.os != "Other"))) as $p
    | (total($p)) as $all
    | "== " + $title + " ==",
      "",
      "By platform (" + ($all | tostring) + " downloads of installers and archives)",
      ($p | group_by(.os + "|" + .what) | map({os: .[0].os, what: .[0].what, n: total(.)})
         | sort_by(. as $x | [(["macOS","Windows","Linux"] | index($x.os)), -$x.n, $x.what])[] | line(.os + ": " + .what; .n; $all)),
      "",
      "Summary by system",
      (["macOS","Windows","Linux"][] as $o | line($o; total($p | map(select(.os == $o))); $all)),
      "",
      "Summary by kind",
      (["Installer","Archive"][] as $f | line($f; total($p | map(select(.flavour == $f))); $all)),
      "",
      "Linux by family",
      (($p | map(select(.os == "Linux"))) as $l | (total($l)) as $lt
        | (["Debian, Ubuntu and similar (.deb)","Fedora, openSUSE, RHEL and similar (.rpm)","Any Linux (.tar.gz archive)"][] as $w
           | line($w; total($l | map(select(.what == $w))); $lt))),
      "",
      "Not counted above: " + (total($a | map(select(.os == "Other"))) | tostring) + " downloads of checksums and package-manager files"
  '
}

if [ -n "$ONE" ]; then
  sel=$(echo "$releases" | jq --arg t "$ONE" '[.[] | select(.tag == $t)]')
  [ "$(echo "$sel" | jq length)" -gt 0 ] || { echo "No release called $ONE." >&2; exit 1; }
  report "Release $ONE" "$sel"
  exit 0
fi

n=$(echo "$releases" | jq length)
report "All $n releases added together" "$releases"
latest=$(echo "$releases" | jq -r '.[0].tag')
echo
report "Latest release, $latest" "$(echo "$releases" | jq '[.[0]]')"
if [ "$BY_RELEASE" = 1 ]; then
  echo
  echo "== One line per release (installers and archives) =="
  echo "$releases" | jq -r "$classify"'
    .[] | . as $r | ([.assets[] | . + (.name | kind) | select(.os != "Other") | .n] | add // 0) as $t
    | $r.tag + "  " + ($t | tostring)'
fi
