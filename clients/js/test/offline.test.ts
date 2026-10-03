// The offline entry point, with no SQLite and no network: the decoder is
// the built WebAssembly and the database is the rows recorded in
// test/fixtures/offline-cases.json. Runs on every supported Node.
import assert from "node:assert/strict";
import { after, before, test } from "node:test";

import { WenmarOpen, WenmarOpenError as HostedError } from "wenmar-open";
import { MAX_BATCH, WenmarOpenError, WenmarOpenOffline, d1Store, isBatchError } from "wenmar-open/offline";
import type { Store } from "wenmar-open/offline";

import {
  FakeD1,
  RecordedStore,
  STOPPING_WASM,
  ask,
  caseNamed,
  fixture,
  outcome,
  rowsOf,
  sameSurface,
} from "./support/offline.js";
import { ApiServer } from "./support/server.js";

const KONA = "KM8K2CAB4PU001140";
const YEAR = fixture.current_year;

function offline(store: Store = new RecordedStore()): WenmarOpenOffline {
  return new WenmarOpenOffline({ store, currentYear: YEAR });
}

async function failure(call: Promise<unknown>): Promise<WenmarOpenError> {
  try {
    await call;
  } catch (error) {
    assert.ok(error instanceof WenmarOpenError, `threw ${String(error)}`);
    return error;
  }
  throw new assert.AssertionError({ message: "the call did not fail" });
}

// ----- the same answers -----

test("every case is answered as the libraries answer it natively", async () => {
  const client = offline();
  for (const one of fixture.cases) {
    assert.deepEqual(await outcome(ask(sameSurface(client), one)), one.answer, one.name);
  }
});

let api: ApiServer;
before(async () => {
  api = await ApiServer.start();
});
after(async () => {
  await api.close();
});

const STATUS: Record<string, number> = { invalid_vin: 400, validation_failed: 400, not_found: 404 };

test("the hosted client and the offline client give the same answer and the same error", async () => {
  const hosted = new WenmarOpen({ baseUrl: api.url });
  const local = offline();
  for (const one of fixture.cases) {
    // The API's answer for this case, served as the API would serve it.
    if ("ok" in one.answer) {
      api.next({ body: one.answer.ok });
    } else {
      api.next({ status: STATUS[one.answer.error.code] ?? 500, body: { error: one.answer.error } });
    }
    const [overHttp, here] = [await outcome(ask(hosted, one)), await outcome(ask(local, one))];
    assert.deepEqual(here, overHttp, one.name);
  }
});

test("there is one error class, whichever entry point it came from", async () => {
  const error = await failure(offline().decodeVin("KM8"));
  assert.ok(error instanceof HostedError);
  assert.equal(error.name, "WenmarOpenError");
  assert.equal(error.code, "invalid_vin");
  assert.equal(error.status, undefined);
  assert.deepEqual(error.details, { suggestions: [] });
});

test("trims is submodels under another name", async () => {
  const client = offline();
  const query = { make: "honda", model: "civic", year: 2019 };
  assert.deepEqual(await client.trims(query), await client.submodels(query));
});

// ----- batches -----

test("a batch answers in order, with an error in the place of a VIN that has one", async () => {
  const store = new RecordedStore();
  const items = await offline(store).decodeVins([KONA, "KM8", "ZZZK2CAB4PU001140", KONA]);
  assert.equal(items.length, 4);
  const [first, second, third, fourth] = items;
  assert.ok(first !== undefined && second !== undefined && third !== undefined && fourth !== undefined);
  assert.ok(!isBatchError(first));
  assert.equal(first.vin, KONA);
  assert.ok(isBatchError(second));
  assert.equal(second.error.code, "invalid_vin");
  assert.ok(isBatchError(third));
  assert.equal(third.error.code, "not_found");
  assert.deepEqual(fourth, first);
  // The VINs are read together: as many steps as the slowest needs, and a
  // statement two of them need is run once. One step more is the opening.
  assert.equal(store.steps, 1 + caseNamed("decode").steps);
  const run = store.statements.map((statement) => `${statement.sql}${JSON.stringify(statement.params)}`);
  assert.equal(new Set(run).size, run.length);
});

test("a batch over 50 is refused before anything is read", async () => {
  const store = new RecordedStore();
  const error = await failure(offline(store).decodeVins(Array.from({ length: MAX_BATCH + 1 }, () => KONA)));
  assert.equal(error.code, "validation_failed");
  assert.deepEqual(error.details, { field: "vins", max: 50, received: 51 });
  assert.equal(store.steps, 0);
});

