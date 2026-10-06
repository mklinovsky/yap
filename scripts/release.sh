#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

current=$(sed -n 's/^  "version": "\(.*\)",$/\1/p' package.json)
IFS=. read -r major minor patch <<<"$current"
case "${1:-}" in
  patch) version="$major.$minor.$((patch + 1))" ;;
  minor) version="$major.$((minor + 1)).0" ;;
  major) version="$((major + 1)).0.0" ;;
  [0-9]*.[0-9]*.[0-9]*) version=$1 ;;
  *) echo "Usage: pnpm release <patch|minor|major|x.y.z>" >&2; exit 1 ;;
esac
tag="v$version"

[[ $(git branch --show-current) == main ]] || { echo "Release from main." >&2; exit 1; }
[[ -z $(git status --porcelain) ]] || { echo "Working tree is not clean." >&2; exit 1; }
[[ -z $(git tag --list "$tag") ]] || { echo "Tag $tag already exists." >&2; exit 1; }

# The release workflow publishes without running checks, so they run here (same order as CI).
pnpm build
pnpm lint
pnpm test

perl -pi -e 's/^  "version": "[^"]+"/  "version": "'"$version"'"/' package.json src-tauri/tauri.conf.json
perl -pi -e 's/^version = "[^"]+"/version = "'"$version"'"/' src-tauri/Cargo.toml
perl -0pi -e 's/(name = "yap"\nversion = )"[^"]+"/$1"'"$version"'"/' src-tauri/Cargo.lock

git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "Bump version to $version"
git tag "$tag"
git push --atomic origin main "$tag"
echo "Released $tag ($current → $version)."
