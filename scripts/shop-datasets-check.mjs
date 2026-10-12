#!/usr/bin/env node
// Refuses a shop data release that has lost data the release before it had.
//
//   node scripts/shop-datasets-check.mjs <new manifest .json> [previous manifest .json]
//
// Both files are manifests written by scripts/shop-manifest.mjs. It fails,
// naming each dataset, when one the previous release held is missing from
// the new file, when a dataset of the new file has no rows, or when one has
// fewer than half the rows it had: a source that answered with part of a
// file looks like that. With no previous manifest, which is the first
// release, only the empty-dataset rule applies.
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
  const found = manifest?.datasets;
  if (typeof found !== "object" || found === null || Array.isArray(found)) {
    console.error(`shop-datasets-check: ${path} is not a manifest: it has no datasets object.`);
    process.exit(2);
  }
  return { version: String(manifest.data_version ?? "unknown"), found };
}

const now = datasets(newer);
const before = previous === undefined ? { version: "none", found: {} } : datasets(previous);
const rows = (entry) => (Number.isFinite(entry?.rows) ? entry.rows : 0);

const problems = [];
for (const id of Object.keys(before.found).sort()) {
  if (!(id in now.found)) {
    problems.push(`${id} was in ${before.version} and is missing from ${now.version}`);
  }
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

if (problems.length > 0) {
  for (const problem of problems) console.error(`shop-datasets-check: ${problem}`);
  console.error("shop-datasets-check: not released. A dataset is only ever dropped on purpose; see docs/releasing.md.");
  process.exit(1);
}
console.log(`shop-datasets-check: ${now.version} holds everything ${before.version} held.`);
