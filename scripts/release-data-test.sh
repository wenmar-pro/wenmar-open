#!/usr/bin/env bash
# Tests scripts/release-data.sh against the states it must refuse, and against
# the package it must build. It publishes nothing, pushes nothing to origin,
# and touches nothing in this working tree.
#
#   scripts/release-data-test.sh
#
# Each case runs in a throwaway clone of this repository, with this working
# tree's uncommitted changes applied. Three seams keep the network out:
#
#   RELEASE_API_DIR      GitHub's API is answered from files in the scratch
#                        directory, so there is no way to reach a release.
#   RELEASE_ASSETS_URL   points at a directory the test filled with a data
#                        file and its checksum, so curl, sha256sum and gunzip
#                        all really run against a file:// URL.
#   a stub `npm`         first on PATH, answers `npm view` from a file so the
#                        "already on npm" question is asked and answered
#                        without a registry. Everything else falls through to
#                        the real npm, so npm pack and npm install are real.
#
# The data file the cases build from is a real SQLite file with this project's
# own tables and a meta table saying schema 3, data 2026.09 — small enough to
# write in a case, and shaped like the file prepare.mjs is given in production.
#
# Cases:
#   refuses_off_main                       a data package is built from main
#   refuses_a_dirty_tree                   an unreviewed tree is not published
#   refuses_when_no_data_release           a data version comes from somewhere
#   picks_the_newest_plain_data_release    a rebuild does not win
#   refuses_a_bad_data_version             --data-version is checked
#   refuses_a_malformed_data_version       it is an argument like any other
#   refuses_a_missing_checksum_file        the release's checksum must be there
#   refuses_a_file_that_fails_its_checksum a download that is not the file
#   refuses_a_data_file_that_is_not_one    prepare.mjs refuses a stranger
#   refuses_an_already_published_version   npm keeps what it is given
#   refuses_a_package_without_the_data     a placeholder is not publishable
#   builds_and_verifies_the_package        the whole thing, end to end
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
clone="$scratch/clone"
export MISE_TRUSTED_CONFIG_PATHS="$scratch"

git clone --quiet "$root" "$clone"
git -C "$clone" config user.name test
git -C "$clone" config user.email test@example.invalid
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

failed=0
pass() { echo "ok      $1"; }
miss() {
  echo "FAILED  $1: $2"
  sed 's/^/    | /' "$scratch/output" | tail -n 12
  failed=1
}

# The GitHub API, answered from files named for the path a step asks for.
api="$scratch/api"
reset_api() {
  rm -rf "$api"
  mkdir -p "$api"
}

# data_releases <tag>... The REST API spells it tag_name, which is not the
# camelCase gh prints for --json tagName.
data_releases() {
  printf '%s\n' "$@" | jq -R '{tag_name: .}' | jq -s '.' > "$api/releases"
}

# The stub npm. Only `view` is answered here; everything else, including pack
# and install, runs the real npm through the PATH the case runs with.
stub_dir="$scratch/stub"
mkdir -p "$stub_dir"
# The real npm, by absolute path. The stub is first on PATH, so exec'ing a bare
# `npm` would find the stub again and recurse until the run was killed.
real_npm=$(command -v npm)
cat > "$stub_dir/npm" <<EOF
#!/usr/bin/env bash
# Answers \`npm view <pkg>@<version> version\` from \$NPM_VIEW_FILE, and passes
# every other npm invocation to the real npm. Prints nothing when the file
# names no version, which is npm's own answer for "not published".
set -euo pipefail
if [ "\${1:-}" = "view" ]; then
  [ -n "\${NPM_VIEW_FILE:-}" ] && [ -f "\$NPM_VIEW_FILE" ] && cat "\$NPM_VIEW_FILE"
  exit 0
fi
exec "$real_npm" "\$@"
EOF
chmod +x "$stub_dir/npm"

