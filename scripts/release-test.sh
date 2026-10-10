#!/usr/bin/env bash
# Tests scripts/release.sh against the states it must refuse, and against the
# release it must make. It publishes nothing, pushes nothing to origin, and
# touches nothing in this working tree.
#
#   scripts/release-test.sh
#
# Each case runs in a throwaway clone of this repository, with this working
# tree's uncommitted changes applied. GitHub's API is answered from files in
# the scratch directory through RELEASE_API_DIR, and scripts/release-check.sh is
# stood in for by a stub that records the tag it was given: the check itself
# has its own test, and a real one here would compile the whole workspace once
# per case.
#
# Cases:
#   refuses_off_main                     a release is cut from main
#   refuses_a_dirty_tree                 an unfinished change is not released
#   refuses_when_main_is_unpushed        the tag must name what origin has
#   refuses_when_ci_is_red               nothing is tagged on a red CI
#   refuses_when_ci_has_not_finished     a run still going is not a pass
#   refuses_when_ci_is_missing           no run for this commit at all
#   refuses_when_ci_is_for_another_commit  another commit's pass is not this one's
#   refuses_when_no_data_release         a data version comes from somewhere
#   refuses_a_bad_data_version           --data-version is checked
#   refuses_a_malformed_data_version     it is an argument like any other
#   refuses_an_already_published_tag     a released version is not tagged twice
#   refuses_when_the_tags_cannot_be_read an unanswered question ends the run
#   picks_the_newest_plain_data_release  a rebuild does not win
#   an_empty_unreleased_is_no_release    a month with nothing to say
#   makes_the_commit_and_the_tag         the whole thing, end to end
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
clone="$scratch/clone"
export CARGO_TARGET_DIR="$root/target"
export MISE_TRUSTED_CONFIG_PATHS="$scratch"

git clone --quiet "$root" "$clone"
# A clone of a shallow repository is shallow too, and git refuses a push from a
# shallow clone with "shallow update not allowed". This script pushes main and
# tags below, so it cannot run in one. Name the cause rather than let the first
# push fail with an error about git internals.
if git -C "$clone" rev-parse --is-shallow-repository | grep -qx true; then
  echo "release-test: this repository is a shallow clone, so it cannot push." >&2
  echo "Clone it whole, or run 'git fetch --unshallow' here first." >&2
  exit 1
fi
if ! git diff --quiet HEAD; then
  git diff --binary HEAD | git -C "$clone" apply
fi
git ls-files --others --exclude-standard -z | while IFS= read -r -d '' path; do
  mkdir -p "$clone/$(dirname "$path")"
  cp -p "$path" "$clone/$path"
done
git -C "$clone" add --all
git -C "$clone" -c user.name=test -c user.email=test@example.invalid \
  commit --quiet --allow-empty --no-verify -m "the tree under test"

# A remote of its own, so no case can reach GitHub. It is bare because pushing
# a tag into a non-bare repository is refused.
git init --quiet --bare "$scratch/origin.git"
git -C "$clone" remote set-url origin "$scratch/origin.git"
git -C "$clone" push --quiet origin main
# The commit every case starts from. A case that makes a real release commits
# and tags, so resetting to HEAD would leave the next case starting from a
# bumped tree.
base=$(git -C "$clone" rev-parse HEAD)

# GitHub's API, answered from files named for the path a step asks for. There
# is no network here and no way to reach one.
api="$scratch/api"
mkdir -p "$api/actions"

# ci_runs <sha> <conclusion-json> <status>. The conclusion is written as JSON
# unquoted, because a run still going has null there, not a string.
ci_runs() {
  cat > "$api/actions/runs" <<EOF
{"workflow_runs": [
  {"id": 1, "name": "CI", "head_branch": "main", "head_sha": "$1",
   "status": "$3", "conclusion": $2, "created_at": "2026-10-09T09:00:00Z",
   "html_url": "https://github.com/wenmar-pro/wenmar-open/actions/runs/1"}
]}
EOF
}

# data_releases <tag>... The REST API spells it tag_name, which is not the
# camelCase gh prints for --json tagName.
data_releases() {
  printf '%s\n' "$@" | jq -R '{tag_name: .}' | jq -s '.' > "$api/releases"
}

# The API's files start empty for each case, so nothing a case does can be
# answered by what an earlier case left behind.
reset_api() {
  rm -rf "$api"
  mkdir -p "$api/actions"
}

# release-check.sh stood in for: it is checked by its own test, and running the
# real one here would compile the workspace in every case. Written inside
# prepare, because reset --hard puts the real one back.
stub_release_check() {
  cat > "$clone/scripts/release-check.sh" <<'EOF'
#!/usr/bin/env bash
echo "release-check: RELEASE_TAG=$RELEASE_TAG"
exit 0
EOF
  chmod +x "$clone/scripts/release-check.sh"
}

