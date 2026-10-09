#!/usr/bin/env bash
# Reports whether a tag exists on origin. It prints nothing.
#
#   scripts/tag-exists.sh v0.1.1
#
# Exit 0 means the tag is on origin, 1 means it is not, and 2 means the
# question could not be answered. It needs no credentials because the
# repository is public.
#
# scripts/release-bump-test.sh tests this script; run it after changing it.
set -euo pipefail
cd "$(dirname "$0")/.."

[ $# -eq 1 ] || { echo "tag-exists: pass one tag name, such as v0.1.1." >&2; exit 2; }

# git ls-remote --exit-code exits 0 with the tag, 2 without it, and 128 when
# origin cannot be reached. Only 2 is an answer; 128 must not be read as "no
# such tag", or a network failure would publish over an existing tag.
status=0
git ls-remote --exit-code --tags origin "refs/tags/$1" >/dev/null 2>&1 || status=$?
case "$status" in
  0) exit 0 ;;
  2) exit 1 ;;
  *) exit 2 ;;
esac
