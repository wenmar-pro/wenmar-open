#!/usr/bin/env bash
# Checks that a release would work. It publishes nothing, tags nothing and
# pushes nothing: every registry command below is a dry run.
#
#   scripts/release-check.sh
#
# Environment:
#   RELEASE_TAG=v0.1.0          also require the tag to name the version
#   RELEASE_CHECK_UNRELEASED=1  before the changelog is cut: accept a
#                               non-empty [Unreleased] section instead of a
#                               section for the version
#   RELEASE_CHECK_FULL=1        build every packaged crate, including the
#                               turso adapter (about 2 GB more in target/;
#                               the release workflow sets it)
#
# `mise run release-check` runs `mise run check` and `mise run js` first.
# scripts/release-check-test.sh tests this script; run it after changing it.
set -euo pipefail
cd "$(dirname "$0")/.."

PUBLISHED="wenmar-vin wenmar-vehicles wenmar-open-turso"
export CARGO_INCREMENTAL=0
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT

fail() {
  echo "release-check: $*" >&2
  exit 1
}

step() {
  printf '\n== %s\n' "$*"
}

step "One version everywhere"
version=$(sed -n '/^\[workspace\.package\]/,/^\[workspace\.dependencies\]/s/^version = "\(.*\)"$/\1/p' Cargo.toml)
[ -n "$version" ] || fail "Cargo.toml has no version under [workspace.package]"
echo "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' ||
  fail "the version is $version; a release is three numbers, such as 0.1.0"
echo "Cargo.toml: $version"

for crate in $PUBLISHED; do
  grep -Fxq "$crate = { version = \"$version\", path = \"crates/$crate\" }" Cargo.toml ||
    fail "[workspace.dependencies] in Cargo.toml does not have $crate at version $version"
done

node - "$version" <<'EOF' || fail "the versions disagree"
const fs = require("node:fs");
const version = process.argv[2];
const read = (path) => JSON.parse(fs.readFileSync(path, "utf8"));
const lock = read("clients/js/package-lock.json");
const found = {
  "clients/js/package.json": read("clients/js/package.json").version,
  "clients/js/package-lock.json": lock.version,
  "clients/js/package-lock.json, its root package": lock.packages[""].version,
  "crates/open-server/openapi.json, info.version": read("crates/open-server/openapi.json").info.version,
};
let ok = true;
for (const [where, has] of Object.entries(found)) {
  console.log(`${where}: ${has}`);
  if (has !== version) {
    console.error(`release-check: ${where} has ${has}, Cargo.toml has ${version}`);
    ok = false;
  }
}
process.exit(ok ? 0 : 1);
EOF

if [ -n "${RELEASE_TAG:-}" ]; then
  [ "$RELEASE_TAG" = "v$version" ] ||
    fail "the tag is $RELEASE_TAG and the version is $version: the tag must be v$version"
  echo "tag: $RELEASE_TAG"
fi

step "The release workflow's actions"
# Every workflow a release runs through: release.yml publishes, and
# monthly-release.yml pushes the tag that starts it. The jobs that publish may
# ask crates.io and npm for a publishing token. A tag such as v4 can be moved
# to other code by whoever controls the action; a commit cannot. A checkout
# that keeps its credentials leaves the job's GitHub token in .git/config for
# every later step to read. monthly-release.yml holds a write token of its own,
# so it must keep no credentials either.
for workflow in .github/workflows/release.yml .github/workflows/monthly-release.yml; do
  [ -f "$workflow" ] || continue
  node - "$workflow" <<'EOF' || fail "a release workflow's actions are not as a release needs them"