# A data file shaped like the real one: this project's own schema, taken out of
# crates/wenmar-vehicles/src/schema.rs so it cannot drift from it, and a meta
# table saying schema 3 and the month asked for. Small enough to write in a
# case, and readable by prepare.mjs and by the check the script makes.
make_data_file() {
  local out="$1" data_version="${2:-2026.09}"
  rm -f "$out"
  # The schema is Rust string constants, in two crates: the decoder's tables
  # (which hold `meta`) in wenmar-vin, the catalog's in wenmar-vehicles. Take
  # each constant's body, so neither can drift from the code that writes them.
  take_schema() {
    awk '
      /^pub const SCHEMA: &str = "$/ { inside = 1; next }
      inside && /^";$/ { exit }
      inside { print }
    ' "$1"
  }
  {
    take_schema "$root/crates/wenmar-vin/src/sqlite.rs"
    take_schema "$root/crates/wenmar-vehicles/src/schema.rs"
  } > "$scratch/schema.sql"
  [ -s "$scratch/schema.sql" ] || { echo "release-data-test: the schema was not found." >&2; exit 1; }
  node -e '
    const { DatabaseSync } = require("node:sqlite");
    const [file, dataVersion, schemaFile] = process.argv.slice(1);
    const database = new DatabaseSync(file);
    database.exec("PRAGMA journal_mode = OFF");
    database.exec(require("node:fs").readFileSync(schemaFile, "utf8"));
    const rows = [
      ["schema_version", "3"],
      ["data_version", dataVersion],
      ["vpic_release", "vPICList_lite_2026_09"],
      ["built_at", "2026-10-01 00:00:00"],
    ];
    const insert = database.prepare("INSERT INTO meta (key, value) VALUES (?, ?)");
    for (const row of rows) insert.run(...row);
    database.exec(`
      INSERT INTO catalog_type (id, name) VALUES (2, '"'"'Passenger Car'"'"');
      INSERT INTO catalog_make (id, slug, name, norm, rank, types, light)
        VALUES (1, '"'"'honda'"'"', '"'"'Honda'"'"', '"'"'honda'"'"', 132, 4, 1);
      INSERT INTO catalog_model
        (id, make_id, slug, name, norm, year_from, year_to, types, light)
        VALUES (1, 1, '"'"'civic'"'"', '"'"'Civic'"'"', '"'"'civic'"'"', 2018, 2026, 4, 1);
    `);
    database.close();
  ' "$out" "$data_version" "$scratch/schema.sql"
}

# The release's files, gzipped and with its checksum, where the download will
# find them. Returns the directory it filled.
assets="$scratch/assets"
make_assets() {
  local version="$1" source="$2"
  local dir="$assets/data-$version"
  mkdir -p "$dir"
  cp "$source" "$dir/wenmar-open-$version.sqlite3"
  gzip -n -f "$dir/wenmar-open-$version.sqlite3"
  (cd "$dir" && sha256sum "wenmar-open-$version.sqlite3.gz" > "wenmar-open-$version.sqlite3.gz.sha256")
  printf '%s' "$dir"
}

# Every case starts from main, clean, with one data release and files to
# download. A case changes one thing from here.
setup() {
  git -C "$clone" checkout --quiet main
  git -C "$clone" reset --quiet --hard "$base"
  git -C "$clone" clean -qfdx -e node_modules
  reset_api
  data_releases data-2026.09
  npm_file="$scratch/npm-view"
  : > "$npm_file"
  good_data="$scratch/wenmar-open-2026.09.sqlite3"
  make_data_file "$good_data" 2026.09
  make_assets 2026.09 "$good_data" >/dev/null
}

# The script under test. The seams are all it has, so this is the whole world
# it can see.
run() {
  (
    cd "$clone"
    PATH="$stub_dir:$PATH" \
    NPM_VIEW_FILE="$npm_file" \
    RELEASE_API_DIR="$api" \
    RELEASE_ASSETS_URL="file://$assets" \
    scripts/release-data.sh "$@" >"$scratch/output" 2>&1
  )
  status=$?
  cat "$scratch/output" > "$scratch/seen"
  return $status
}

