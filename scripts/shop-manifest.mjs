#!/usr/bin/env node
// Prints the manifest of a shop data release: what the file is, which data
// file it pairs with, and which datasets it holds.
//
//   node scripts/shop-manifest.mjs <offline shop file .sqlite3> <that file, gzipped> \
//     [<full shop file .sqlite3> <that file, gzipped>]
//
// A release carries two shop files. The offline file is the one people
// download: `wenmar-open data pull --shop` and the npm package take it, and
// the first seven fields of the manifest describe it, as they always have.
// The full file also holds the datasets only the hosted service carries
// (bulletins); the server's image takes it, and the eighth field, `full`,
// gives its size, its checksum and its datasets. An older command-line tool
// reads the fields it knows and never looks at `full`.
//
// `wenmar-open data pull --shop` reads catalog_data_version to find the
// release that pairs with the data file it has, scripts/shop-pull.mjs reads
// `full` to fetch and check the server's file, and the next release reads
// both dataset lists so that a dataset cannot go missing unnoticed
// (scripts/shop-datasets-check.mjs).
//
// Everything comes from the files' own meta tables, never from their names.
// Needs node:sqlite: Node 22.16 or later.
import { createHash } from "node:crypto";
import { readFileSync, statSync } from "node:fs";

import { HOSTED_ONLY, hostedOnlyIn } from "../clients/data-shop/lib/hosted-only.mjs";

const USAGE =
  "usage: node scripts/shop-manifest.mjs <offline shop file .sqlite3> <that file, gzipped> [<full shop file .sqlite3> <that file, gzipped>]";
const args = process.argv.slice(2);
if (args.length !== 2 && args.length !== 4) {
  console.error(USAGE);
  process.exit(2);
}
const [shop, packed, full, fullPacked] = args;

function fail(message) {
  console.error(`shop-manifest: ${message}`);
  process.exit(1);
}

let DatabaseSync;
try {
  ({ DatabaseSync } = await import("node:sqlite"));
} catch {
  fail("this needs node:sqlite: Node 22.16 or later.");
}

/** The meta table of a shop file and its datasets, or a refusal that names the file. */
function read(file) {
  let meta;
  try {
    const database = new DatabaseSync(file, { readOnly: true });
    try {
      meta = Object.fromEntries(
        database
          .prepare("SELECT key, value FROM meta")
          .all()
          .map((row) => [row.key, row.value]),
      );
    } finally {
      database.close();
    }
  } catch (error) {
    fail(`${file} is not a Wenmar Open shop file: ${error.message}`);
  }
  for (const key of ["schema_version", "data_version", "catalog_data_version", "datasets"]) {
    if (typeof meta[key] !== "string" || meta[key] === "") {
      fail(`${file} is not a Wenmar Open shop file: its meta table has no ${key}.`);
    }
  }
  if (!/^[0-9]{4}\.[0-9]{2}\.[0-9]{2}(\.[1-9][0-9]{0,3})?$/.test(meta.data_version)) {
    fail(`${file} has data version ${meta.data_version}, which is not like 2026.10.12 or 2026.10.12.1.`);
  }
  let datasets;
  try {
    datasets = JSON.parse(meta.datasets);
  } catch (error) {
    fail(`${file} has a datasets value that is not JSON: ${error.message}`);
  }
  if (typeof datasets !== "object" || datasets === null || Array.isArray(datasets)) {
    fail(`${file} has a datasets value that is not an object.`);
  }
  return { meta, datasets };
}

/** The SHA-256 of a file, in hexadecimal. */
function sha256Of(file) {
  try {
    return createHash("sha256").update(readFileSync(file)).digest("hex");
  } catch (error) {
    fail(`${file} could not be read: ${error.message}`);
  }
}

const { meta, datasets } = read(shop);
const hosted = hostedOnlyIn(datasets);
if (hosted.length > 0) {
  fail(
    `${shop} holds ${hosted.join(", ")}, which only the hosted service carries. The first file is the offline one: the file \`open-data build-shop --offline-out\` wrote.`,
  );
}

const manifest = {
  data_version: meta.data_version,
  catalog_data_version: meta.catalog_data_version,
  schema_version: meta.schema_version,
  built_at: meta.built_at ?? "",
  bytes: statSync(shop).size,
  sha256: sha256Of(packed),
  datasets,
};

if (full !== undefined) {
  const whole = read(full);
  // The two are one build: the offline file is a copy of the full one
  // without the hosted-only datasets, and nothing else differs.
  for (const key of ["data_version", "catalog_data_version", "schema_version"]) {
    if (whole.meta[key] !== meta[key]) {
      fail(`${full} has ${key} ${whole.meta[key]} and ${shop} has ${meta[key]}. The two files of a release come from one build.`);
    }
  }
  for (const id of Object.keys(datasets)) {
    if (whole.datasets[id]?.rows !== datasets[id]?.rows) {
      fail(`${id} has ${datasets[id]?.rows} rows in ${shop} and ${whole.datasets[id]?.rows ?? "none"} in ${full}. The two files of a release come from one build.`);
    }
  }
  const extra = Object.keys(whole.datasets).filter((id) => !(id in datasets));
  const misplaced = extra.filter((id) => !HOSTED_ONLY.includes(id));
  if (misplaced.length > 0) {
    fail(`${full} holds ${misplaced.join(", ")}, which ${shop} lacks and which the offline file should carry.`);
  }
  manifest.full = {
    bytes: statSync(full).size,
    sha256: sha256Of(fullPacked),
    datasets: whole.datasets,
  };
}
process.stdout.write(`${JSON.stringify(manifest, null, 2)}\n`);