# What this working tree's changelog looks like under a case. Committed, so the
# tree the script sees is clean: a dirty tree is a refusal of its own, and
# every case here has to get past it.
with_changelog() {
  printf '# Changelog\n\n## [Unreleased]\n%s\n## [0.1.0] - 2026-10-01\n\n- something\n' "$1" \
    > "$clone/CHANGELOG.md"
  git -C "$clone" add -A
  # --allow-empty: a case that reuses a changelog the tree already has would
  # otherwise fail on "nothing to commit" and take the whole run with it.
  git -C "$clone" -c user.name=test -c user.email=test@example.invalid \
    commit --quiet --allow-empty --no-verify -m "the changelog under test"
}

commit_all() {
  git -C "$clone" add -A
  git -C "$clone" -c user.name=test -c user.email=test@example.invalid \
    commit --quiet --allow-empty --no-verify -m "$1"
}

failed=0
pass() { echo "ok      $1"; }
miss() {
  echo "FAILED  $1: $2"
  sed 's/^/    | /' "$scratch/output" | tail -n 15
  failed=1
}

# The version the tree carries, and the patch version above it.
current() {
  sed -n '/^\[workspace\.package\]/,/^\[workspace\.dependencies\]/s/^version = "\(.*\)"$/\1/p' \
    "$clone/Cargo.toml"
}
patched() { echo "$(current)" | awk -F. '{ print $1 "." $2 "." $3 + 1 }'; }

body=$'\n### Fixed\n\n- a thing\n'

# What prepare builds, and what a case changes. Reset before every case,
# because an assignment in front of a function call outlives it in bash.
ci_conclusion='"success"'
ci_status=completed
data_tags="data-2026.10"
changelog_body="$body"

# A releasable clone: on main, clean, pushed, green CI, one data release, and a
# changelog with one bullet under [Unreleased]. A case changes one of these
# before calling it.
prepare() {
  # On main, because a release is cut from main and the script says so.
  git -C "$clone" switch --quiet main
  git -C "$clone" reset --quiet --hard "$base"
  stub_release_check
  # Tags from an earlier case must not make this one look released.
  while read -r t; do git -C "$clone" tag -d "$t" >/dev/null; done < <(git -C "$clone" tag)
  # A tag an earlier case pushed is still on origin, and a local delete does
  # not touch it. Each one is deleted by name, because a wildcard refspec is
  # not one git accepts.
  while read -r t; do
    git -C "$clone" push --quiet origin --delete "refs/tags/$t" >/dev/null 2>&1 || true
  done < <(git -C "$clone" ls-remote --tags origin | sed 's|.*refs/tags/||')
  reset_api
  with_changelog "$changelog_body"
  git -C "$clone" push --quiet --force origin main
  ci_runs "$(git -C "$clone" rev-parse HEAD)" "$ci_conclusion" "$ci_status"
  if [ -n "$data_tags" ]; then
    data_releases $data_tags
  else
    data_releases
  fi
}

# The script under test, in the clone, with the API answered from files.
attempt() {
  last_status=0
  (cd "$clone" && env RELEASE_API_DIR="$api" scripts/release.sh "$@") \
    > "$scratch/output" 2>&1 || last_status=$?
}

# refused <name> <text the refusal must contain>
refused() {
  if [ "$last_status" -eq 0 ]; then
    miss "$1" "it passed"
  elif grep -F "release: " "$scratch/output" | grep -Fq "$2"; then
    pass "$1"
  else
    miss "$1" "it failed, but not with '$2'"
  fi
}

refuses_off_main() {
  prepare
  git -C "$clone" switch --quiet --detach
  attempt
  refused "${FUNCNAME[0]}" "a detached HEAD"
}

refuses_a_dirty_tree() {
  prepare
  echo "unfinished" >> "$clone/README.md"
  attempt
  refused "${FUNCNAME[0]}" "working tree has changes"
}

# A commit origin does not have: the tag would name something nobody else can
# fetch, and the version would be cut from a tree CI never saw.
refuses_when_main_is_unpushed() {
  prepare
  commit_all "a commit origin lacks"
  attempt
  refused "${FUNCNAME[0]}" "origin/main"
}

refuses_when_ci_is_red() {
  ci_conclusion='"failure"'
  prepare
  attempt
  refused "${FUNCNAME[0]}" "CI has passed"
}

refuses_when_ci_has_not_finished() {
  ci_status=in_progress
  ci_conclusion=null
  prepare
  attempt
  refused "${FUNCNAME[0]}" "CI has passed"
}

refuses_when_ci_is_missing() {
  prepare
  ci_runs 0000000000000000000000000000000000000000 '"success"' completed
  attempt
  refused "${FUNCNAME[0]}" "no CI run"
}

# Another commit's green run says nothing about this one.
refuses_when_ci_is_for_another_commit() {
  prepare
  ci_runs 1111111111111111111111111111111111111111 '"success"' completed
  attempt
  refused "${FUNCNAME[0]}" "no CI run"
}