refused() {
  local name="$1" needle="$2"
  shift 2
  if run "$@"; then
    miss "$name" "it did not refuse; it printed: $(tail -n 3 "$scratch/seen" | tr '\n' ' ')"
  elif grep -q "$needle" "$scratch/seen"; then
    pass "$name"
  else
    miss "$name" "it refused, but not with '$needle'"
  fi
}

base=$(git -C "$clone" rev-parse HEAD)

refuses_off_main() {
  setup
  git -C "$clone" checkout --quiet -b elsewhere
  refused refuses_off_main "a data package is built from main"
}

refuses_a_dirty_tree() {
  setup
  printf 'edited\n' >> "$clone/README.md"
  refused refuses_a_dirty_tree "the working tree has changes"
}

refuses_when_no_data_release() {
  setup
  data_releases v0.2.0
  refused refuses_when_no_data_release "no data-YYYY.MM release to name"
}

picks_the_newest_plain_data_release() {
  setup
  make_data_file "$scratch/oct.sqlite3" 2026.10 >/dev/null
  make_assets 2026.10 "$scratch/oct.sqlite3" >/dev/null
  # A rebuild is not a plain month and must not win.
  data_releases data-2026.09 data-2026.10.1 data-2026.10
  if run; then
    grep -q "Data version 2026.10" "$scratch/seen" &&
      grep -q "wenmar-open-data@3.202610.0" "$scratch/seen" &&
      pass picks_the_newest_plain_data_release ||
      miss picks_the_newest_plain_data_release "it did not pick 2026.10: $(grep -i 'data version\|package' "$scratch/seen" | tr '\n' ' ')"
  else
    miss picks_the_newest_plain_data_release "it failed: $(tail -n 3 "$scratch/seen" | tr '\n' ' ')"
  fi
}

refuses_a_bad_data_version() {
  setup
  refused refuses_a_bad_data_version "not a data version" --data-version 2026.13
}

refuses_a_malformed_data_version() {
  setup
  refused refuses_a_malformed_data_version "not a data version" --data-version "next month"
}

refuses_a_missing_checksum_file() {
  setup
  rm -f "$assets/data-2026.09/wenmar-open-2026.09.sqlite3.gz.sha256"
  refused refuses_a_missing_checksum_file "could not download"
}

refuses_a_file_that_fails_its_checksum() {
  setup
  # A different file under the released name.
  printf 'not the file you are looking for\n' | gzip -n > \
    "$assets/data-2026.09/wenmar-open-2026.09.sqlite3.gz"
  refused refuses_a_file_that_fails_its_checksum "does not match the checksum"
}

refuses_a_data_file_that_is_not_one() {
  setup
  printf 'this is not a database\n' > "$scratch/stranger.sqlite3"
  if run --data-file "$scratch/stranger.sqlite3"; then
    miss refuses_a_data_file_that_is_not_one "it accepted a file that is not a data file"
  else
    grep -qi "not a Wenmar Open data file" "$scratch/seen" &&
      pass refuses_a_data_file_that_is_not_one ||
      miss refuses_a_data_file_that_is_not_one "it refused for the wrong reason: $(tail -n 2 "$scratch/seen" | tr '\n' ' ')"
  fi
}

refuses_an_already_published_version() {
  setup
  printf '3.202609.0' > "$npm_file"
  refused refuses_an_already_published_version "already on npm"
}

