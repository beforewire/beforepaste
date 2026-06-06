#!/usr/bin/env bash
#
# Interactive release helper.
#
# 1. Refuses to run with a dirty working tree.
# 2. Prompts for patch/minor/major/explicit bump.
# 3. Updates Cargo.toml, desktop manifests, and lockfiles with the new version.
# 4. Creates the release commit + tag.
# 5. Optionally pushes (off by default).
#
# Website and hosting deployment are intentionally outside this helper. This
# script only updates app/package versions, creates the release commit, and
# tags it.

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

if ! git diff-index --quiet HEAD --; then
  echo "error: working tree is dirty. Commit or stash before releasing." >&2
  exit 1
fi

current="$(grep -E '^version *= *"' Cargo.toml | head -n1 | sed -E 's/^version *= *"([^"]+)".*/\1/')"
if [[ -z "$current" ]]; then
  echo "error: could not parse current version from Cargo.toml" >&2
  exit 1
fi

IFS='.' read -r major minor patch <<<"$current"
patch_next="$major.$minor.$((patch + 1))"
minor_next="$major.$((minor + 1)).0"
major_next="$((major + 1)).0.0"

echo
echo "Current version: $current"
echo "Select bump:"
echo "  1) patch     -> $patch_next"
echo "  2) minor     -> $minor_next"
echo "  3) major     -> $major_next"
echo "  4) explicit  (you type the next version)"
echo
read -r -p "Choice [1-4]: " choice
case "$choice" in
  1) new="$patch_next" ;;
  2) new="$minor_next" ;;
  3) new="$major_next" ;;
  4)
    read -r -p "New version (e.g. 1.2.3): " new
    ;;
  *) echo "abort: invalid choice" >&2; exit 1 ;;
esac

if ! [[ "$new" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.-]+)?$ ]]; then
  echo "error: '$new' is not a valid SemVer string" >&2
  exit 1
fi

if git rev-parse --verify "v$new" >/dev/null 2>&1; then
  echo "error: tag v$new already exists. Pick a higher version, or delete the" >&2
  echo "       existing tag first with:  git tag -d v$new && git push origin :refs/tags/v$new" >&2
  exit 1
fi

# Bump Cargo.toml version (the FIRST `version = ` line - the package one).
sed -i -E '0,/^version = "/{ s/^version = "[^"]+"/version = "'"$new"'"/ }' Cargo.toml

# Keep the Tauri desktop package version aligned with the CLI release tag.
if [[ -f desktop/src-tauri/Cargo.toml ]]; then
  sed -i -E '0,/^version = "/{ s/^version = "[^"]+"/version = "'"$new"'"/ }' desktop/src-tauri/Cargo.toml
fi

if [[ -f desktop/src-tauri/tauri.conf.json ]]; then
  node -e '
const fs = require("fs");
const path = "desktop/src-tauri/tauri.conf.json";
const version = process.argv[1];
const config = JSON.parse(fs.readFileSync(path, "utf8"));
config.version = version;
fs.writeFileSync(path, JSON.stringify(config, null, 2) + "\n");
' "$new"
fi

if [[ -f desktop/package.json ]]; then
  (cd desktop && npm version "$new" --no-git-tag-version --allow-same-version >/dev/null)
fi

# Refresh lockfile package versions.
cargo update -p beforepaste >/dev/null 2>&1 || true
if [[ -f desktop/src-tauri/Cargo.toml ]]; then
  cargo update --manifest-path desktop/src-tauri/Cargo.toml -p beforepaste -p beforepaste-desktop >/dev/null 2>&1 || true
fi

echo
echo "--- staged changes ---"
git --no-pager diff -- Cargo.toml Cargo.lock desktop/package.json desktop/package-lock.json desktop/src-tauri/Cargo.toml desktop/src-tauri/Cargo.lock desktop/src-tauri/tauri.conf.json
echo "--- end staged changes ---"
echo

read -r -p "Commit and tag v$new? [y/N]: " do_commit
if [[ "${do_commit:-}" != "y" && "${do_commit:-}" != "Y" ]]; then
  echo "aborted before commit. Version files are modified;"
  echo "run 'git checkout -- Cargo.toml Cargo.lock desktop/package.json desktop/package-lock.json desktop/src-tauri/Cargo.toml desktop/src-tauri/Cargo.lock desktop/src-tauri/tauri.conf.json' to undo."
  exit 0
fi

git add Cargo.toml Cargo.lock desktop/package.json desktop/package-lock.json desktop/src-tauri/Cargo.toml desktop/src-tauri/Cargo.lock desktop/src-tauri/tauri.conf.json
git commit -m "chore(release): v$new"
# Annotated tag (not lightweight) so `git push --follow-tags` actually ships
# it. Without -a, the tag stays local and release.yml never fires.
git tag -a "v$new" -m "v$new"

echo
echo "Created commit and tag v$new on $(git rev-parse --abbrev-ref HEAD)."
echo

read -r -p "Push to origin now? [y/N]: " do_push
if [[ "${do_push:-}" != "y" && "${do_push:-}" != "Y" ]]; then
  current_branch="$(git rev-parse --abbrev-ref HEAD)"
  echo "skipped push. When ready, run:"
  echo "  git push --follow-tags origin $current_branch"
  exit 0
fi

current_branch="$(git rev-parse --abbrev-ref HEAD)"
git push --follow-tags origin "$current_branch"
echo
echo "Pushed v$new. release.yml will start on GitHub now."
