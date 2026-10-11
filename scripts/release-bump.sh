#!/usr/bin/env bash
# Bumps every version the release needs, cuts the changelog and names the new
# data version in config/deploy.yml. It commits nothing, tags nothing, pushes
# nothing, publishes nothing, and touches nothing else in the tree.
#
#   DATA_VERSION=2026.10 scripts/release-bump.sh [--minor | --patch]
#
# Environment:
#   DATA_VERSION   required, the data version to record, YYYY.MM or YYYY.MM.N
#
# With no flag the version bump is decided from the changelog's [Unreleased]
# section; --minor and --patch force it and both together are refused.
#
# Output: exit 0 with version=X.Y.Z as the last line when it bumped something;
# exit 0 with version= when there is nothing to release; exit 1 with
# release-bump: on stderr when it refuses. Changed files are listed on stdout
# as changed=<path>, one per line, before the version line.
#
# scripts/release-bump-test.sh tests this script; run it after changing it.
set -euo pipefail
cd "$(dirname "$0")/.."

fail() {
  echo "release-bump: $*" >&2
  exit 1
}

minor=""
patch=""
for arg in "$@"; do
  case "$arg" in
    --minor) minor=1 ;;
    --patch) patch=1 ;;
    *) fail "unknown argument $arg. Pass --minor, --patch, or neither to decide from the changelog." ;;
  esac
done
[ -z "${minor:-}" ] || [ -z "${patch:-}" ] ||
  fail "--minor and --patch ask for different versions. Pass one, or neither."
[ -n "${DATA_VERSION:-}" ] ||
  fail "DATA_VERSION is not set. It is the data version to record, such as 2026.10."
echo "$DATA_VERSION" | grep -Eq '^[0-9]{4}\.[0-9]{2}(\.[0-9]+)?$' ||
  fail "DATA_VERSION is $DATA_VERSION, which is not a data version like 2026.10 or 2026.10.1."

unreleased_body() {
  [ -f CHANGELOG.md ] || fail "CHANGELOG.md is not here."
  awk '/^## \[Unreleased\]/ { inside = 1; next } /^## \[/ { inside = 0 } inside { print }' CHANGELOG.md
}

# Both patterns live here alone, so a change to them is one edit. The anchors
# keep `### Added` from matching `### Added, later`.
wants_minor() {
  local body; body=$(unreleased_body)
  echo "$body" | grep -Eq '^### (Added|Changed)$' && return 0
  echo "$body" | grep -Eqi 'schema version|new field|new endpoint|/v2' && return 0
  return 1
}

# Empty when it holds no line starting `- `, the rule release-check.sh already
# uses for RELEASE_CHECK_UNRELEASED=1, so the two agree about what "something
# to release" means.
unreleased_is_empty() { [ -z "$(unreleased_body | grep '^- ' || true)" ]; }

# A missing [Unreleased] heading must be refused, not read as an empty body.
grep -q '^## \[Unreleased\]' CHANGELOG.md ||
  fail "CHANGELOG.md has no ## [Unreleased] heading. Nothing was changed."

if unreleased_is_empty; then
  echo "version="
  exit 0
fi

if [ -n "${minor:-}" ]; then
  mode=minor
elif [ -n "${patch:-}" ]; then
  mode=patch
elif wants_minor; then
  mode=minor
else
  mode=patch
fi

current_version() {
  local v
  v=$(sed -n '/^\[workspace\.package\]/,/^\[workspace\.dependencies\]/s/^version = "\(.*\)"$/\1/p' Cargo.toml)
  [ -n "$v" ] || fail "Cargo.toml has no version under [workspace.package]."
  echo "$v"
}

next_version() {
  local v; v=$(current_version)
  echo "$v" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' ||
    fail "the version is $v, which is not three numbers such as 0.1.0."
  awk -F. -v mode="$1" 'BEGIN { OFS = "." }
    mode == "minor" { print $1, $2 + 1, 0; next }
    { print $1, $2, $3 + 1 }' <<< "$v"
}

old=$(current_version)
new=$(next_version "$mode")