// ----- how often the store is read -----

test("a question reads the store as often as the fixture says, after one opening", async () => {
  for (const name of ["decode", "decode, bus of 2021", "search", "makes", "decode, too short"]) {
    const store = new RecordedStore();
    const client = offline(store);
    await client.meta();
    assert.equal(store.steps, 1, "opening is one step");
    assert.equal(store.statements.length, 6);
    await outcome(ask(client, caseNamed(name)));
    assert.equal(store.steps - 1, caseNamed(name).steps, name);
  }
});

test("the data is opened once, however many questions arrive together", async () => {
  const store = new RecordedStore();
  const client = offline(store);
  const answers = await Promise.all([client.decodeVin(KONA), client.years(), client.meta(), client.decodeVin(KONA)]);
  assert.equal(answers[0].vin, KONA);
  const openings = store.statements.filter((statement) => statement.sql === "SELECT key, value FROM meta");
  assert.equal(openings.length, 1);
});

// Review Focus 4, through the built WebAssembly.
test("a VIN whose model-year cycle the VIN does not settle is decoded in both and the better kept", async () => {
  const client = offline();
  assert.equal((await client.decodeVin("1M8PDMPA1MP000001")).year, 2021);
  assert.equal((await client.decodeVin("1M8PEMPA5MP000001")).year, 1991);
});

test("the current year is the one given, and a year override is checked against it", async () => {
  const client = new WenmarOpenOffline({ store: new RecordedStore(), currentYear: 2030 });
  const error = await failure(client.decodeVin(KONA, { year: 2040 }));
  assert.equal(error.code, "validation_failed");
  assert.deepEqual(error.details, { field: "year", min: 1980, max: 2032 });
});

// ----- an asynchronous store -----

test("a D1 database gives the same answers, one query for each statement", async () => {
  const database = new FakeD1();
  const client = offline(d1Store(database));
  for (const one of fixture.cases) {
    assert.deepEqual(await outcome(ask(client, one)), one.answer, one.name);
  }
  const opening = 6;
  await client.decodeVin(KONA);
  const before = database.queries;
  await client.decodeVin(KONA);
  assert.equal(database.queries - before, 15, "statements for one decode");
  assert.ok(before > opening);
});

// Review Focus 3.
test("a D1 query that fails midway fails that call with store_error and leaves the client usable", async () => {
  const database = new FakeD1();
  const client = offline(d1Store(database));
  await client.meta();
  // The fourth query of the decode: after its manufacturer, schemas and patterns.
  database.failAt = database.queries + 4;
  const error = await failure(client.decodeVin(KONA));
  assert.equal(error.code, "store_error");
  assert.equal(error.message, "The database failed while it was being read.");
  assert.match(String((error.cause as Error).message), /Network connection lost/);
  // Nothing of the failed decode is kept: the next one starts over and is whole.
  database.failAt = undefined;
  assert.deepEqual({ ok: await client.decodeVin(KONA) }, caseNamed("decode").answer);
});

test("a store that fails while the data is opened is store_error with its cause, and opening is tried again", async () => {
  const database = new FakeD1();
  database.failAt = 2;
  const client = offline(d1Store(database));
  const error = await failure(client.meta());
  assert.equal(error.code, "store_error");
  assert.equal(error.message, "The database failed while it was being read.");
  assert.match(String((error.cause as Error).message), /Network connection lost/);
  database.failAt = undefined;
  assert.equal((await client.meta()).data_version, "2026.09");
});

// ----- what a store may return -----

// Review Focus 2.
test("a store that returns every integer as a BigInt is read as it is meant", async () => {
  const store = new RecordedStore();
  store.alter = (rows) => rows.map((row) => row.map((value) => (typeof value === "number" ? BigInt(value) : value)));
  assert.deepEqual({ ok: await offline(store).decodeVin(KONA) }, caseNamed("decode").answer);
});

