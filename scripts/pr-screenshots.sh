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
# The owner/name for the image links, from GitHub itself: never from the remote's address, which
# can have a login embedded in it that must not end up in a pull request.
slug="$(gh repo view --json nameWithOwner -q .nameWithOwner)"
# Push with the GitHub CLI's login, not whatever the remote's address carries (a login in an
# address can be stale, and must never be printed).
origin="https://$(gh api user -q .login)@github.com/$slug.git"
git_gh() { git -c credential.helper= -c 'credential.helper=!gh auth git-credential' "$@"; }
tmp="$(mktemp -d)"
trap 'rm -rf -- "$tmp"' EXIT
git -C "$tmp" init --quiet
git -C "$tmp" remote add origin "$origin"
git -C "$tmp" config user.name "$(git config user.name)"
git -C "$tmp" config user.email "$(git config user.email)"
if git_gh -C "$tmp" fetch --quiet --depth 1 origin pr-screenshots 2>/dev/null; then
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
if ! git -C "$tmp" diff --cached --quiet; then
  git -C "$tmp" commit --quiet -m "Screenshots for pull request $pr"
  git_gh -C "$tmp" push --quiet origin pr-screenshots
fi
sha="$(git -C "$tmp" rev-parse HEAD)"
for f in "$@"; do
  name="$(basename "$f")"
  printf '![%s](https://raw.githubusercontent.com/%s/%s/pr-%s/%s)\n' "${name%.png}" "$slug" "$sha" "$pr" "$name"
done
