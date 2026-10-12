// The scripts of wenmar-open-data-shop, against the small data file and the
// small shop file the npm client's tests use. Needs node:sqlite: Node 22.16
// or later. On an older Node every test here that reads a file is skipped.
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

import { statements } from "../../data/lib/d1.mjs";
import { HOSTED_ONLY, hostedOnlyIn } from "../lib/hosted-only.mjs";
import { npmShopVersion, shopDataVersion } from "../lib/version.mjs";

const sqlite = await import("node:sqlite").then(
  (module) => (typeof module.StatementSync.prototype.setReturnArrays === "function" ? module : undefined),
  () => undefined,
);
const skip = sqlite === undefined ? "needs node:sqlite of Node 22.16 or later" : false;

const home = fileURLToPath(new URL("../", import.meta.url));
const prepare = join(home, "scripts", "prepare.mjs");
const coreSql = readFileSync(new URL("../../js/test/fixtures/offline.sql", import.meta.url), "utf8");
const shopSql = readFileSync(new URL("../../js/test/fixtures/offline-shop.sql", import.meta.url), "utf8");
// The real path: on macOS the temporary directory is reached through a link.
const directory = realpathSync(mkdtempSync(join(tmpdir(), "wenmar-open-data-shop-")));
after(() => rmSync(directory, { recursive: true, force: true }));

function file(name, sql) {
  const path = join(directory, name);
  const database = new sqlite.DatabaseSync(path);
  database.exec(sql);
  database.close();
  return path;
}

/** Runs prepare.mjs and gives its exit code and what it wrote to standard error. */
function refused(args, env = {}) {
  const run = spawnSync(process.execPath, [prepare, ...args], { encoding: "utf8", env: { ...process.env, ...env } });
  return { status: run.status, stderr: run.stderr };
}

test("the npm version puts the schema first, then the day, then the rebuild", () => {
  assert.equal(npmShopVersion("1", "2026.10.12"), "1.20261012.0");
  assert.equal(npmShopVersion("1", "2026.10.12.1"), "1.20261012.1");
  assert.equal(npmShopVersion(2, "2027.01.04.12"), "2.20270104.12");
  for (const [schema, data] of [
    ["1", "2026.10"],
    ["1", "2026.10.1"],
    ["1", "2026.13.01"],
    ["1", "2026.10.32"],
    ["1", "26.10.12"],
    ["01", "2026.10.12"],
    ["", "2026.10.12"],
    ["1", "2026.10.12.0"],
    ["1", "2026.10.12.01"],
    ["1", ""],
  ]) {
    assert.throws(() => npmShopVersion(schema, data), `${schema} ${data}`);
  }
  assert.equal(shopDataVersion("1.20261012.0"), "2026.10.12");
  assert.equal(shopDataVersion("1.20261012.1"), "2026.10.12.1");
  assert.throws(() => shopDataVersion("3.202609.0"));
  // Later files are later versions, as a version range needs.
  assert.ok(Number("20261019") > Number("20261012"));
});

test("the committed manifest ships the shop file and carries a version that must never be published", () => {
  const manifest = JSON.parse(readFileSync(join(home, "package.json"), "utf8"));
  assert.equal(manifest.name, "wenmar-open-data-shop");
  assert.equal(manifest.version, "0.0.0");
  assert.ok(manifest.files.includes("wenmar-open-shop.sqlite3"));
  assert.equal(manifest.dependencies, undefined);
  for (const hook of ["preinstall", "install", "postinstall"]) assert.equal(manifest.scripts[hook], undefined);
});

test("prepare writes the package: the file, its versions, and the one data package it goes with", { skip }, async () => {
  const [shop, core, out] = [file("shop.sqlite3", shopSql), file("core.sqlite3", coreSql), join(directory, "build")];
  const printed = execFileSync(process.execPath, [prepare, shop, core, out], { encoding: "utf8" });
  assert.match(printed, /^wenmar-open-data-shop@1\.20261012\.0: shop data 2026\.10\.12 for wenmar-open-data@3\.202609\.0, schema 1, recalls_us, recalls_ca, ratings, /);
  assert.deepEqual(readdirSync(out).sort(), ["LICENSE", "NOTICE.md", "README.md", "index.d.ts", "index.js", "package.json", "wenmar-open-shop.sqlite3"]);
  const manifest = JSON.parse(readFileSync(join(out, "package.json"), "utf8"));
  assert.equal(manifest.version, "1.20261012.0");
  assert.deepEqual(manifest.peerDependencies, { "wenmar-open-data": "3.202609.0" });
  assert.equal(manifest.scripts, undefined);
  const built = await import(pathToFileURL(join(out, "index.js")).href);
  assert.equal(built.path, join(out, "wenmar-open-shop.sqlite3"));
  assert.equal(built.dataVersion, "2026.10.12");
  assert.equal(built.catalogDataVersion, "2026.09");
  assert.equal(built.schemaVersion, "1");
  assert.equal(built.builtAt, "2026-10-12 06:00:00");
  assert.deepEqual(Object.keys(built.datasets), ["recalls_us", "recalls_ca", "ratings"]);
  assert.equal(built.datasets.recalls_ca.licence, "ogl-canada");
  // The file in the package is the file that was given.
  assert.deepEqual(readFileSync(built.path), readFileSync(shop));
});

