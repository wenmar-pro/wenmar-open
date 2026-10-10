#!/usr/bin/env bash
# Tests scripts/release-bump.sh and scripts/tag-exists.sh against the trees
# and tags they must accept or refuse. It commits nothing, tags nothing and
# pushes nothing.
#
#   scripts/release-bump-test.sh
#
# Each case runs in a throwaway clone of this repository, with this working
# tree's uncommitted changes applied, so nothing here is modified. The clone
# builds into this repository's target/ and so compiles nothing twice, and it
# symlinks clients/js/node_modules so `npm ci` is never run.
#
# Cases: the two that check every file, the seven decision cases, the two
# changelog cases, the refusal and reporting cases, and the three tag-exists
# cases. The runner at the bottom is the full list.
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
clone="$scratch/clone"
export CARGO_TARGET_DIR="$root/target"
# With mise, cargo and node are shims that refuse a mise.toml in a directory
# not seen before. The clone's is this repository's own. Without mise this
# variable does nothing.
export MISE_TRUSTED_CONFIG_PATHS="$scratch"

git clone --quiet "$root" "$clone"
# A clone of a shallow repository is shallow too, and the cases below push tags
# into a repository of their own, which git refuses from a shallow clone with
# "shallow update not allowed". That names a git internal and not the cause, so
# say what is wrong while the reader still knows which step is running.
if git -C "$clone" rev-parse --is-shallow-repository | grep -qx true; then
  echo "release-bump-test: this repository is a shallow clone, so the cases that" >&2
  echo "push cannot run. Clone it whole, or run 'git fetch --unshallow' here first." >&2
  exit 1
fi
if ! git diff --quiet HEAD; then
  git diff --binary HEAD | git -C "$clone" apply
fi
# Files not yet added to git: the script under test may be one of them.
git ls-files --others --exclude-standard -z | while IFS= read -r -d '' path; do
  mkdir -p "$clone/$(dirname "$path")"
  cp -p "$path" "$clone/$path"
done
git -C "$clone" add --all
git -C "$clone" -c user.name=test -c user.email=test@example.invalid \
  commit --quiet --allow-empty --no-verify -m "the tree under test"
# Saves a download: the check runs `npm ci` only when this is missing.
if [ -d "$root/clients/js/node_modules" ]; then
  ln -s "$root/clients/js/node_modules" "$clone/clients/js/node_modules"
fi
# clients/js/scripts/build-wasm.mjs reads the built artifact from target/
# under its own repository root and ignores CARGO_TARGET_DIR, so without this
# the clone has no target/ to read and npm run build fails on a missing file.
ln -s "$root/target" "$clone/target"

failed=0
pass() { echo "ok      $1"; }
miss() {
  echo "FAILED  $1: $2"
  sed 's/^/    | /' "$scratch/output" | tail -n 15
  failed=1
}

restore() {
  git -C "$clone" reset --quiet --hard
}

# Runs the script in the clone. Its output, stdout and stderr together, goes
# to $scratch/output.
#
# The assignments (DATA_VERSION=2026.10) go to env and the flags (--patch) go
# to the script. GNU env honours `--` only before the assignments, so it comes
# first: `env DATA_VERSION=2026.10 --patch -- script` would fail with
# "env: '--patch': No such file or directory".
bump() {
  local envs=() flags=() arg
  for arg in "$@"; do
    case "$arg" in
      *=*) envs+=("$arg") ;;
      *) flags+=("$arg") ;;
    esac
  done
  (cd "$clone" && env -- "${envs[@]}" scripts/release-bump.sh "${flags[@]}") > "$scratch/output" 2>&1
}

# The version the run chose, or the empty string.
chose() { sed -n 's/^version=//p' "$scratch/output" | tail -n 1; }

# Replaces the [Unreleased] body with the lines on stdin, leaving an empty
# heading in its place.
unreleased() {
  restore
  cat > "$scratch/body"
  awk -v body="$scratch/body" '
    /^## \[Unreleased\]/ { print; insection = 1; next }
    insection && /^## \[/ {
      print ""
      while ((getline line < body) > 0) print line
      print ""
      insection = 0
    }
    !insection { print }
  ' "$clone/CHANGELOG.md" > "$clone/CHANGELOG.md.new" && mv "$clone/CHANGELOG.md.new" "$clone/CHANGELOG.md"
}

