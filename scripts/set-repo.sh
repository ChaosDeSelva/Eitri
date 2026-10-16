#!/usr/bin/env bash
# Points every link in the repo at your GitHub repository, once it exists.
#
#   ./scripts/set-repo.sh your-user/eitri
#
# Replaces the YOUR-GITHUB-USER/eitri placeholder in the README, website, issue
# templates and package manifests, and sets the website URL used in the README.
set -euo pipefail
repo="${1:-}"
if [[ ! "$repo" =~ ^[A-Za-z0-9-]+/[A-Za-z0-9._-]+$ ]]; then
  echo "usage: $0 owner/name" >&2
  exit 1
fi
owner="${repo%%/*}"
name="${repo##*/}"
owner_lower="$(echo "$owner" | tr '[:upper:]' '[:lower:]')"
site="https://${owner_lower}.github.io/${name}/"
cd "$(dirname "$0")/.."
files=$(grep -rl "YOUR-GITHUB-USER/eitri\|YOUR-SITE-URL" README.md site .github package.json src-tauri/Cargo.toml CONTRIBUTING.md SECURITY.md docs 2>/dev/null || true)
for f in $files; do
  sed -i.bak -e "s#YOUR-GITHUB-USER/eitri#${repo}#g" -e "s#YOUR-SITE-URL#${site}#g" "$f" && rm -f "$f.bak"
done
echo "Linked to https://github.com/${repo} and ${site}"
echo "Next: enable Pages in the repository's Settings > Pages > Source: GitHub Actions."
