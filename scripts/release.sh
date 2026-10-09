#!/usr/bin/env bash
# Makes the monthly release by hand: it checks that main is releasable, bumps
# every version, cuts the changelog, commits and tags. It then stops and prints
# the two push commands.
#
#   scripts/release.sh [--data-version YYYY.MM]
#
# It publishes nothing, pushes nothing and tags nothing on origin. It creates
# one local commit and one local tag; the push is yours to make after reading
# the diff. Pushing the tag starts .github/workflows/release.yml, which does
# the publishing.
#
# It holds no credential and needs none: it reads GitHub's public API, so it
# works whether or not `gh` is logged in. Set GH_TOKEN to raise the API's rate
# limit, never as a requirement.
#
# In order, refusing rather than half-doing anything:
#   1. on main, working tree clean, main matching origin/main
#   2. CI finished successfully for this commit on main
#   3. a data version, from the newest plain data-YYYY.MM release, or
#      --data-version
#   4. scripts/release-bump.sh, which decides the version from the changelog
#   5. scripts/tag-exists.sh, so a released version is never tagged twice
#   6. scripts/release-check.sh, the gate a real release has to pass
#   7. one commit and one tag, then it stops
#
# No secret is stored: the push is made by you, as you, and the tag ruleset
# admits repository administrators.
#
# scripts/release-test.sh tests this script; run it after changing it.
set -euo pipefail
cd "$(dirname "$0")/.."

# GitHub's API. RELEASE_API_DIR names a directory of canned answers instead,
# which is how scripts/release-test.sh answers the two queries without a
# network. Unset in real use.
api="https://api.github.com/repos/wenmar-pro/wenmar-open"
api_dir="${RELEASE_API_DIR:-}"
data_version=""

fail() {
  echo "release: $*" >&2
  exit 1
}

while [ $# -gt 0 ]; do
  case "$1" in
    --data-version)
      [ $# -ge 2 ] || fail "--data-version needs a value, such as --data-version 2026.10."
      data_version="$2"
      shift 2
    ;;
    --data-version=*)
      data_version="${1#--data-version=}"
      shift
    ;;
    -h|--help)
      sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
    ;;
    *) fail "unknown argument $1. Pass --data-version YYYY.MM, or nothing." ;;
  esac
done

# get <path-and-query>: GitHub's public API, with a token when one happens to
# be set. The Authorization header is omitted rather than sent empty.
get() {
  if [ -n "$api_dir" ]; then
    # The canned answer is named for the path alone: a query string is not part
    # of a file name, and the test writes one file per query it wants answered.
    cat "$api_dir/${1%%\?*}"
    return
  fi
  if [ -n "${GH_TOKEN:-}" ]; then
    curl -fsSL -H "Authorization: Bearer $GH_TOKEN" "$api/$1"
  else
    curl -fsSL "$api/$1"
  fi
}

# 1. The tree this makes a release from.
branch=$(git branch --show-current)
[ "$branch" = "main" ] ||
  fail "a release is made from main, and this is on ${branch:-a detached HEAD}."
[ -z "$(git status --porcelain)" ] ||
  fail "the working tree has changes. Commit or stash them, then run this again."
git fetch --quiet origin main
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] ||
  fail "main is not what origin/main has. Push or pull, then run this again."

# 2. CI for this commit, on main. A run still going is not finished, and a red
# one releases nothing.
sha=$(git rev-parse HEAD)
runs=$(get "actions/runs?head_sha=$sha&per_page=20")
# The query already asks for this commit; the filter is repeated here so the
# check is this script's own and does not rest on the query being honoured.
ci_id=$(printf '%s' "$runs" |
  jq -r --arg sha "$sha" \
    '[.workflow_runs[] | select(.name == "CI" and .head_branch == "main" and .head_sha == $sha)] |
     sort_by(.created_at) | last | .id // empty')
[ -n "$ci_id" ] ||
  fail "no CI run for $(git rev-parse --short "$sha") on main. Push, wait for it, then run this again."
conclusion=$(printf '%s' "$runs" | jq -r --argjson id "$ci_id" '
  [.workflow_runs[] | select(.id == $id)] | .[0].conclusion // "not finished"')
ci_url=$(printf '%s' "$runs" | jq -r --argjson id "$ci_id" '
  [.workflow_runs[] | select(.id == $id)] | .[0].html_url // ""')
if [ "$conclusion" != "success" ]; then
  echo "CI run $ci_id is $conclusion." >&2
  [ -n "$ci_url" ] && echo "$ci_url" >&2
  fail "nothing is tagged until CI has passed on this commit."
fi
echo "CI run $ci_id passed."

# 3. The data version to record. A rebuild such as data-2026.10.1 is not a
# plain month and its file has a name no release builds by hand, so the filter
# is anchored at both ends.
if [ -z "$data_version" ]; then
  tags=$(get "releases?per_page=30" | jq -r '.[].tag_name')
  data_version=$(printf '%s\n' "$tags" |
    sed -n 's/^data-\([0-9]\{4\}\.[0-9]\{2\}\)$/\1/p' | sort -r | head -n 1)
  [ -n "$data_version" ] ||
    fail "no data-YYYY.MM release to name. Run data-release.yml first, or pass --data-version YYYY.MM."
  echo "Data version $data_version, from the newest data release."
else
  echo "$data_version" | grep -Eq '^[0-9]{4}\.[0-9]{2}(\.[0-9]+)?$' ||
    fail "--data-version is $data_version, which is not a data version like 2026.10 or 2026.10.1."
  echo "Data version $data_version, as given."
fi

# 4. The bump, which decides the version and writes every file. It commits
# nothing.
output=$(DATA_VERSION="$data_version" scripts/release-bump.sh)
printf '%s\n' "$output"
version=$(printf '%s\n' "$output" | sed -n 's/^version=//p' | tail -n 1)
[ -n "$version" ] || {
  echo "Nothing under [Unreleased]. A month with nothing to say is not a release."
  exit 0
}
tag="v$version"

# 5. A version that is already released must not be tagged again: gh release
# create would refuse it only after the crates were published.
if scripts/tag-exists.sh "$tag"; then
  fail "$tag is already on origin. That version is released."
else
  status=$?
  [ "$status" -eq 1 ] ||
    fail "could not read the tags on origin, so this does not know whether $tag is taken."
fi

# 6. The gate a real release has to pass, on the tree this has just written.
RELEASE_TAG="$tag" scripts/release-check.sh

# 7. One commit and one tag, then stop.
git add -A
git commit -m "chore: release $tag" -m "Cut from CHANGELOG.md's [Unreleased] section."
git tag "$tag"

cat <<EOF

Release $tag is committed and tagged here, on main. Nothing has been pushed.

Read it before you push:

  git show --stat $tag

Then, to publish:

  git push origin main
  git push origin $tag

Pushing the tag starts .github/workflows/release.yml, which publishes to
crates.io, npm, GitHub and the binary archives. If a publishing job fails, use
"Re-run failed jobs"; each step skips what is already published.
EOF