# The version in the tree, read the way the script reads it.
current_version() {
  sed -n '/^\[workspace\.package\]/,/^\[workspace\.dependencies\]/s/^version = "\(.*\)"$/\1/p' \
    "$clone/Cargo.toml"
}

# A.B.C+1 for patch, A.B+1.0 for minor.
bumped_from() {
  awk -F. -v mode="$2" 'BEGIN { OFS = "." }
    mode == "minor" { print $1, $2 + 1, 0; next }
    { print $1, $2, $3 + 1 }' <<< "$1"
}

patch_bump_writes_every_file() {
  local before want
  before=$(current_version)
  want=$(bumped_from "$before" patch)
  printf '### Fixed\n\n- something\n' | unreleased
  bump DATA_VERSION=2026.10 --patch || { miss "${FUNCNAME[0]}" "the script failed"; return; }
  [ "$(chose)" = "$want" ] || { miss "${FUNCNAME[0]}" "chose '$(chose)', wanted $want"; return; }
  for f in Cargo.toml Cargo.lock clients/js/package.json clients/js/package-lock.json \
           crates/open-server/openapi.json server.json CHANGELOG.md; do
    git -C "$clone" diff --quiet -- "$f" && { miss "${FUNCNAME[0]}" "$f did not change"; return; }
  done
  [ "$(node -p "require('$clone/clients/js/package.json').version")" = "$want" ] || { miss "${FUNCNAME[0]}" "package.json disagrees"; return; }
  [ "$(node -p "require('$clone/clients/js/package-lock.json').packages[''].version")" = "$want" ] || { miss "${FUNCNAME[0]}" "package-lock.json disagrees"; return; }
  [ "$(node -p "require('$clone/crates/open-server/openapi.json').info.version")" = "$want" ] || { miss "${FUNCNAME[0]}" "openapi.json disagrees"; return; }
  [ "$(node -p "require('$clone/server.json').version")" = "$want" ] || { miss "${FUNCNAME[0]}" "server.json disagrees"; return; }
  [ "$(sed -n 's/^ *DATA_VERSION: "\(.*\)"$/\1/p' "$clone/config/deploy.yml")" = "2026.10" ] || { miss "${FUNCNAME[0]}" "DATA_VERSION is not 2026.10"; return; }
  pass "${FUNCNAME[0]}"
}

minor_bump_writes_every_file() {
  local before want
  before=$(current_version)
  want=$(bumped_from "$before" minor)
  printf '### Added\n\n- something\n' | unreleased
  bump DATA_VERSION=2026.10 --minor || { miss "${FUNCNAME[0]}" "the script failed"; return; }
  [ "$(chose)" = "$want" ] || { miss "${FUNCNAME[0]}" "chose '$(chose)', wanted $want"; return; }
  pass "${FUNCNAME[0]}"
}

# <body> <mode>: asserts the version chosen is the current one bumped <mode>.
# FUNCNAME[1] is the case calling decides, so the report names the case rather
# than this helper.
decides() {
  local name=${FUNCNAME[1]} mode=$2 want
  printf '%b\n' "$1" | unreleased
  want=$(bumped_from "$(current_version)" "$mode")
  bump DATA_VERSION=2026.10
  [ "$(chose)" = "$want" ] && pass "$name" || miss "$name" "chose '$(chose)', wanted $want"
}
minor_from_added_heading()  { decides '### Added\n\n- a thing\n' minor; }
minor_from_changed_heading() { decides '### Changed\n\n- a thing\n' minor; }
minor_from_new_field()      { decides '### Fixed\n\n- a new field in MetaResponse\n' minor; }
minor_from_new_endpoint()   { decides '### Fixed\n\n- a new endpoint answers /v1/thing\n' minor; }
minor_from_schema_version() { decides '### Fixed\n\n- the data file schema version is 4\n' minor; }
patch_from_fixed_only()     { decides '### Fixed\n\n- a thing\n' patch; }
patch_from_removed_only()   { decides '### Removed\n\n- a thing\n' patch; }

