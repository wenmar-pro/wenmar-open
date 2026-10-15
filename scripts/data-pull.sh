#!/usr/bin/env bash
# Fetches the data file the server image will be built around: the asset of
# the data-<DATA_VERSION> release, where DATA_VERSION is the line in
# config/deploy.yml unless an argument gives one.
#
#   scripts/data-pull.sh [YYYY.MM]
#
# The download is checked against the release's .sha256 file, unpacked
# beside its final name and moved into place only when whole, so an
# interrupted run never leaves a file the image build would take. Nothing is
# committed or deployed. It needs no credential: the releases are public.
set -euo pipefail
cd "$(dirname "$0")/.."

fail() {
  echo "data-pull: $1" >&2
  exit 1
}

version=${1:-$(sed -n 's/^ *DATA_VERSION: "\(.*\)"$/\1/p' config/deploy.yml)}
[[ $version =~ ^[0-9]{4}\.[0-9]{2}(\.[0-9]+)?$ ]] || fail "'$version' is not a data version like 2026.09."

[ -L data/build ] && fail "data/build is a symlink. Remove it and run this again: rm data/build"
mkdir -p data/build
out="data/build/wenmar-open-$version.sqlite3"
if [ -f "$out" ] && [ ! -L "$out" ]; then
  echo "$out is already here."
  exit 0
fi

base="https://github.com/wenmar-pro/wenmar-open/releases/download/data-$version"
name="wenmar-open-$version.sqlite3.gz"
work=$(mktemp -d -p data/build .data-pull.XXXXXX)
trap 'rm -rf "$work"' EXIT

echo "Downloading data-$version ..."
curl -fsSL --retry 3 -o "$work/$name" "$base/$name" || fail "$base/$name could not be downloaded. Is data-$version released?"
curl -fsSL --retry 3 -o "$work/$name.sha256" "$base/$name.sha256" || fail "$base/$name.sha256 could not be downloaded."
(cd "$work" && sha256sum --check --quiet "$name.sha256") || fail "the download does not match its checksum. Nothing was changed."
gunzip -c "$work/$name" >"$work/file.sqlite3" || fail "the download could not be unpacked. Nothing was changed."
mv "$work/file.sqlite3" "$out"
echo "data-pull: $out: $(stat -c %s "$out") bytes"