# The 0.0.1 on npm is exactly this: a package whose tarball holds a README
# and nothing else, published under a version that looks real.
refuses_a_package_without_the_data() {
  setup
  # A package built by hand that claims a version and ships no data file.
  local fake="$scratch/placeholder"
  mkdir -p "$fake"
  cp "$clone/clients/data/README.md" "$fake/README.md"
  node -e '
    const [dir, version] = process.argv.slice(1);
    require("node:fs").writeFileSync(
      dir + "/package.json",
      JSON.stringify({ name: "wenmar-open-data", version, files: ["README.md"] }, null, 2) + "\n",
    );
  ' "$fake" "3.202609.0"
  # Stand in for prepare.mjs, so what is under test is the check and not the
  # packaging step that would have built a real one.
  cat > "$clone/clients/data/scripts/prepare.mjs" <<'EOF'
#!/usr/bin/env node
// A stand-in that produces a package with no data file in it, which is what
// 0.0.1 was.
import { mkdirSync, writeFileSync, cpSync } from "node:fs";
const [, , , target] = process.argv;
mkdirSync(target, { recursive: true });
cpSync(new URL("../README.md", import.meta.url), `${target}/README.md`);
writeFileSync(`${target}/package.json`, JSON.stringify({
  name: "wenmar-open-data", version: "3.202609.0", files: ["README.md"],
}, null, 2) + "\n");
EOF
  # The stand-in is part of the tree under test now, and a dirty tree is a
  # refusal of its own, so commit it the way a developer would.
  git -C "$clone" add --all
  git -C "$clone" -c user.name=test -c user.email=test@example.invalid \
    commit --quiet --allow-empty --no-verify -m "a prepare.mjs that builds a placeholder"
  if run; then
    miss refuses_a_package_without_the_data "it accepted a package holding no data file"
  else
    grep -q "Not publishing it" "$scratch/seen" &&
      pass refuses_a_package_without_the_data ||
      miss refuses_a_package_without_the_data "it refused for the wrong reason: $(tail -n 2 "$scratch/seen" | tr '\n' ' ')"
  fi
}

builds_and_verifies_the_package() {
  setup
  if ! run; then
    miss builds_and_verifies_the_package "it failed: $(tail -n 4 "$scratch/seen" | tr '\n' ' ')"
    return
  fi
  local problems=0
  grep -q "Package wenmar-open-data@3.202609.0" "$scratch/seen" || {
    miss builds_and_verifies_the_package "it did not name the version it built"
    problems=1
  }
  # The point of the whole thing: the tarball is installed and the data file
  # read back out of it.
  grep -q "the installed package holds data 2026.09" "$scratch/seen" || {
    miss builds_and_verifies_the_package "it did not verify the packed package"
    problems=1
  }
  grep -q "npm publish" "$scratch/seen" || {
    miss builds_and_verifies_the_package "it did not print the publish command"
    problems=1
  }
  # The command has to be one that can be run. This script runs from a
  # checkout, and npm can only make a provenance statement when the publish
  # comes from GitHub Actions or GitLab CI: anywhere else it refuses with
  # "Automatic provenance generation not supported for provider: null" and
  # publishes nothing. The workflow path is where provenance comes from.
  # Match the command itself, not the prose: the words appear in why there is
  # no such flag.
  if grep -E '^[[:space:]]*npm publish' "$scratch/seen" | grep -q -- '--provenance'; then
    miss builds_and_verifies_the_package \
      "it printed a --provenance command, which npm refuses outside a supported CI provider"
    problems=1
  fi
  grep -qi "workflow" "$scratch/seen" || {
    miss builds_and_verifies_the_package \
      "it did not say where provenance does come from"
    problems=1
  }
  # And it must not have published anything.
  grep -qi "^+ wenmar-open-data" "$scratch/seen" && {
    miss builds_and_verifies_the_package "it published"
    problems=1
  }
  [ "$problems" -eq 0 ] && pass builds_and_verifies_the_package
}

refuses_off_main
refuses_a_dirty_tree
refuses_when_no_data_release
picks_the_newest_plain_data_release
refuses_a_bad_data_version
refuses_a_malformed_data_version
refuses_a_missing_checksum_file
refuses_a_file_that_fails_its_checksum
refuses_a_data_file_that_is_not_one
refuses_an_already_published_version
refuses_a_package_without_the_data
builds_and_verifies_the_package

if [ "$failed" -ne 0 ]; then
  echo "release-data-test: something failed. Nothing was published." >&2
  exit 1
fi
echo "release-data-test: all cases passed. Nothing was published."