headings_only_unreleased_is_nothing_to_release() {
  printf '### Added\n\nProse, but no bullet.\n' | unreleased
  # unreleased() has already edited the clone's CHANGELOG.md, so "changed
  # nothing" is measured against that, not against the commit.
  local before after
  before=$(git -C "$clone" diff)
  if bump DATA_VERSION=2026.10 --patch; then
    after=$(git -C "$clone" diff)
    if [ -z "$(chose)" ] && [ "$before" = "$after" ]; then
      pass "${FUNCNAME[0]}"
    else
      miss "${FUNCNAME[0]}" "it chose '$(chose)' or changed the tree"
    fi
  else
    miss "${FUNCNAME[0]}" "the script failed instead of reporting nothing to release"
  fi
}

no_unreleased_heading_is_refused() {
  restore
  grep -v '^## \[Unreleased\]' "$clone/CHANGELOG.md" > "$clone/c" && mv "$clone/c" "$clone/CHANGELOG.md"
  if bump DATA_VERSION=2026.10 --patch; then
    miss "${FUNCNAME[0]}" "the script passed"
  elif grep -Fq 'no ## [Unreleased]' "$scratch/output"; then
    pass "${FUNCNAME[0]}"
  else
    miss "${FUNCNAME[0]}" "it failed without naming the missing heading"
  fi
}

missing_file_changes_nothing() {
  printf '### Fixed\n\n- a thing\n' | unreleased
  rm "$clone/clients/js/package.json"
  if bump DATA_VERSION=2026.10 --patch; then
    miss "${FUNCNAME[0]}" "the script passed"
  elif grep -Fq 'clients/js/package.json' "$scratch/output" && git -C "$clone" diff --quiet -- Cargo.toml; then
    pass "${FUNCNAME[0]}"
  else
    miss "${FUNCNAME[0]}" "it did not name the file, or it changed Cargo.toml first"
  fi
}

both_flags_are_refused() {
  printf '### Fixed\n\n- a thing\n' | unreleased
  if bump DATA_VERSION=2026.10 --minor --patch; then
    miss "${FUNCNAME[0]}" "the script passed"
  elif grep -Fq -- '--minor and --patch' "$scratch/output"; then
    pass "${FUNCNAME[0]}"
  else
    miss "${FUNCNAME[0]}" "it did not say the flags contradict"
  fi
}

data_version_must_be_set() {
  printf '### Fixed\n\n- a thing\n' | unreleased
  if bump --patch; then
    miss "${FUNCNAME[0]}" "the script passed"
  elif grep -Fq 'DATA_VERSION' "$scratch/output"; then
    pass "${FUNCNAME[0]}"
  else
    miss "${FUNCNAME[0]}" "it did not name DATA_VERSION"
  fi
}

data_version_is_only_that_line() {
  printf '### Fixed\n\n- a thing\n' | unreleased
  cp "$clone/config/deploy.yml" "$scratch/before"
  bump DATA_VERSION=2026.10 --patch || { miss "${FUNCNAME[0]}" "the script failed"; return; }
  diff <(sed 's/^\( *DATA_VERSION: \).*$/\1X/' "$scratch/before") \
       <(sed 's/^\( *DATA_VERSION: \).*$/\1X/' "$clone/config/deploy.yml") > "$scratch/d" ||
    { miss "${FUNCNAME[0]}" "something else in deploy.yml moved"; cat "$scratch/d"; return; }
  pass "${FUNCNAME[0]}"
}

lock_is_consistent() {
  printf '### Fixed\n\n- a thing\n' | unreleased
  bump DATA_VERSION=2026.10 --patch || { miss "${FUNCNAME[0]}" "the script failed"; return; }
  (cd "$clone" && cargo publish --quiet --locked --allow-dirty --dry-run -p wenmar-vin) > "$scratch/rc" 2>&1 ||
    { miss "${FUNCNAME[0]}" "cargo refused the rewritten lock"; tail -n 15 "$scratch/rc"; return; }
  pass "${FUNCNAME[0]}"
}

release_check_accepts_the_result() {
  printf '### Fixed\n\n- a thing\n' | unreleased
  bump DATA_VERSION=2026.10 --patch || { miss "${FUNCNAME[0]}" "the script failed"; return; }
  (cd "$clone" && env RELEASE_TAG="v$(chose)" scripts/release-check.sh) > "$scratch/rc" 2>&1 ||
    { miss "${FUNCNAME[0]}" "release-check refused the tree"; tail -n 15 "$scratch/rc"; return; }
  pass "${FUNCNAME[0]}"
}

