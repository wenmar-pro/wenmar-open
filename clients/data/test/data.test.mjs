// The scripts of wenmar-open-data, against the small data file the npm
// client's tests use. Needs node:sqlite: Node 22.16 or later. On an older
// Node every test here is skipped.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

import { STATEMENT_BYTES, literal, writeParts } from "../lib/d1.mjs";
import { dataVersion, npmVersion } from "../lib/version.mjs";

const sqlite = await import("node:sqlite").then(
  (module) => (typeof module.StatementSync.prototype.setReturnArrays === "function" ? module : undefined),
  () => undefined,
);
const skip = sqlite === undefined ? "needs node:sqlite of Node 22.16 or later" : false;

const home = fileURLToPath(new URL("../", import.meta.url));
const fixtureSql = readFileSync(new URL("../../js/test/fixtures/offline.sql", import.meta.url), "utf8");
// The real path: on macOS the temporary directory is reached through a link.
const directory = realpathSync(mkdtempSync(join(tmpdir(), "wenmar-open-data-")));
after(() => rmSync(directory, { recursive: true, force: true }));

function dataFile(name, sql = fixtureSql) {
  const path = join(directory, name);
  const database = new sqlite.DatabaseSync(path);
  database.exec(sql);
  database.close();
  return path;
}

/** Every table's rows and every index, to compare two databases. */
function contents(database) {
  const objects = database
    .prepare("SELECT type, name, sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY type, name")
    .all();
  const rows = {};
  for (const { type, name } of objects) {
    if (type !== "table") continue;
    const select = database.prepare(`SELECT * FROM "${name}" ORDER BY rowid`);
    select.setReturnArrays(true);
    rows[name] = select.all();
  }
  return { objects: objects.map(({ type, name, sql }) => ({ type, name, sql })), rows };
}

test("the npm version puts the schema first, then the month, then the rebuild", () => {
  assert.equal(npmVersion("3", "2026.09"), "3.202609.0");
  assert.equal(npmVersion("3", "2026.09.1"), "3.202609.1");
  assert.equal(npmVersion("4", "2027.01"), "4.202701.0");
  assert.equal(npmVersion(3, "2026.12.12"), "3.202612.12");
  for (const [schema, data] of [["3", "2026.9"], ["3", "2026.13"], ["3", "26.09"], ["03", "2026.09"], ["", "2026.09"], ["3", "2026.09.0"], ["3", "2026.09.01"], ["3", ""]]) {
    assert.throws(() => npmVersion(schema, data), `${schema} ${data}`);
  }
  assert.equal(dataVersion("3.202609.0"), "2026.09");
  assert.equal(dataVersion("3.202609.1"), "2026.09.1");
  assert.throws(() => dataVersion("2026.9.0"));
});

test("npm versions sort as the data versions do, and a new schema sorts after every month", () => {
  const order = ["3.202609.0", "3.202609.1", "3.202610.0", "3.202701.0", "4.202612.0"];
  const numbers = (version) => version.split(".").map(Number);
  const sorted = [...order].sort((left, right) => {
    const [a, b] = [numbers(left), numbers(right)];
    return a[0] - b[0] || a[1] - b[1] || a[2] - b[2];
  });
  assert.deepEqual(sorted, order);
});

test("prepare builds the package from a data file, with its version from the file", { skip }, () => {
  const out = join(directory, "package");
  const source = dataFile("prepare.sqlite3");
  const said = execFileSync("node", ["scripts/prepare.mjs", source, out], { cwd: home, encoding: "utf8" });
  assert.match(said, /^wenmar-open-data@3\.202609\.0: data 2026\.09, schema 3, \d+ bytes, in /);

  const manifest = JSON.parse(readFileSync(join(out, "package.json"), "utf8"));
  assert.equal(manifest.version, "3.202609.0");
  assert.equal(manifest.scripts, undefined);
  for (const field of ["dependencies", "peerDependencies", "optionalDependencies"]) {
    assert.equal(manifest[field], undefined, field);
  }

  const [packed] = JSON.parse(execFileSync("npm", ["pack", "--dry-run", "--json", "--ignore-scripts"], { cwd: out, encoding: "utf8" }));
  assert.deepEqual(packed.files.map((file) => file.path).sort(), [
    "LICENSE",
    "NOTICE.md",
    "README.md",
    "bin/d1.mjs",
    "index.d.ts",
    "index.js",
    "lib/d1.mjs",
    "lib/version.mjs",
    "package.json",
    "wenmar-open.sqlite3",
  ]);
});

test("the prepared package says where its file is and what it is", { skip }, async () => {
  const out = join(directory, "package-2");
  execFileSync("node", ["scripts/prepare.mjs", dataFile("prepare-2.sqlite3"), out], { cwd: home });
  const data = await import(pathToFileURL(join(out, "index.js")).href);
  assert.equal(data.path, join(out, "wenmar-open.sqlite3"));
  assert.equal(data.dataVersion, "2026.09");
  assert.equal(data.schemaVersion, "3");
  assert.equal(data.vpicRelease, "vPICList_lite_2026_09");
  assert.equal(data.builtAt, "2026-10-01 04:25:57");
  const database = new sqlite.DatabaseSync(data.path, { readOnly: true });
  assert.equal(database.prepare("SELECT COUNT(*) AS n FROM wmi").all()[0].n, 4);
  database.close();
});

