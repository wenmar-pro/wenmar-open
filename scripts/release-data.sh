#!/usr/bin/env bash
# Publishes a released data file to npm as `wenmar-open-data`, by hand.
#
#   scripts/release-data.sh [--data-version YYYY.MM] [--data-file PATH]
#
# It is the npm half of .github/workflows/data-release.yml, runnable from a
# checkout, so the data package does not have to go out through the Actions
# interface — which is what you want the first time, and whenever you want to
# see the package before it is published.
#
# It publishes nothing. It builds the package, proves that the package holds a
# real data file, and prints the `npm publish` to run. An npm version cannot be
# reused, so that command is yours to read and run.
#
# It needs no stored credential: the release is public, so GitHub's API and
# its assets are read anonymously. Set GH_TOKEN to raise the API rate limit,
# never as a requirement. `npm publish` will want you to be logged in to npm;
# see docs/releasing.md for the trusted-publishing setup.
#
# In order, refusing rather than half-doing anything:
#   1. on main, working tree clean — the package embeds this checkout's
#      README, licence and notices, so the tree is part of what is published
#   2. a data version, from the newest plain data-YYYY.MM release, or
#      --data-version, and a file to build it from
#   3. the release's file, downloaded and checked against its SHA256SUMS
#   4. clients/data/scripts/prepare.mjs, which names the package from the
#      data file's own meta table
#   5. the packed package, installed into a scratch directory, holding a data
#      file of the schema and the month that were asked for
#   6. that version already on npm
#   7. stop, and print the two commands to run
#
# Environment:
#   RELEASE_API_DIR      answer GitHub's API from files in this directory
#                        instead of the network. For scripts/release-data-test.sh.
#   RELEASE_ASSETS_URL   where the release's files are downloaded from. The
#                        test points it at a directory. Defaults to GitHub.
#   RELEASE_DATA_FILE    the data file to build the package from, already
#                        unpacked. Skips the download; for when you built it
#                        with `mise run data`.
#
# scripts/release-data-test.sh tests this script; run it after changing it.
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD

api="https://api.github.com/repos/wenmar-pro/wenmar-open"
api_dir="${RELEASE_API_DIR:-}"
assets="${RELEASE_ASSETS_URL:-https://github.com/wenmar-pro/wenmar-open/releases/download}"
data_version=""
data_file="${RELEASE_DATA_FILE:-}"

fail() {
  echo "release-data: $*" >&2
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
    --data-file)
      [ $# -ge 2 ] || fail "--data-file needs a path to an unpacked data file."
      data_file="$2"
      shift 2
    ;;
    --data-file=*)
      data_file="${1#--data-file=}"
      shift
    ;;
    -h|--help)
      sed -n '2,33p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
    ;;
    *) fail "unknown argument $1. Pass --data-version YYYY.MM, --data-file PATH, or nothing." ;;
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

# 1. The tree this builds a package from. The package carries this checkout's
# README, licence, notices and the d1 script, so an edited tree would publish
# something nobody reviewed.
branch=$(git branch --show-current)
[ "$branch" = "main" ] ||
  fail "a data package is built from main, and this is on ${branch:-a detached HEAD}."
[ -z "$(git status --porcelain)" ] ||
  fail "the working tree has changes. Commit or stash them, then run this again."

# 2. The data version to build. A rebuild such as data-2026.10.1 is not a plain
# month and no release builds it by hand, so the filter is anchored at both ends.
if [ -z "$data_version" ]; then
  tags=$(get "releases?per_page=30" | jq -r '.[].tag_name')
  data_version=$(printf '%s\n' "$tags" |
    sed -n 's/^data-\([0-9]\{4\}\.[0-9]\{2\}\)$/\1/p' | sort -r | head -n 1)
  [ -n "$data_version" ] ||
    fail "no data-YYYY.MM release to name. Run data-release.yml first, or pass --data-version YYYY.MM."
  echo "Data version $data_version, from the newest data release."
else
  # The month is checked as a month, not as two digits: 2026.13 is not a
  # version of anything. This is the same shape clients/data/lib/version.mjs
  # uses to write a package version from a data version.
  echo "$data_version" | grep -Eq '^[0-9]{4}\.(0[1-9]|1[0-2])(\.[0-9]+)?$' ||
    fail "--data-version is $data_version, which is not a data version like 2026.10 or 2026.10.1."
  echo "Data version $data_version, as given."
fi
name="wenmar-open-$data_version.sqlite3"

