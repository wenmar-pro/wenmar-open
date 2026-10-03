#!/usr/bin/env node
// Writes the data file as SQL files for Cloudflare D1.
//
//   npx wenmar-open-d1 --out ./d1-import [--file data.sqlite3] [--part-mb 50] [--dry-run]
//
// It reads the file and writes text. It does not call Cloudflare, and it
// needs Node 22.16 or later for node:sqlite. Then, for each file it wrote:
//
//   npx wrangler d1 execute <database> --remote --yes --file=./d1-import/part-001.sql
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parseArgs } from "node:util";

import { D1_STATEMENT_BYTES, writeParts } from "../lib/d1.mjs";

const { values } = parseArgs({
  options: {
    out: { type: "string" },
    file: { type: "string" },
    "part-mb": { type: "string", default: "50" },
    "dry-run": { type: "boolean", default: false },
    help: { type: "boolean", short: "h", default: false },
  },
});
const usage = "usage: wenmar-open-d1 --out <directory> [--file <data file>] [--part-mb 50] [--dry-run]";
if (values.help) {
  console.log(usage);
  process.exit(0);
}
if (values.out === undefined && !values["dry-run"]) {
  console.error(usage);
  process.exit(2);
}
const partBytes = Number(values["part-mb"]) * 1024 * 1024;
if (!Number.isFinite(partBytes) || partBytes < 1024 * 1024) {
  console.error("--part-mb must be a number of megabytes, 1 or more.");
  process.exit(2);
}

let DatabaseSync;
try {
  ({ DatabaseSync } = await import("node:sqlite"));
} catch {
  console.error("This needs node:sqlite: Node 22.16 or later.");
  process.exit(1);
}
const file = values.file ?? (await import("../index.js")).path;
const database = new DatabaseSync(file, { readOnly: true });

const dry = values["dry-run"];
if (!dry) mkdirSync(values.out, { recursive: true });
const names = [];
let summary;
try {
  summary = writeParts(database, partBytes, (name, text) => {
    names.push(name);
    if (!dry) writeFileSync(join(values.out, name), text);
  });
} catch (error) {
  console.error(`${file} cannot be written for D1: ${error.message}`);
  process.exit(1);
} finally {
  database.close();
}

const megabytes = (summary.bytes / 1024 / 1024).toFixed(1);
console.log(
  `${summary.statements} statements, ${megabytes} MB, in ${summary.parts} files${dry ? " (dry run: nothing written)" : ` in ${values.out}`}.`,
);
console.log(`The longest statement is ${summary.longestStatement} bytes; D1 allows ${D1_STATEMENT_BYTES.toLocaleString("en-US")}.`);
if (!dry) {
  console.log("\nImport them in order into a new, empty database:\n");
  console.log("  npx wrangler d1 create wenmar-open-YYYY-MM");
  console.log(`  for part in ${join(values.out, "part-*.sql")}; do`);
  console.log('    npx wrangler d1 execute wenmar-open-YYYY-MM --remote --yes --file="$part" || break');
  console.log("  done");
}