const fs = require("node:fs");
const file = process.argv[2];
const lines = fs.readFileSync(file, "utf8").split("\n");
const indent = (line) => line.length - line.trimStart().length;
let ok = true;
let count = 0;
const refuse = (index, message) => {
  console.error(`release-check: ${file}:${index + 1}: ${message}`);
  ok = false;
};
lines.forEach((line, index) => {
  const uses = line.match(/^\s*(?:- )?uses:\s*(\S+)\s*(?:#\s*(.*?)\s*)?$/);
  if (!uses) return;
  count += 1;
  const [, action, comment] = uses;
  const [name, ref] = action.split("@");
  console.log(`${action}${comment ? ` (${comment})` : ""}`);
  if (!/^[0-9a-f]{40}$/.test(ref ?? "")) {
    refuse(index, `${action} is not pinned to a commit: write ${name}@<the 40 characters of the commit> # <version>`);
  } else if (!/^v\d+\.\d+\.\d+$/.test(comment ?? "")) {
    refuse(index, `${name} is pinned to a commit with no comment saying which version it is, such as # v4.4.0`);
  }
  if (name !== "actions/checkout") return;
  // The step is the lines from its "- " to the next line indented no deeper.
  let first = index;
  while (!/^\s*- /.test(lines[first])) first -= 1;
  let last = first + 1;
  while (last < lines.length && (lines[last].trim() === "" || indent(lines[last]) > indent(lines[first]))) last += 1;
  if (!lines.slice(first, last).some((inside) => /^\s*persist-credentials:\s*false\s*$/.test(inside))) {
    refuse(index, "this checkout does not have persist-credentials: false");
  }
});
if (count === 0) refuse(0, "no action found; has the file changed shape?");
process.exit(ok ? 0 : 1);
EOF
done

step "Which crates are published"
cargo metadata --no-deps --format-version 1 --locked > "$scratch/metadata.json"
node - "$version" "$PUBLISHED" "$scratch/metadata.json" <<'EOF' || fail "the crates are not as a release needs them"
const fs = require("node:fs");
const [version, published, file] = process.argv.slice(2);
const wanted = published.split(" ").sort();
const { packages } = JSON.parse(fs.readFileSync(file, "utf8"));
let ok = true;
const publishable = [];
for (const crate of packages) {
  // `publish = false` is an empty list here; no `publish` key is null.
  const publishes = crate.publish === null;
  console.log(`${crate.name} ${crate.version} ${publishes ? "published" : "publish = false"}`);
  if (publishes) publishable.push(crate.name);
  if (crate.version !== version) {
    console.error(`release-check: ${crate.name} is ${crate.version}, the workspace is ${version}`);
    ok = false;
  }
  if (!publishes) continue;
  for (const field of ["description", "license", "repository", "readme"]) {
    if (!crate[field]) {
      console.error(`release-check: ${crate.name} has no ${field}`);
      ok = false;
    }
  }
  for (const dependency of crate.dependencies) {
    // A dependency with a path is one of this workspace's crates. A
    // published crate may depend only on published ones, except in tests.
    if (dependency.path && dependency.kind !== "dev" && !wanted.includes(dependency.name)) {
      console.error(`release-check: ${crate.name} depends on ${dependency.name}, which is not published`);
      ok = false;
    }
  }
}
if (JSON.stringify(publishable.sort()) !== JSON.stringify(wanted)) {
  console.error(
    `release-check: the crates without publish = false are ${publishable.join(", ")}; expected ${wanted.join(", ")}`,
  );
  ok = false;
}
process.exit(ok ? 0 : 1);
EOF

step "wenmar-vin without sqlite depends on serde and thiserror only"
direct=$(cargo tree --locked -p wenmar-vin -e normal --depth 1 --prefix none |
  sed '1d' | cut -d' ' -f1 | sort -u | tr '\n' ' ')
echo "wenmar-vin: $direct"
[ "$direct" = "serde thiserror " ] || fail "wenmar-vin's dependencies are: $direct"

step "The changelog"
if [ "${RELEASE_CHECK_UNRELEASED:-0}" = "1" ]; then
  awk '/^## \[Unreleased\]/{inside=1; next} /^## \[/{inside=0} inside && /^- /{found=1} END{exit !found}' CHANGELOG.md ||
    fail "CHANGELOG.md has nothing under [Unreleased]"
  echo "[Unreleased] has entries. At release they move under: ## [$version] - $(date +%Y-%m-%d)"
else
  grep -Eq "^## \[$version\] - [0-9]{4}-[0-9]{2}-[0-9]{2}$" CHANGELOG.md ||
    fail "CHANGELOG.md has no section '## [$version] - YYYY-MM-DD'. See docs/releasing.md."
  grep -E "^## \[$version\]" CHANGELOG.md
fi

step "What each crate's package holds"
for crate in $PUBLISHED; do
  echo "-- $crate"
  cargo package --list --locked --allow-dirty -p "$crate"
done

# --allow-dirty: docs/releasing.md runs this check before the release's
# changes are committed, and without the flag cargo refuses a Cargo.toml that
# differs from the last commit. A real `cargo publish` never has the flag.
step "cargo publish --dry-run"
if [ "${RELEASE_CHECK_FULL:-0}" = "1" ]; then
  cargo publish --dry-run --locked --allow-dirty -p wenmar-vin -p wenmar-vehicles -p wenmar-open-turso
else
  # The two small crates are built from their packages. The turso adapter is
  # packaged and its dependencies resolved, but it is not built: building it
  # from its package needs a second copy of turso in target/.
  cargo publish --dry-run --locked --allow-dirty -p wenmar-vin -p wenmar-vehicles
  cargo publish --dry-run --locked --allow-dirty --no-verify -p wenmar-vin -p wenmar-vehicles -p wenmar-open-turso
  echo "wenmar-open-turso was packaged but not built from its package."
  echo "RELEASE_CHECK_FULL=1 builds it; the release workflow does."
fi

step "npm pack --dry-run"
(
  cd clients/js
  [ -d node_modules ] || npm ci --no-audit --no-fund
  # prepack is "npm run build", which now prints a line of its own to stdout
  # and would end up inside the JSON. Build to stderr, then pack without it.
  npm run build >&2
  npm pack --dry-run --json --ignore-scripts > "$scratch/pack.json"
)
node - "$version" "$scratch/pack.json" <<'EOF' || fail "the npm package is not as a release needs it"
const fs = require("node:fs");
const [version, file] = process.argv.slice(2);
const [packed] = JSON.parse(fs.readFileSync(file, "utf8"));
const allowed =
  /^(package\.json|README\.md|LICENSE|dist\/[a-z]+\.(js|d\.ts)|dist\/offline\/[a-z-]+\.(js|d\.ts)|dist\/offline\/wenmar_open\.wasm)$/;
let ok = packed.name === "wenmar-open" && packed.version === version;
console.log(`${packed.name}@${packed.version}: ${packed.files.length} files, ${packed.size} bytes packed`);
for (const { path } of packed.files) {
  console.log(`  ${path}`);
  if (!allowed.test(path)) {
    console.error(`release-check: the npm package would include ${path}`);
    ok = false;
  }
}
for (const needed of [
  "package.json",
  "README.md",
  "LICENSE",
  "dist/index.js",
  "dist/index.d.ts",
  "dist/offline/index.js",
  "dist/offline/node.js",
  "dist/offline/wasm-inline.js",
  "dist/offline/wenmar_open.wasm",
]) {
  if (!packed.files.some(({ path }) => path === needed)) {
    console.error(`release-check: the npm package would lack ${needed}`);
    ok = false;
  }
}
process.exit(ok ? 0 : 1);
EOF

printf '\nrelease-check: version %s would release. Nothing was published, tagged or pushed.\n' "$version"