test("prepare refuses a pair that do not go together, a file that is not one, and a file that is too large", { skip }, () => {
  const [shop, core, out] = [file("pair-shop.sqlite3", shopSql), file("pair-core.sqlite3", coreSql), join(directory, "refused")];
  const other = file("other-core.sqlite3", coreSql.replace("('data_version', '2026.09')", "('data_version', '2026.10')"));
  const cases = [
    [[shop, other, out], /was matched against data version 2026\.09, and .* is data version 2026\.10/],
    [[core, core, out], /is not a Wenmar Open shop file: its meta table has no catalog_data_version/],
    [[shop, shop, out], /is not a Wenmar Open data file: its meta table has no vpic_release/],
    [[join(directory, "missing.sqlite3"), core, out], /is not a Wenmar Open shop file/],
    [[shop, core, directory], /would delete the data file or the package/],
  ];
  for (const [args, message] of cases) {
    const run = refused(args);
    assert.equal(run.status, 1, run.stderr);
    assert.match(run.stderr, message);
  }
  const large = refused([shop, core, out], { WENMAR_OPEN_SHOP_MAX_BYTES: "1000" });
  assert.equal(large.status, 1);
  assert.match(large.stderr, /the largest shop file published is 1000/);
  assert.equal(refused([shop]).status, 2);
  // Nothing was written by a refusal.
  assert.ok(!readdirSync(directory).includes("refused"));
});

test("the D1 exporter of wenmar-open-data writes the shop file too, and it reads back the same", { skip }, () => {
  const database = new sqlite.DatabaseSync(file("d1-shop.sqlite3", shopSql), { readOnly: true });
  const sql = [...statements(database)];
  assert.ok(sql.length >= 8, `${sql.length} statements`);
  for (const statement of sql) assert.ok(new TextEncoder().encode(statement).length <= 100_000);
  const copy = new sqlite.DatabaseSync(":memory:");
  for (const statement of sql) copy.exec(statement);
  for (const table of ["meta", "recall", "recall_vehicle", "rating", "rating_vehicle"]) {
    const rows = (from) => from.prepare(`SELECT * FROM "${table}" ORDER BY rowid`).all();
    assert.deepEqual(rows(copy), rows(database), table);
  }
  const indexes = copy.prepare("SELECT count(*) AS found FROM sqlite_schema WHERE type = 'index' AND sql IS NOT NULL").get();
  assert.equal(indexes.found, 6);
  database.close();
  copy.close();
});

// ----- the package never carries a hosted-only dataset -----

test("the hosted-only list is the one the Rust crate has", () => {
  const schema = readFileSync(new URL("../../../crates/wenmar-shopdata/src/schema.rs", import.meta.url), "utf8");
  const start = schema.indexOf("pub const HOSTED_ONLY_TABLES");
  assert.ok(start >= 0, "schema.rs has no HOSTED_ONLY_TABLES");
  const list = schema.slice(start, schema.indexOf(";", start));
  // Each entry is `("dataset id", &["table", ...])`.
  const ids = [...list.matchAll(/\(\s*"([a-z_]+)",\s*&\[/g)].map((match) => match[1]);
  assert.ok(ids.length > 0, list);
  assert.deepEqual([...HOSTED_ONLY].sort(), ids.sort());
  assert.deepEqual(hostedOnlyIn({ recalls_us: {}, bulletins: {}, ratings: {} }), ["bulletins"]);
  assert.deepEqual(hostedOnlyIn({ recalls_us: {} }), []);
});

test("prepare refuses the full file: one that holds a hosted-only dataset", { skip }, () => {
  const fullSql = shopSql.replace(
    `"ratings":{`,
    `"bulletins":{"as_of":"2026-10-11","rows":2,"licence":"public-domain","attribution":""},"ratings":{`,
  );
  assert.notEqual(fullSql, shopSql);
  const [full, core, out] = [file("full-shop.sqlite3", fullSql), file("full-core.sqlite3", coreSql), join(directory, "full-refused")];
  const run = refused([full, core, out]);
  assert.equal(run.status, 1, run.stderr);
  assert.match(run.stderr, /full-shop\.sqlite3 holds bulletins, which only the hosted service carries\. Give the offline shop file/);
  // Nothing was written.
  assert.ok(!readdirSync(directory).includes("full-refused"));
  // The same file without that dataset is the offline file, and is taken.
  const taken = refused([file("offline-shop.sqlite3", shopSql), core, out]);
  assert.equal(taken.status, 0, taken.stderr);
  const built = JSON.parse(readFileSync(join(out, "package.json"), "utf8"));
  assert.equal(built.version, "1.20261012.0");
});
