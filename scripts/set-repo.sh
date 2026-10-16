#!/usr/bin/env bash
# Points every link in the repo at your GitHub repository, once it exists.
#
#   ./scripts/set-repo.sh your-user/eitri
#
# Replaces the __OWNER__/eitri and __SITE_URL__ placeholders in the README, website, issue
# templates and package manifests. The placeholder is deliberately not a valid GitHub username,
# so an unfilled link can never point at an account someone else registered. CI and the website
# workflow both refuse to run while a placeholder remains.
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
files=$(grep -rl "__OWNER__/eitri\|__SITE_URL__" README.md site .github package.json src-tauri/Cargo.toml CONTRIBUTING.md SECURITY.md docs 2>/dev/null || true)
for f in $files; do
  sed -i.bak -e "s#__OWNER__/eitri#${repo}#g" -e "s#__SITE_URL__#${site}#g" "$f" && rm -f "$f.bak"
done
echo "Linked to https://github.com/${repo} and ${site}"
echo "Next: enable Pages in the repository's Settings > Pages > Source: GitHub Actions."
