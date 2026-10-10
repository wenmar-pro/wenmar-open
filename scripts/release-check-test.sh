#!/usr/bin/env bash
# Tests scripts/release-check.sh against states it must accept or refuse.
# It publishes nothing, tags nothing and pushes nothing.
#
#   scripts/release-check-test.sh
#
# Each case runs in a throwaway clone of this repository, with this working
# tree's uncommitted changes applied, so nothing here is modified. The clone
# builds into this repository's target/ and so compiles nothing twice.
#
# Cases:
#   version_change_not_committed  docs/releasing.md runs the check before the
#                                 commit: steps 1 and 2 done, nothing committed
#   action_at_a_moving_tag        an action in release.yml named by a tag
#   checkout_keeps_credentials    a checkout without persist-credentials: false
#   server_json_version_disagrees  server.json still at the previous version
#   published_names_the_deleted_crate
#                                 release-check.sh would publish
#                                 wenmar-open-turso, which the workspace no
#                                 longer has
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
workflow=.github/workflows/release.yml

git clone --quiet "$root" "$clone"
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
# It also saves a rebuild. Guarded, because on a machine with nothing built
# yet there is no target/ to point at, and the clone builds its own.
if [ -d "$root/target" ]; then
  ln -s "$root/target" "$clone/target"
fi

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

# Runs the check in the clone. Its output goes to $scratch/output.
check() {
  (cd "$clone" && env "$@" scripts/release-check.sh) > "$scratch/output" 2>&1
}

version_change_not_committed() {
  restore
  local old new
  old=$(sed -n '/^\[workspace\.package\]/,/^\[workspace\.dependencies\]/s/^version = "\(.*\)"$/\1/p' "$clone/Cargo.toml")
  new=$(echo "$old" | awk -F. '{ print $1 "." $2 + 1 ".0" }')
  (
    cd "$clone"
    # Step 1 of "Making a release".
    sed -i.bak "s/\"$old\"/\"$new\"/" Cargo.toml && rm Cargo.toml.bak
    (cd clients/js && npm version --no-git-tag-version "$new" > /dev/null)
    cargo update --quiet --offline --workspace
    node -e '
      const fs = require("node:fs");
      const [file, version] = process.argv.slice(1);
      const api = JSON.parse(fs.readFileSync(file, "utf8"));
      api.info.version = version;
      fs.writeFileSync(file, JSON.stringify(api, null, 2) + "\n");
    ' crates/open-server/openapi.json "$new"
    node -e '
      const fs = require("node:fs");
      const [file, version] = process.argv.slice(1);
      const server = JSON.parse(fs.readFileSync(file, "utf8"));
      server.version = version;
      fs.writeFileSync(file, JSON.stringify(server, null, 2) + "\n");
    ' server.json "$new"
    # Step 2.
    awk -v section="## [$new] - $(date +%Y-%m-%d)" '
      /^## \[Unreleased\]/ { print; print ""; print section; next }
      { print }
    ' CHANGELOG.md > CHANGELOG.md.new && mv CHANGELOG.md.new CHANGELOG.md
  )
  if git -C "$clone" diff --quiet -- Cargo.toml; then
    miss "${FUNCNAME[0]}" "the case did not change Cargo.toml"
    return
  fi
  # Step 3.
  if check && grep -Fq "version $new would release" "$scratch/output"; then
    pass "${FUNCNAME[0]}"
  else
    miss "${FUNCNAME[0]}" "the check failed with the release's changes not committed"
  fi
}

# refused <case> <text the refusal must contain>
refused() {
  if check RELEASE_CHECK_UNRELEASED=1; then
    miss "$1" "the check passed"
  elif grep -F "release-check:" "$scratch/output" | grep -Fq "$2"; then
    pass "$1"
  else
    miss "$1" "the check failed, but not with '$2'"
  fi
}

published_names_the_deleted_crate() {
  restore
  # The check must first be right about what it publishes, or putting the
  # deleted crate back in proves nothing.
  grep -Fxq 'PUBLISHED="wenmar-vin wenmar-vehicles wenmar-open-db"' "$clone/scripts/release-check.sh" ||
    { miss "${FUNCNAME[0]}" "scripts/release-check.sh does not publish wenmar-open-db"; return; }
  sed -i.bak 's/"wenmar-vin wenmar-vehicles wenmar-open-db"/"wenmar-vin wenmar-vehicles wenmar-open-turso"/' \
    "$clone/scripts/release-check.sh"
  rm "$clone/scripts/release-check.sh.bak"
  grep -Fxq 'PUBLISHED="wenmar-vin wenmar-vehicles wenmar-open-turso"' "$clone/scripts/release-check.sh" ||
    { miss "${FUNCNAME[0]}" "the case did not change scripts/release-check.sh"; return; }
  refused "${FUNCNAME[0]}" "does not have wenmar-open-turso at version"
}

action_at_a_moving_tag() {
  restore
  sed -i.bak -E 's|(uses: rust-lang/crates-io-auth-action)@.*|\1@v1|' "$clone/$workflow"
  rm "$clone/$workflow.bak"
  grep -Fq "crates-io-auth-action@v1" "$clone/$workflow" ||
    { miss "${FUNCNAME[0]}" "the case did not change $workflow"; return; }
  refused "${FUNCNAME[0]}" "rust-lang/crates-io-auth-action@v1 is not pinned to a commit"
}

checkout_keeps_credentials() {
  restore
  # Drops the line from the last checkout only: every checkout is checked,
  # not only the first.
  awk '
    { lines[NR] = $0 }
    /persist-credentials: false/ { last = NR }
    END { for (i = 1; i <= NR; i++) if (i != last) print lines[i] }
  ' "$clone/$workflow" > "$scratch/workflow" && cp "$scratch/workflow" "$clone/$workflow"
  refused "${FUNCNAME[0]}" "persist-credentials: false"
}

server_json_version_disagrees() {
  restore
  local old new
  old=$(sed -n 's/^  "version": "\(.*\)",$/\1/p' "$clone/server.json")
  new=$(echo "$old" | awk -F. '{ print $1 "." $2 + 1 ".0" }')
  sed -i.bak "s/\"version\": \"$old\"/\"version\": \"$new\"/" "$clone/server.json"
  rm "$clone/server.json.bak"
  grep -Fq "\"version\": \"$new\"" "$clone/server.json" ||
    { miss "${FUNCNAME[0]}" "the case did not change server.json"; return; }
  refused "${FUNCNAME[0]}" "server.json, its version has $new"
}

action_at_a_moving_tag
checkout_keeps_credentials
server_json_version_disagrees
published_names_the_deleted_crate
version_change_not_committed

[ "$failed" = 0 ] || { echo "release-check-test: failed"; exit 1; }
echo "release-check-test: 5 passed. Nothing was published, tagged or pushed."
