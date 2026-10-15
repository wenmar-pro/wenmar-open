#!/usr/bin/env bash
# Checks, before the image is built, what the build needs and git does not
# hold: the data file and the shop file config/deploy.yml names. Kamal runs it
# as the pre-build hook (.kamal/hooks/pre-build), so a missing file stops
# `kamal deploy` here with how to get it, rather than as a COPY error in the
# middle of the Docker build. `kamal deploy --skip-hooks` goes past it.
#
#   scripts/deploy-check.sh
#
# The image build still opens both files (`open-server check`), which is
# what checks the schema and that the two were built as a pair.
set -euo pipefail
cd "$(dirname "$0")/.."

problems=0
problem() {
  echo "deploy-check: $1" >&2
  problems=$((problems + 1))
}

line() { sed -n "s/^ *$1: \"\(.*\)\"$/\1/p" config/deploy.yml; }
data_version=$(line DATA_VERSION)
shop_version=$(line SHOP_VERSION)
[ -n "$data_version" ] || problem "config/deploy.yml has no DATA_VERSION line."
[ -n "$shop_version" ] || problem "config/deploy.yml has no SHOP_VERSION line."

# Kamal builds from the working directory, so anything uncommitted is built
# into the image.
if [ -n "$(git status --porcelain)" ]; then
  problem "the working tree is not clean; commit or remove these first:"
  git status --short >&2
fi

# The data files are never committed. A tracked path under data/build is
# swapped in for the real directory by the next checkout, and git deletes
# the ignored files in it to do so.
if [ -n "$(git ls-files data/build)" ]; then
  problem "git tracks $(git ls-files data/build | head -1) under data/build. Untrack it: git rm -r --cached data/build"
fi

# Docker does not follow a symlink out of the build context.
if [ -L data/build ]; then
  problem "data/build is a symlink to $(readlink data/build), and the build cannot read through it. Replace it with a directory: rm data/build && mkdir -p data/build/shop, then bin/data-pull and bin/shop-pull."
fi

# A regular file that begins as SQLite files do.
check_file() {
  local path=$1 fix=$2
  if [ -L "$path" ]; then
    problem "$path is a symlink, which the build cannot copy. $fix"
  elif [ ! -f "$path" ]; then
    problem "$path is missing. $fix"
  elif [ "$(head -c 15 "$path")" != "SQLite format 3" ]; then
    problem "$path is not a SQLite file. $fix"
  fi
}
if [ -n "$data_version" ]; then
  check_file "data/build/wenmar-open-$data_version.sqlite3" "Fetch it: bin/data-pull"
fi
if [ -n "$shop_version" ]; then
  check_file "data/build/shop/wenmar-open-shop-$shop_version.sqlite3" \
    "Fetch the newest that pairs with data $data_version: bin/shop-pull (or build one: mise run shop-data), then commit config/deploy.yml."
fi

if [ "$problems" -gt 0 ]; then
  echo "deploy-check: $problems problem(s); nothing was built. See docs/deploy.md." >&2
  exit 1
fi
echo "deploy-check: data $data_version and shop $shop_version are in place."