test("prepare refuses a file that is not a data file", { skip }, () => {
  const other = dataFile("other.sqlite3", "CREATE TABLE t (a);");
  assert.throws(
    () => execFileSync("node", ["scripts/prepare.mjs", other, join(directory, "never")], { cwd: home, stdio: "pipe" }),
    /is not a Wenmar Open data file/,
  );
  const unversioned = dataFile("unversioned.sqlite3", fixtureSql.replace("('data_version', '2026.09'),", ""));
  assert.throws(
    () => execFileSync("node", ["scripts/prepare.mjs", unversioned, join(directory, "never")], { cwd: home, stdio: "pipe" }),
    /has no data_version/,
  );
});

test("a value is written as SQL that reads back as itself", () => {
  assert.equal(literal(null), "NULL");
  assert.equal(literal(7), "7");
  assert.equal(literal(2.5), "2.5");
  assert.equal(literal("O'Brien"), "'O''Brien'");
  assert.equal(literal("line\nbreak; DROP TABLE x"), "'line\nbreak; DROP TABLE x'");
  assert.throws(() => literal(new Uint8Array(1)));
  assert.throws(() => literal(Number.NaN));
  assert.throws(() => literal("a\u0000b"));
});

test("the SQL written for D1 rebuilds the same database, whatever the part size", { skip }, () => {
  const source = new sqlite.DatabaseSync(dataFile("d1.sqlite3"), { readOnly: true });
  for (const partBytes of [50 * 1024 * 1024, 2000]) {
    const parts = [];
    const summary = writeParts(source, partBytes, (name, text) => parts.push({ name, text }));
    assert.equal(summary.parts, parts.length);
    assert.deepEqual(parts.map((part) => part.name), parts.map((_, index) => `part-${String(index + 1).padStart(3, "0")}.sql`));
    if (partBytes === 2000) assert.ok(parts.length > 3, `${parts.length} parts`);

    const copy = new sqlite.DatabaseSync(":memory:");
    // In order, each file on its own, as `wrangler d1 execute --file` runs them.
    for (const part of parts) copy.exec(part.text);
    assert.deepEqual(contents(copy), contents(source));
    // And a second import over the first replaces it.
    for (const part of parts) copy.exec(part.text);
    assert.deepEqual(contents(copy), contents(source));
    copy.close();

    const text = parts.map((part) => part.text).join("");
    assert.doesNotMatch(text, /BEGIN|COMMIT|PRAGMA|sqlite_sequence/);
    assert.ok(summary.longestStatement <= STATEMENT_BYTES + 1000, `${summary.longestStatement}`);
  }
  source.close();
});

test("a table of many rows is written as several statements, none over D1's limit", { skip }, () => {
  const big = new sqlite.DatabaseSync(":memory:");
  big.exec("CREATE TABLE pattern (id INTEGER PRIMARY KEY, keys TEXT NOT NULL, value TEXT NOT NULL)");
  const insert = big.prepare("INSERT INTO pattern VALUES (?, ?, ?)");
  for (let id = 1; id <= 4000; id += 1) insert.run(id, "K2***", `It's value ${id} `.repeat(4));
  const parts = [];
  const summary = writeParts(big, 50 * 1024 * 1024, (name, text) => parts.push(text));
  assert.ok(summary.statements > 4, `${summary.statements} statements`);
  assert.ok(summary.longestStatement <= STATEMENT_BYTES, `${summary.longestStatement}`);
  const copy = new sqlite.DatabaseSync(":memory:");
  copy.exec(parts.join(""));
  assert.deepEqual(contents(copy), contents(big));
});

test("a VIN pattern too long for D1 to match stops the export", { skip }, () => {
  const long = new sqlite.DatabaseSync(":memory:");
  long.exec("CREATE TABLE pattern (id INTEGER PRIMARY KEY, keys TEXT NOT NULL)");
  long.prepare("INSERT INTO pattern VALUES (1, ?)").run("K".repeat(50));
  assert.throws(() => writeParts(long, 1024 * 1024, () => {}), /a VIN pattern is 50 bytes/);
});

test("the command writes the files and says how to import them", { skip }, () => {
  const out = join(directory, "d1-import");
  const said = execFileSync("node", ["bin/d1.mjs", "--file", dataFile("command.sqlite3"), "--out", out], { cwd: home, encoding: "utf8" });
  assert.deepEqual(readdirSync(out), ["part-001.sql"]);
  assert.match(said, /statements, 0\.0 MB, in 1 files in /);
  assert.match(said, /wrangler d1 execute wenmar-open-YYYY-MM --remote --yes --file="\$part"/);
  const dry = execFileSync("node", ["bin/d1.mjs", "--file", dataFile("dry.sqlite3"), "--dry-run"], { cwd: home, encoding: "utf8" });
  assert.match(dry, /dry run: nothing written/);
});