test("a BigInt too large to read exactly, bytes, and a row that is an object are refused by name", async () => {
  const spoil = (value: unknown) => {
    const store = new RecordedStore();
    store.alter = (rows, statement) =>
      statement.sql.includes("FROM pattern") ? rows.map((row) => row.map((cell, column) => (column === 0 ? value : cell))) : rows;
    return failure(offline(store).decodeVin(KONA));
  };
  const large = await spoil(2n ** 60n);
  assert.equal(large.code, "data_invalid");
  assert.equal(large.message, "The database returned a whole number too large to read exactly.");
  assert.equal(large.details["column"], 0);

  const bytes = await spoil(new Uint8Array([1, 2, 3]));
  assert.equal(bytes.code, "data_invalid");
  assert.equal(bytes.message, "The database returned a value that is not a number, text or null.");
  assert.equal(bytes.details["kind"], "bytes");

  const store = new RecordedStore();
  store.alter = (rows, statement) =>
    statement.sql.includes("FROM pattern") ? (rows.map((row) => ({ ...row })) as unknown as unknown[][]) : rows;
  const objects = await failure(offline(store).decodeVin(KONA));
  assert.equal(objects.code, "store_error");
  assert.match(objects.message, /each row as an array/);
});

test("text where a number belongs is data_invalid from the decoder, not a stop", async () => {
  const store = new RecordedStore();
  store.alter = (rows, statement) =>
    statement.sql.includes("FROM pattern") ? rows.map((row) => row.map(() => "text")) : rows;
  const client = offline(store);
  const error = await failure(client.decodeVin(KONA));
  assert.equal(error.code, "data_invalid");
  assert.equal(error.message, "The data has an unexpected row in pattern.");
  store.alter = (rows) => rows;
  assert.equal((await client.decodeVin(KONA)).vin, KONA);
});

test("a store that returns too few answers is store_error", async () => {
  const store: Store = { query: (statements) => statements.slice(1).map((statement) => rowsOf(statement)) };
  const client = offline(store);
  const error = await failure(client.decodeVin(KONA));
  assert.ok(error.code === "store_error" || error.code === "data_invalid", error.code);
});

// Review Focus 1, through the client.
test("a data file of another schema version is refused on first use, with both versions", async () => {
  const store = new RecordedStore();
  store.alter = (rows, statement) =>
    statement.sql === "SELECT key, value FROM meta"
      ? rows.map((row) => (row[0] === "schema_version" ? ["schema_version", "2"] : row))
      : rows;
  const client = offline(store);
  const error = await failure(client.decodeVin(KONA));
  assert.equal(error.code, "data_invalid");
  assert.equal(
    error.message,
    "The data file has schema version 2. This version of wenmar-open reads schema version 3.",
  );
  assert.deepEqual(error.details, { schema_version: "2", expected: "3" });
  // Every later call says the same; none reads past the opening.
  assert.equal((await failure(client.years())).code, "data_invalid");
  assert.ok(store.statements.every((statement) => !statement.sql.includes("FROM pattern")));
});

// ----- stopping -----

test("an aborted signal and a time limit stop a question between reads of the store", async () => {
  const client = offline();
  const aborted = await failure(client.decodeVin(KONA, { signal: AbortSignal.abort() }));
  assert.equal(aborted.code, "aborted");

  let reads = 0;
  const slow: Store = {
    query: async (statements) => {
      reads += 1;
      await new Promise((resolve) => setTimeout(resolve, 15));
      return statements.map((statement) => rowsOf(statement));
    },
  };
  const slowClient = offline(slow);
  await slowClient.meta();
  reads = 0;
  const late = await failure(slowClient.decodeVin(KONA, { timeoutMs: 20 }));
  assert.equal(late.code, "timeout");
  assert.deepEqual(late.details, { timeout_ms: 20 });
  assert.ok(reads < caseNamed("decode").steps, `${reads} reads`);
});

// ----- the WebAssembly -----

// Review Focus 5.
test("the decoder loads with no fetch, no file system and no network", async () => {
  const names = ["fetch", "XMLHttpRequest", "WebSocket"] as const;
  const kept = names.map((name) => [name, Object.getOwnPropertyDescriptor(globalThis, name)] as const);
  const used: string[] = [];
  for (const name of names) {
    Object.defineProperty(globalThis, name, {
      configurable: true,
      get: () => {
        used.push(name);
        return undefined;
      },
    });
  }
  try {
    const client = offline();
    assert.deepEqual({ ok: await client.decodeVin(KONA) }, caseNamed("decode").answer);
    assert.equal((await client.search({ q: "2019 civic si" })).length, 2);
  } finally {
    for (const [name, descriptor] of kept) {
      if (descriptor === undefined) delete (globalThis as Record<string, unknown>)[name];
      else Object.defineProperty(globalThis, name, descriptor);
    }
  }
  assert.deepEqual(used, []);
});