refuses_when_no_data_release() {
  data_tags=""
  prepare
  attempt
  refused "${FUNCNAME[0]}" "no data-YYYY.MM release"
}

refuses_a_bad_data_version() {
  prepare
  attempt --data-version 2026-10-1-1
  refused "${FUNCNAME[0]}" "is not a data version"
}

refuses_a_malformed_data_version() {
  prepare
  attempt --data-version
  refused "${FUNCNAME[0]}" "needs a value"
}

# The version the bump will choose, already tagged on origin: gh release create
# would refuse it only after the crates were published.
refuses_an_already_published_tag() {
  prepare
  git -C "$clone" tag "v$(patched)"
  git -C "$clone" push --quiet origin "v$(patched)"
  attempt
  refused "${FUNCNAME[0]}" "already on origin"
}

# The one case the review called out: releases are listed newest-first by
# creation, so a rebuild can come before the month it rebuilds. Taking it
# would name a data file no release builds by hand.
picks_the_newest_plain_data_release() {
  data_tags=$'data-2026.10.1\ndata-2026.10\ndata-2026.09'
  prepare
  attempt
  [ "$last_status" -eq 0 ] || { miss "${FUNCNAME[0]}" "it failed"; return; }
  [ "$(sed -n 's/^ *DATA_VERSION: "\(.*\)"$/\1/p' "$clone/config/deploy.yml")" = "2026.10" ] &&
    pass "${FUNCNAME[0]}" ||
    miss "${FUNCNAME[0]}" "DATA_VERSION is not 2026.10"
}

an_empty_unreleased_is_no_release() {
  changelog_body=""
  prepare
  attempt
  if [ "$last_status" -eq 0 ] && grep -Fq "Nothing under [Unreleased]" "$scratch/output" &&
    [ -z "$(git -C "$clone" log --format=%s origin/main..HEAD)" ]; then
    pass "${FUNCNAME[0]}"
  else
    miss "${FUNCNAME[0]}" "it did not stop successfully with nothing committed"
  fi
}

makes_the_commit_and_the_tag() {
  prepare
  # Before the run: afterwards the tree carries the version it chose.
  local want="v$(patched)"
  attempt
  if [ "$last_status" -ne 0 ]; then
    miss "${FUNCNAME[0]}" "the script failed"
    return
  fi
  git -C "$clone" rev-parse -q --verify "refs/tags/$want" >/dev/null ||
    { miss "${FUNCNAME[0]}" "no tag $want"; return; }
  git -C "$clone" log -1 --format=%s | grep -Fq "release $want" ||
    { miss "${FUNCNAME[0]}" "the commit is not named for the release"; return; }
  # The push is the human's: the tag must not have reached origin.
  git -C "$clone" ls-remote --exit-code --tags origin "refs/tags/$want" >/dev/null 2>&1 &&
    { miss "${FUNCNAME[0]}" "the tag reached origin, which the script must not do"; return; }
  grep -Fq "git push origin $want" "$scratch/output" ||
    { miss "${FUNCNAME[0]}" "it did not print the push to run"; return; }
  grep -Fq "RELEASE_TAG=$want" "$scratch/output" ||
    { miss "${FUNCNAME[0]}" "release-check did not see the tag"; return; }
  # The changelog heading carries the version; the tag carries it with a v.
  grep -Fq "## [${want#v}] - $(date +%Y-%m-%d)" "$clone/CHANGELOG.md" ||
    { miss "${FUNCNAME[0]}" "the changelog has no dated heading for $want"; return; }
  [ "$(sed -n 's/^ *DATA_VERSION: "\(.*\)"$/\1/p' "$clone/config/deploy.yml")" = "2026.10" ] ||
    { miss "${FUNCNAME[0]}" "DATA_VERSION is not 2026.10"; return; }
  pass "${FUNCNAME[0]}"
}

for case in refuses_off_main refuses_a_dirty_tree refuses_when_main_is_unpushed \
  refuses_when_ci_is_red refuses_when_ci_has_not_finished refuses_when_ci_is_missing \
  refuses_when_ci_is_for_another_commit refuses_when_no_data_release \
  refuses_a_bad_data_version refuses_a_malformed_data_version \
  refuses_an_already_published_tag picks_the_newest_plain_data_release \
  an_empty_unreleased_is_no_release makes_the_commit_and_the_tag; do
  # Each case starts from the defaults: an assignment in front of prepare
  # outlives the call in bash, so a case that changed one must not leave it
  # changed for the next.
  ci_conclusion='"success"'
  ci_status=completed
  data_tags="data-2026.10"
  changelog_body="$body"
  "$case"
done

[ "$failed" = 0 ] || { echo "release-test: failed"; exit 1; }
echo "release-test: 14 passed. Nothing was pushed, tagged or published."