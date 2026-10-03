// Whether scripts/prepare.mjs may empty an output directory.
//
// prepare.mjs replaces its output directory. If that directory is, or holds,
// the data file or this package, it would delete the only copy of a file
// that takes a full build to make, or the package's own sources. This only
// decides; it reads paths and deletes nothing.
import { existsSync, realpathSync } from "node:fs";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";

/** The real path of a place that may not exist yet: its nearest existing ancestor is resolved through links, and the rest is added back. */
function realish(path) {
  const full = resolve(path);
  if (existsSync(full)) return realpathSync(full);
  const parent = dirname(full);
  return parent === full ? full : join(realish(parent), full.slice(parent.length + 1));
}

/** Whether `inner` is `outer` or inside it. */
function holds(outer, inner) {
  const way = relative(outer, inner);
  return way === "" || (!way.startsWith("..") && !isAbsolute(way));
}

/**
 * Returns a one-line refusal when emptying `out` would delete `source` (the
 * data file), its directory or `home` (this package's directory), and
 * nothing when it is safe.
 */
export function refuseOutput(source, out, home) {
  const target = realish(out);
  const file = realish(source);
  if (target === file || holds(target, dirname(file)) || holds(target, realish(home))) {
    return `${out} would delete the data file or the package; give an output directory of its own.`;
  }
  return undefined;
}