test("a compiled module or its bytes can be given, as a Cloudflare Worker must", async () => {
  const { readFileSync } = await import("node:fs");
  const { createRequire } = await import("node:module");
  // The path a Worker imports: wenmar-open/offline.wasm.
  const bytes = readFileSync(createRequire(import.meta.url).resolve("wenmar-open/offline.wasm"));
  const module = new WebAssembly.Module(bytes);
  for (const wasm of [module, bytes, bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength)]) {
    const client = new WenmarOpenOffline({ store: new RecordedStore(), wasm, currentYear: YEAR });
    assert.equal((await client.decodeVin(KONA)).year, 2023);
  }
});

test("where bytes may not be compiled, the error says what to pass", async () => {
  const compile = WebAssembly.compile;
  WebAssembly.compile = () => Promise.reject(new Error("Wasm code generation disallowed by embedder"));
  try {
    const error = await failure(offline().decodeVin(KONA));
    assert.equal(error.code, "internal_error");
    assert.match(error.message, /import wasm from "wenmar-open\/offline.wasm"/);
  } finally {
    WebAssembly.compile = compile;
  }
});

test("something that is not the decoder is refused when it starts", async () => {
  const empty = Uint8Array.from([0, 0x61, 0x73, 0x6d, 1, 0, 0, 0]);
  const client = new WenmarOpenOffline({ store: new RecordedStore(), wasm: empty });
  const error = await failure(client.meta());
  assert.equal(error.code, "internal_error");
  assert.deepEqual(error.details, { missing: "memory" });
});

test("a stop inside the decoder is an error with its message, and the next call starts a new copy", async () => {
  const instantiate = WebAssembly.instantiate;
  let started = 0;
  WebAssembly.instantiate = ((...args: Parameters<typeof instantiate>) => {
    started += 1;
    return instantiate(...args);
  }) as typeof instantiate;
  try {
    const client = new WenmarOpenOffline({ store: new RecordedStore(), wasm: STOPPING_WASM });
    const first = await failure(client.decodeVin(KONA));
    assert.equal(first.code, "internal_error");
    assert.deepEqual(first.details, { panic: "the decoder panicked" });
    assert.ok(first.cause instanceof WebAssembly.RuntimeError);
    const second = await failure(client.years());
    assert.equal(second.code, "internal_error");
    assert.equal(started, 2);
  } finally {
    WebAssembly.instantiate = instantiate;
  }
});

test("a stop after a successful opening is an error, and the next question starts a new copy that opens the data again", async () => {
  const store = new RecordedStore();
  const client = offline(store);
  const openings = () => store.statements.filter((statement) => statement.sql === "SELECT key, value FROM meta").length;

  // The first copy of the real decoder stops on the call after `trap` is set.
  const instantiate = WebAssembly.instantiate;
  let started = 0;
  let trap = false;
  WebAssembly.instantiate = (async (...args: Parameters<typeof instantiate>) => {
    started += 1;
    const instance = await instantiate(...args);
    if (started > 1) return instance;
    const real = instance.exports as { wo_call(pointer: number, length: number): number };
    const exports = {
      ...instance.exports,
      wo_call: (pointer: number, length: number) => {
        if (trap) throw new WebAssembly.RuntimeError("unreachable");
        return real.wo_call(pointer, length);
      },
    };
    return { exports } as unknown as typeof instance;
  }) as typeof instantiate;
  try {
    await client.meta();
    assert.equal(openings(), 1);
    trap = true;
    const stopped = await failure(client.years());
    assert.equal(stopped.code, "internal_error");
    assert.ok(stopped.cause instanceof WebAssembly.RuntimeError);
    // The answer of the question before is not taken for what stopped it.
    assert.deepEqual(stopped.details, {});
    // A new copy answers, and it opened the data again.
    assert.deepEqual({ ok: await client.decodeVin(KONA) }, caseNamed("decode").answer);
    assert.equal(started, 2);
    assert.equal(openings(), 2);
  } finally {
    WebAssembly.instantiate = instantiate;
  }
});

test("a question that cannot be written as JSON is validation_failed, and the decoder goes on", async () => {
  const store = new RecordedStore();
  const client = offline(store);
  const openings = () => store.statements.filter((statement) => statement.sql === "SELECT key, value FROM meta").length;
  await client.meta();
  const error = await failure(client.makes({ year: 10n as never }));
  assert.equal(error.code, "validation_failed");
  assert.equal(error.details["panic"], undefined);
  assert.ok(error.cause instanceof TypeError);
  // The same copy of the decoder answers the next question.
  assert.equal((await client.meta()).data_version, "2026.09");
  assert.equal(openings(), 1);
});