changelog_moves_under_the_version() {
  printf '### Fixed\n\n- a thing\n' | unreleased
  bump DATA_VERSION=2026.10 --patch || { miss "${FUNCNAME[0]}" "the script failed"; return; }
  local v; v=$(chose)
  grep -Fq "## [$v] - $(date +%Y-%m-%d)" "$clone/CHANGELOG.md" ||
    { miss "${FUNCNAME[0]}" "no dated heading for $v"; return; }
  [ "$(grep -c '^- a thing$' "$clone/CHANGELOG.md")" = 1 ] ||
    { miss "${FUNCNAME[0]}" "the entry did not survive exactly once"; return; }
  awk '/^## \[Unreleased\]/{n=1;next} /^## \[/{n=0} n && /^- /{found=1} END{exit found}' "$clone/CHANGELOG.md" ||
    { miss "${FUNCNAME[0]}" "the fresh [Unreleased] is not empty"; return; }
  grep -Fq '## [Unreleased]' "$clone/CHANGELOG.md" ||
    { miss "${FUNCNAME[0]}" "the [Unreleased] heading is gone"; return; }
  pass "${FUNCNAME[0]}"
}

an_existing_section_is_left_alone() {
  printf '### Fixed\n\n- a new thing\n' | unreleased
  bump DATA_VERSION=2026.10 --patch || { miss "${FUNCNAME[0]}" "the script failed"; return; }
  [ "$(grep -Fxc -- '- Data file schema version 3. Files built before this must be rebuilt.' "$clone/CHANGELOG.md")" = 1 ] ||
    { miss "${FUNCNAME[0]}" "the 0.1.0 section moved or lost entries"; return; }
  pass "${FUNCNAME[0]}"
}

tag_exists_finds_a_tag() {
  restore
  git -C "$clone" tag -f v0.0.1 >/dev/null
  # A bare clone of this repository, in the scratch directory: pushing to the
  # real origin would move a tag on GitHub.
  git init --quiet --bare "$scratch/remote.git"
  git -C "$clone" remote set-url origin "$scratch/remote.git"
  git -C "$clone" push --quiet origin v0.0.1
  (cd "$clone" && scripts/tag-exists.sh v0.0.1) >/dev/null 2>&1 &&
    pass "${FUNCNAME[0]}" || miss "${FUNCNAME[0]}" "it did not find a tag that exists"
}

tag_exists_says_no_for_a_new_tag() {
  restore
  git init --quiet --bare "$scratch/remote2.git"
  git -C "$clone" remote set-url origin "$scratch/remote2.git"
  (cd "$clone" && scripts/tag-exists.sh v99.99.99) >/dev/null 2>&1 &&
    miss "${FUNCNAME[0]}" "it claimed a tag that does not exist" ||
    pass "${FUNCNAME[0]}"
}

tag_exists_refuses_two_arguments() {
  (cd "$clone" && scripts/tag-exists.sh v1 v2) >/dev/null 2>&1 &&
    miss "${FUNCNAME[0]}" "it accepted two arguments" ||
    pass "${FUNCNAME[0]}"
}

# An origin that cannot be reached is neither answer: it must not come back as
# "no such tag", or the workflow publishes over an existing tag.
tag_exists_cannot_answer_is_not_no_such_tag() {
  restore
  git -C "$clone" remote set-url origin "$scratch/absent.git"
  local status=0
  (cd "$clone" && scripts/tag-exists.sh v0.1.1) >/dev/null 2>&1 || status=$?
  [ "$status" -eq 2 ] &&
    pass "${FUNCNAME[0]}" ||
    miss "${FUNCNAME[0]}" "an unreachable origin came back as exit $status, not 2"
}

for case in patch_bump_writes_every_file minor_bump_writes_every_file \
  minor_from_added_heading minor_from_changed_heading minor_from_new_field \
  minor_from_new_endpoint minor_from_schema_version patch_from_fixed_only \
  patch_from_removed_only changelog_moves_under_the_version \
  an_existing_section_is_left_alone headings_only_unreleased_is_nothing_to_release \
  no_unreleased_heading_is_refused missing_file_changes_nothing both_flags_are_refused \
  data_version_must_be_set data_version_is_only_that_line lock_is_consistent \
  release_check_accepts_the_result tag_exists_finds_a_tag \
  tag_exists_says_no_for_a_new_tag tag_exists_refuses_two_arguments \
  tag_exists_cannot_answer_is_not_no_such_tag; do
  "$case"
done
exit $failed
