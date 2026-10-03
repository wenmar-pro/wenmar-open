// The offline entry point over a real SQLite: Node's own. These tests are
// skipped where node:sqlite cannot return rows as arrays (before Node
// 22.16); test/offline.test.ts covers the decoder there.
import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";

import { WenmarOpenError, WenmarOpenOffline, syncStore } from "wenmar-open/offline";
import type { Param, SyncDatabase } from "wenmar-open/offline";
import { openOffline } from "wenmar-open/offline/node";

import { ask, caseNamed, fixture, fixtureSql, outcome } from "./support/offline.js";

type Sqlite = typeof import("node:sqlite");
const sqlite: Sqlite | undefined = await import("node:sqlite").then(
  (module) => (typeof module.StatementSync.prototype.setReturnArrays === "function" ? module : undefined),
  () => undefined,
);
const skip = sqlite === undefined ? "node:sqlite with setReturnArrays needs Node 22.16 or later" : false;

const KONA = "KM8K2CAB4PU001140";
const YEAR = fixture.current_year;
const directory = mkdtempSync(join(tmpdir(), "wenmar-open-"));
after(() => rmSync(directory, { recursive: true, force: true }));

/** The fixture as a data file on disk, at a schema version. */
function dataFile(name: string, schemaVersion = "3"): string {
  if (sqlite === undefined) throw new Error("skipped");
  const path = join(directory, name);
  const database = new sqlite.DatabaseSync(path);
  database.exec(fixtureSql.replace("('schema_version', '3')", `('schema_version', '${schemaVersion}')`));
  database.close();
  return path;
}

function memory(): InstanceType<Sqlite["DatabaseSync"]> {
  if (sqlite === undefined) throw new Error("skipped");
  const database = new sqlite.DatabaseSync(":memory:");
  database.exec(fixtureSql);
  return database;
}

test("every case, read from SQLite through node:sqlite", { skip }, async () => {
  const database = memory();
  const client = new WenmarOpenOffline({ store: syncStore(database), currentYear: YEAR });
  for (const one of fixture.cases) {
    assert.deepEqual(await outcome(ask(client, one)), one.answer, one.name);
  }
  // The caller opened the database, so the caller closes it.
  client.close();
  assert.equal(database.prepare("SELECT 1").all().length, 1);
  database.close();
});

test("a database handle of better-sqlite3's shape, and binder, gives the same answers", { skip }, async () => {
  const database = memory();
  // better-sqlite3: `statement.raw(true)` returns the statement, and rows
  // then come back as arrays. It reads `?1` as a parameter named `1`, so a
  // positional value for it is refused (checked against the real library:
  // "Missing named parameters"); only an object keyed by number binds.
  const shaped: SyncDatabase = {
    prepare(sql: string) {
      const statement = database.prepare(sql);
      return {
        raw(enabled = true) {
          statement.setReturnArrays(enabled);
          return this;
        },
        all: (...params: unknown[]) => {
          const positional = params.some((value) => typeof value !== "object" || value === null);
          if (positional && /\?\d/.test(sql)) throw new TypeError("Missing named parameters");
          return statement.all(...(params as Param[]));
        },
      };
    },
  };
  const client = new WenmarOpenOffline({ store: syncStore(shaped), currentYear: YEAR });
  for (const one of fixture.cases) {
    assert.deepEqual(await outcome(ask(client, one)), one.answer, one.name);
  }
  database.close();
});

test("a database set to read every integer as a BigInt gives the same answers", { skip }, async () => {
  const database = memory();
  const bigints: SyncDatabase = {
    prepare(sql: string) {
      const statement = database.prepare(sql);
      statement.setReadBigInts(true);
      return statement;
    },
  };
  const client = new WenmarOpenOffline({ store: syncStore(bigints), currentYear: YEAR });
  assert.deepEqual({ ok: await client.decodeVin(KONA) }, caseNamed("decode").answer);
  database.close();
});

