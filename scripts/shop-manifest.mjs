#!/usr/bin/env node
// Prints the manifest of a shop data release: what the file is, which data
// file it pairs with, and which datasets it holds.
//
//   node scripts/shop-manifest.mjs <shop file .sqlite3> <that file, gzipped>
//
// The manifest is the third file of a shop-YYYY.MM.DD release, beside the
// gzipped shop file and its checksum. `wenmar-open data pull --shop` reads
// catalog_data_version from it to find the release that pairs with the data
// file it has, and the next release reads datasets from it so that a dataset
// cannot go missing unnoticed (scripts/shop-datasets-check.mjs).
//
// Everything comes from the file's own meta table, never from its name.
// Needs node:sqlite: Node 22.16 or later.
import { createHash } from "node:crypto";
import { readFileSync, statSync } from "node:fs";

const [shop, packed] = process.argv.slice(2);
if (shop === undefined || packed === undefined) {
  console.error("usage: node scripts/shop-manifest.mjs <shop file .sqlite3> <that file, gzipped>");
  process.exit(2);
}

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

let meta;
try {
  const database = new DatabaseSync(shop, { readOnly: true });
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
  fail(`${shop} is not a Wenmar Open shop file: ${error.message}`);
}
for (const key of ["schema_version", "data_version", "catalog_data_version", "datasets"]) {
  if (typeof meta[key] !== "string" || meta[key] === "") {
    fail(`${shop} is not a Wenmar Open shop file: its meta table has no ${key}.`);
  }
}
if (!/^[0-9]{4}\.[0-9]{2}\.[0-9]{2}(\.[1-9][0-9]{0,3})?$/.test(meta.data_version)) {
  fail(`${shop} has data version ${meta.data_version}, which is not like 2026.10.12 or 2026.10.12.1.`);
}
let datasets;
try {
  datasets = JSON.parse(meta.datasets);
} catch (error) {
  fail(`${shop} has a datasets value that is not JSON: ${error.message}`);
}
if (typeof datasets !== "object" || datasets === null || Array.isArray(datasets)) {
  fail(`${shop} has a datasets value that is not an object.`);
}
let sha256;
try {
  sha256 = createHash("sha256").update(readFileSync(packed)).digest("hex");
} catch (error) {
  fail(`${packed} could not be read: ${error.message}`);
}

const manifest = {
  data_version: meta.data_version,
  catalog_data_version: meta.catalog_data_version,
  schema_version: meta.schema_version,
  built_at: meta.built_at ?? "",
  bytes: statSync(shop).size,
  sha256,
  datasets,
};
process.stdout.write(`${JSON.stringify(manifest, null, 2)}\n`);