# 3. The file. The release's own checksum is the only thing that says the
# download is the file that was built and released, so it is checked before the
# package is assembled, not after.
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
if [ -z "$data_file" ]; then
  echo "Downloading $name from the data release."
  curl -fsSL -o "$scratch/$name.gz" "$assets/data-$data_version/$name.gz" ||
    fail "could not download $name.gz from the data-$data_version release."
  curl -fsSL -o "$scratch/$name.gz.sha256" "$assets/data-$data_version/$name.gz.sha256" ||
    fail "could not download $name.gz.sha256 from the data-$data_version release."
  (cd "$scratch" && sha256sum --check --status "$name.gz.sha256") ||
    fail "$name.gz does not match the checksum the data release published. Not publishing it."
  echo "Checksum matches."
  gunzip "$scratch/$name.gz"
  data_file="$scratch/$name"
else
  [ -f "$data_file" ] || fail "--data-file is $data_file, which is not a file."
  echo "Data file $data_file, as given."
fi

# 4. The package. prepare.mjs reads the schema and the month out of the file's
# own meta table and names the package from them, so the version is never
# written by hand anywhere in this script. It lands in clients/data/build, which
# is gitignored and is where the data release workflow assembles it too, so the
# directory you publish from survives this script.
package_dir="$root/clients/data/build"
node clients/data/scripts/prepare.mjs "$data_file" "$package_dir"
version=$(node -p "require('$package_dir/package.json').version")
echo "Package wenmar-open-data@$version."

# 5. The proof. 0.0.1 was a placeholder whose tarball held a README and
# nothing else, so the check is on what the packed tarball actually carries:
# installed into a scratch directory and asked for its meta table. A package
# without the data file in it fails here rather than after it is published.
tarball=$(cd "$package_dir" && npm pack --silent --pack-destination "$scratch")
tarball="$scratch/$(basename "$tarball")"
scratch_home="$scratch/home"
mkdir -p "$scratch_home"
(cd "$scratch_home" && npm init -y >/dev/null &&
  npm install --no-audit --no-fund --silent "$tarball" >/dev/null) ||
  fail "the packed package could not be installed. Not publishing it."
node -e '
  const { DatabaseSync } = require("node:sqlite");
  const { path, dataVersion, schemaVersion } = require(process.argv[1] + "/index.js");
  const database = new DatabaseSync(path, { readOnly: true });
  const meta = Object.fromEntries(
    database.prepare("SELECT key, value FROM meta").all().map((row) => [row.key, row.value]),
  );
  const models = database.prepare("SELECT COUNT(*) AS n FROM catalog_model").get().n;
  database.close();
  const wanted = process.argv[2];
  const problems = [];
  if (meta.data_version !== wanted) {
    problems.push(`the file is data ${meta.data_version}, not ${wanted}`);
  }
  if (String(schemaVersion) !== String(meta.schema_version)) {
    problems.push(`the package says schema ${schemaVersion}, the file says ${meta.schema_version}`);
  }
  if (models < 1) {
    problems.push(`the catalog has no models (${models})`);
  }
  if (problems.length > 0) {
    console.error("release-data: the packed package does not hold the data asked for:");
    for (const problem of problems) console.error("  " + problem);
    process.exit(1);
  }
  console.log(`  the installed package holds data ${dataVersion}, schema ${schemaVersion}, ${models} models`);
' "$scratch_home/node_modules/wenmar-open-data" "$data_version" ||
  fail "the packed package did not hold the data file asked for. Not publishing it."

# 6. A version on npm cannot be replaced, so one already there is a stop, not a
# job to redo. A question this cannot answer is also a stop: reading a network
# failure as "not published" would publish over a version that is there.
published=$(npm view "wenmar-open-data@$version" version 2>/dev/null) || published=""
[ -z "$published" ] ||
  fail "wenmar-open-data@$version is already on npm. Nothing to do."

# 7. Stop. Everything above is reversible; npm publish is not.
cat <<EOF

wenmar-open-data@$version is built, verified and unpacked in $package_dir.

It holds a real data file of $data_version, read back out of the packed
tarball after installing it. Nothing has been published.

To publish it, and then the code release that depends on it:

  npm publish --access public "$package_dir"
  bin/release

The data package has to be on npm first: wenmar-open names it as an optional
peer of a schema version, and the 0.0.1 placeholder on npm does not satisfy
that range.

There is no --provenance on that command, and there cannot be: npm only makes
a provenance statement for a publish coming from GitHub Actions or GitLab CI,
and refuses anywhere else with "Automatic provenance generation not supported
for provider: null" before publishing a byte. A package published from here
carries no provenance statement. If you want one, publish through the
"npm" job of .github/workflows/data-release.yml instead, which is a trusted
publisher and needs no npm login.
EOF