test("a database that cannot return rows as arrays is refused, not misread", { skip }, async () => {
  const database = memory();
  const objects: SyncDatabase = { prepare: (sql: string) => ({ all: (...params: Param[]) => database.prepare(sql).all(...params) }) };
  const client = new WenmarOpenOffline({ store: syncStore(objects), currentYear: YEAR });
  await assert.rejects(client.meta(), (error: unknown) => {
    assert.ok(error instanceof WenmarOpenError);
    assert.equal(error.code, "store_error");
    assert.match(error.message, /cannot return rows as arrays/);
    return true;
  });
  database.close();
});

test("openOffline reads a data file at a path, read-only, and closes it", { skip }, async () => {
  const path = dataFile("data.sqlite3");
  const client = await openOffline({ path, currentYear: YEAR });
  assert.deepEqual({ ok: await client.decodeVin(KONA) }, caseNamed("decode").answer);
  assert.equal((await client.meta()).data_version, "2026.09");
  client.close();
  await assert.rejects(client.years());
});

// Review Focus 1, at the door.
test("openOffline refuses a data file of another schema version before any question", { skip }, async () => {
  const path = dataFile("old.sqlite3", "2");
  await assert.rejects(openOffline({ path }), (error: unknown) => {
    assert.ok(error instanceof WenmarOpenError);
    assert.equal(error.code, "data_invalid");
    assert.deepEqual(error.details, { schema_version: "2", expected: "3" });
    return true;
  });
});

// A file from before the catalog's tables existed has none of them.
test("a data file with only a meta table of another schema version is refused by its version, not by a missing table", { skip }, async () => {
  if (sqlite === undefined) throw new Error("skipped");
  const path = join(directory, "meta-only.sqlite3");
  const database = new sqlite.DatabaseSync(path);
  database.exec("CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL); INSERT INTO meta VALUES ('schema_version', '2');");
  database.close();
  const refused = (error: unknown) => {
    assert.ok(error instanceof WenmarOpenError);
    assert.equal(error.code, "data_invalid");
    assert.deepEqual(error.details, { schema_version: "2", expected: "3" });
    return true;
  };
  await assert.rejects(openOffline({ path }), refused);
  const opened = new sqlite.DatabaseSync(path, { readOnly: true });
  const client = new WenmarOpenOffline({ store: syncStore(opened), currentYear: YEAR });
  await assert.rejects(client.decodeVin(KONA), refused);
  opened.close();
});

test("openOffline: a missing file is no_data, and a file that is not a data file is data_invalid", { skip }, async () => {
  await assert.rejects(openOffline({ path: join(directory, "missing.sqlite3") }), (error: unknown) => {
    assert.ok(error instanceof WenmarOpenError);
    assert.equal(error.code, "no_data");
    assert.equal(error.details["path"], join(directory, "missing.sqlite3"));
    return true;
  });
  const other = join(directory, "other.sqlite3");
  writeFileSync(other, "this is not a database");
  await assert.rejects(openOffline({ path: other }), (error: unknown) => {
    assert.ok(error instanceof WenmarOpenError);
    assert.equal(error.code, "data_invalid");
    return true;
  });
  // The missing file was not created by looking for it.
  assert.throws(() => rmSync(join(directory, "missing.sqlite3")));
});

test("openOffline finds the file of the wenmar-open-data package, and says so when it is not installed", { skip }, async () => {
  await assert.rejects(openOffline(), (error: unknown) => {
    assert.ok(error instanceof WenmarOpenError);
    assert.equal(error.code, "no_data");
    assert.match(error.message, /Install the wenmar-open-data package/);
    return true;
  });
  // A stand-in for the package, where Node looks for it from dist/offline/.
  const home = new URL("../node_modules/wenmar-open-data/", import.meta.url);
  mkdirSync(home, { recursive: true });
  try {
    writeFileSync(new URL("package.json", home), JSON.stringify({ name: "wenmar-open-data", version: "3.202609.0", type: "module", exports: "./index.js" }));
    writeFileSync(new URL("index.js", home), `export const path = ${JSON.stringify(dataFile("packaged.sqlite3"))};\n`);
    const client = await openOffline({ currentYear: YEAR });
    assert.equal((await client.decodeVin(KONA)).year, 2023);
    client.close();
  } finally {
    rmSync(home, { recursive: true, force: true });
  }
});