# Every file is checked before any write, so a missing file cannot leave a
# half-written tree.
required_files=(Cargo.toml Cargo.lock CHANGELOG.md server.json \
  clients/js/package.json clients/js/package-lock.json \
  crates/open-server/openapi.json config/deploy.yml)
for f in "${required_files[@]}"; do
  [ -f "$f" ] || fail "$f is not here. Nothing was changed."
done

changed=()

# Cargo.toml: the four lines carrying the version in quotes.
sed -i.bak "s/\"$old\"/\"$new\"/g" Cargo.toml && rm Cargo.toml.bak
[ "$(grep -c "\"$new\"" Cargo.toml)" = 4 ] ||
  fail "Cargo.toml has the new version in $(grep -c "\"$new\"" Cargo.toml) places, not 4."
changed+=(Cargo.toml)

# clients/js: writes package.json and both fields of package-lock.json at once.
(cd clients/js && npm version --no-git-tag-version "$new")
changed+=(clients/js/package.json clients/js/package-lock.json)

# Cargo.lock: rewrites all nine workspace crates' versions.
cargo update --quiet --offline --workspace
changed+=(Cargo.lock)

# openapi.json: info.version comes from CARGO_PKG_VERSION, so regenerate rather
# than hand-edit.
UPDATE_OPENAPI=1 cargo test --quiet -p open-server --test it openapi
changed+=(crates/open-server/openapi.json)

# The npm fixtures record the version as well. offline-cases.json answers with
# server_version, which is CARGO_PKG_VERSION baked in when the crate is
# compiled, and a version change alone does not make cargo rebuild it — so the
# source is touched first, or this writes the old version back and the fixture
# test passes on a stale file. The fixture test builds its own in-memory
# database, so it needs no data file.
touch crates/wenmar-open-wasm/src/engine.rs
UPDATE_FIXTURES=1 cargo test --quiet -p wenmar-open-wasm --test fixture
changed+=(clients/js/test/fixtures/offline-cases.json clients/js/test/fixtures/offline.sql)

# server.json: the MCP registry file. release-check.sh reads its version and
# the server's own tests assert it against CARGO_PKG_VERSION, so a miss here
# fails the release rather than shipping.
node -e '
  const fs = require("node:fs");
  const [file, version] = process.argv.slice(1);
  const server = JSON.parse(fs.readFileSync(file, "utf8"));
  server.version = version;
  fs.writeFileSync(file, JSON.stringify(server, null, 2) + "\n");
' server.json "$new"
changed+=(server.json)

# config/deploy.yml: the one DATA_VERSION line under builder.args.
[ "$(grep -c '^ *DATA_VERSION: ' config/deploy.yml)" = 1 ] ||
  fail "config/deploy.yml has $(grep -c '^ *DATA_VERSION: ' config/deploy.yml) DATA_VERSION lines, not 1."
sed -i.bak -E "s/^( *DATA_VERSION: ).*\$/\1\"$DATA_VERSION\"/" config/deploy.yml && rm config/deploy.yml.bak
changed+=(config/deploy.yml)

# CHANGELOG.md: the [Unreleased] body moves under a new dated heading, leaving
# the empty heading for next month. The body is written to a file and read
# back as the lines go by, so it moves rather than copies.
today=$(date +%Y-%m-%d)
scratch=$(mktemp)
unreleased_body > "$scratch.body"
trap 'rm -f "$scratch" "$scratch.body"' EXIT
awk -v version="$new" -v date="$today" -v body="$scratch.body" '
  # The empty heading, which stays for the next month.
  /^## \[Unreleased\]/ && !opened {
    print
    print ""
    print "## [" version "] - " date
    opened = 1
    # The entries, read from the file as they are needed.
    while ((getline line < body) > 0) print line
    close(body)
    skipping = 1
    next
  }
  # The body has been copied above; skip the original.
  skipping && /^## \[/ { skipping = 0 }
  skipping { next }
  { print }
' CHANGELOG.md > "$scratch" && mv "$scratch" CHANGELOG.md
changed+=(CHANGELOG.md)

for f in "${changed[@]}"; do echo "changed=$f"; done
echo "version=$new"
