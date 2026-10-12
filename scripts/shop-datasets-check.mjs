#!/usr/bin/env node
// Refuses a shop data release that has lost data the release before it had.
//
//   node scripts/shop-datasets-check.mjs <new manifest .json> [previous manifest .json]
//
// Both files are manifests written by scripts/shop-manifest.mjs. A release
// carries two shop files, and each is compared with its own kind:
//
// - the offline file (the manifest's `datasets`) with the previous offline
//   file;
// - the datasets only the full file holds (those in `full.datasets` and not
//   in `datasets`) with the previous full file.
//
// It fails, naming each dataset, when one the previous release held is
// missing from the new file of the same kind, when a dataset has no rows,
// or when one has fewer than half the rows it had: a source that answered
// with part of a file looks like that. With no previous manifest, which is
// the first release, only the empty-dataset rule applies.
//
// A previous manifest with no `full` is from a release that had one file,
// which stood for both. So the first release with a full file passes: what
// its full file adds is new, and a dataset that the one file held and that
// is now in the full file alone is compared there and is not lost.
import { readFileSync } from "node:fs";

const [newer, previous] = process.argv.slice(2);
if (newer === undefined) {
  console.error("usage: node scripts/shop-datasets-check.mjs <new manifest .json> [previous manifest .json]");
  process.exit(2);
}

function datasets(path) {
  let manifest;
  try {
    manifest = JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    console.error(`shop-datasets-check: ${path} is not a manifest: ${error.message}`);
    process.exit(2);
  }
  const isObject = (value) => typeof value === "object" && value !== null && !Array.isArray(value);
  const found = manifest?.datasets;
  if (!isObject(found)) {
    console.error(`shop-datasets-check: ${path} is not a manifest: it has no datasets object.`);
    process.exit(2);
  }
  const full = manifest.full === undefined ? undefined : manifest.full?.datasets;
  if (manifest.full !== undefined && !isObject(full)) {
    console.error(`shop-datasets-check: ${path} is not a manifest: its full has no datasets object.`);
    process.exit(2);
  }
  return { version: String(manifest.data_version ?? "unknown"), found, full };
}

const now = datasets(newer);
const before = previous === undefined ? { version: "none", found: {}, full: undefined } : datasets(previous);
const rows = (entry) => (Number.isFinite(entry?.rows) ? entry.rows : 0);

// A release with one file: that file is the full file too.
const nowFull = now.full ?? now.found;
const beforeFull = before.full ?? before.found;
/** The datasets of the new release that only its full file holds. */
const hostedNow = Object.keys(nowFull)
  .filter((id) => !(id in now.found))
  .sort();

const problems = [];

// The offline file, with the previous offline file.
for (const id of Object.keys(before.found).sort()) {
  if (id in now.found) continue;
  // The previous release had one file, and this dataset is now in the full
  // file alone: it is compared below.
  if (before.full === undefined && hostedNow.includes(id)) continue;
  problems.push(`${id} was in ${before.version} and is missing from ${now.version}`);
}
for (const id of Object.keys(now.found).sort()) {
  const have = rows(now.found[id]);
  const had = id in before.found ? rows(before.found[id]) : undefined;
  if (have === 0) {
    problems.push(`${id} has no rows in ${now.version}`);
  } else if (had !== undefined && have * 2 < had) {
    problems.push(`${id} has ${have} rows in ${now.version} and had ${had} in ${before.version}: fewer than half`);
  }
  const was = had === undefined ? "new" : `was ${had}`;
  console.log(`${id}: ${have} rows, as of ${now.found[id]?.as_of ?? "unknown"} (${was})`);
}

// What only the full file holds, with the previous full file.
for (const id of Object.keys(before.full ?? {}).sort()) {
  // One the offline file held too was compared above.
  if (id in before.found) continue;
  if (!(id in nowFull)) {
    problems.push(`${id} was in the full file of ${before.version} and is missing from the full file of ${now.version}`);
  }
}
for (const id of hostedNow) {
  const have = rows(nowFull[id]);
  const had = id in beforeFull ? rows(beforeFull[id]) : undefined;
  if (have === 0) {
    problems.push(`${id} has no rows in the full file of ${now.version}`);
  } else if (had !== undefined && have * 2 < had) {
    problems.push(
      `${id} has ${have} rows in the full file of ${now.version} and had ${had} in ${before.version}: fewer than half`,
    );
  }
  const was = had === undefined ? "new" : `was ${had}`;
  console.log(`${id}: ${have} rows, as of ${nowFull[id]?.as_of ?? "unknown"} (${was}), full file only`);
}

if (problems.length > 0) {
  for (const problem of problems) console.error(`shop-datasets-check: ${problem}`);
  console.error("shop-datasets-check: not released. A dataset is only ever dropped on purpose; see docs/releasing.md.");
  process.exit(1);
}
console.log(`shop-datasets-check: ${now.version} holds everything ${before.version} held.`);
