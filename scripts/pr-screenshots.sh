#!/usr/bin/env bash
# Publish screenshots for a pull request and print the Markdown that shows them.
#   scripts/pr-screenshots.sh <pr-number> <image.png> [<image.png> ...]
# Images go to a branch named `pr-screenshots` (never merged) under `pr-<number>/`, so they
# don't clutter the project's history, and are linked by their raw address. Take them from the
# demo system (`--screenshot`, see AGENTS.md): they must show no real room names or addresses.
set -euo pipefail
pr="${1:-}"
shift || true
if [ -z "$pr" ] || [ "$#" -eq 0 ]; then
  echo "Usage: $0 <pr-number> <image.png> [<image.png> ...]" >&2
  exit 2
fi
origin="$(git remote get-url origin)"
slug="$(printf '%s' "$origin" | sed -E 's#^(git@github.com:|https://github.com/)##; s#\.git$##')"
tmp="$(mktemp -d)"
trap 'rm -rf -- "$tmp"' EXIT
git -C "$tmp" init --quiet
git -C "$tmp" remote add origin "$origin"
git -C "$tmp" config user.name "$(git config user.name)"
git -C "$tmp" config user.email "$(git config user.email)"
if git -C "$tmp" fetch --quiet --depth 1 origin pr-screenshots 2>/dev/null; then
  git -C "$tmp" checkout --quiet -B pr-screenshots FETCH_HEAD
else
  git -C "$tmp" checkout --quiet --orphan pr-screenshots
  printf '%s\n' "Screenshots for pull requests, made by scripts/pr-screenshots.sh. This branch is never merged." > "$tmp/README.md"
  git -C "$tmp" add README.md
fi
mkdir -p "$tmp/pr-$pr"
for f in "$@"; do
  cp -- "$f" "$tmp/pr-$pr/$(basename "$f")"
done
git -C "$tmp" add "pr-$pr"
git -C "$tmp" commit --quiet -m "Screenshots for pull request $pr"
git -C "$tmp" push --quiet origin pr-screenshots
sha="$(git -C "$tmp" rev-parse HEAD)"
for f in "$@"; do
  name="$(basename "$f")"
  printf '![%s](https://raw.githubusercontent.com/%s/%s/pr-%s/%s)\n' "${name%.png}" "$slug" "$sha" "$pr" "$name"
done
