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

const STATUS: Record<string, number> = {
  invalid_vin: 400,
  validation_failed: 400,
  not_found: 404,
  dataset_unavailable: 404,
};

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
  // statement two of them need is run once. Two steps more are the opening.
  assert.equal(store.steps, 2 + caseNamed("decode").steps);
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
    assert.equal(store.steps, 2, "opening is two steps: meta, then the catalog");
    assert.equal(store.statements.length, 6);
    await outcome(ask(client, caseNamed(name)));
    assert.equal(store.steps - 2, caseNamed(name).steps, name);
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
  assert.equal(error.code, "store_error");
  assert.equal(error.message, "The store must return one list of rows for each statement.");
});

test("a store that returns nothing, or something that is not a list, is store_error and never a TypeError", async () => {
  for (const returned of [undefined, null, "rows", { length: 1 }]) {
    const store: Store = { query: () => returned as unknown as unknown[][][] };
    // `failure` fails the test for anything that is not a WenmarOpenError.
    const error = await failure(offline(store).decodeVin(KONA));
    assert.equal(error.code, "store_error", String(returned));
    assert.equal(error.message, "The store must return one list of rows for each statement.");
  }
});

test("a store that returns too few answers after the data is opened is store_error", async () => {
  const recorded = new RecordedStore();
  let short = false;
  const store: Store = {
    query: (statements) => (short ? recorded.query(statements).slice(1) : recorded.query(statements)),
  };
  const client = offline(store);
  await client.meta();
  short = true;
  const error = await failure(client.decodeVin(KONA));
  assert.equal(error.code, "store_error");
  assert.equal(error.message, "The store must return one list of rows for each statement.");
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
  // Nothing but meta: a file of another layout may lack every other table.
  assert.ok(store.statements.length > 0);
  assert.ok(store.statements.every((statement) => statement.sql === "SELECT key, value FROM meta"));
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

// ----- recalls, from the shop file -----

/** A client over the recorded rows of both files, and the two stores. */
function withShop(): { client: WenmarOpenOffline; store: RecordedStore; shop: RecordedStore } {
  const [store, shop] = [new RecordedStore(), new RecordedStore()];
  return { client: new WenmarOpenOffline({ store, shop, currentYear: YEAR }), store, shop };
}

test("every recall case is answered as the libraries answer it natively, each file read through its own store", async () => {
  const { client, store, shop } = withShop();
  for (const one of fixture.shop_cases) {
    assert.deepEqual(await outcome(ask(sameSurface(client), one)), one.answer, one.name);
  }
  assert.ok(shop.statements.length > 0);
  assert.ok(shop.statements.every((statement) => statement.db === "shop"));
  assert.ok(store.statements.every((statement) => statement.db === undefined));
  // The data file's questions are answered as before with a shop store
  // given. `meta` and the overview questions are set aside: they read the
  // shop file when there is one, and so answer more than they do without.
  const readsShop = new Set(["meta", "overview_vehicle", "overview_vin", "model_years"]);
  for (const one of fixture.cases.filter((candidate) => !readsShop.has(candidate.op))) {
    assert.deepEqual(await outcome(ask(client, one)), one.answer, one.name);
  }
});

test("the hosted client and the offline client give the same recall answer and the same error", async () => {
  const hosted = new WenmarOpen({ baseUrl: api.url });
  const { client } = withShop();
  for (const one of fixture.shop_cases) {
    if ("ok" in one.answer) {
      api.next({ body: one.answer.ok });
    } else {
      api.next({ status: STATUS[one.answer.error.code] ?? 500, body: { error: one.answer.error } });
    }
    const [overHttp, here] = [await outcome(ask(hosted, one)), await outcome(ask(client, one))];
    assert.deepEqual(here, overHttp, one.name);
  }
});

test("every recall answer says it may apply and where a VIN can be checked", async () => {
  const { client } = withShop();
  const answers = [
    await client.recalls("2023_hyundai_kona"),
    await client.vinRecalls(KONA),
    await client.recall("nhtsa", "19V888000"),
    await client.searchRecalls({ limit: 3 }),
  ];
  for (const answer of answers) {
    assert.equal(answer.may_apply, true);
    assert.equal(answer.lookup.nhtsa, "https://www.nhtsa.gov/recalls");
    assert.match(answer.lookup.tc, /^https:\/\/wwwapps\.tc\.gc\.ca\//);
    // The maker's own lookup is the hosted API's alone.
    assert.equal(answer.lookup.maker, null);
  }
  assert.equal(answers[0]?.attribution[0], "Contains information licensed under the Open Government Licence - Canada.");
});

test("with no shop store every recall method is dataset_unavailable, and nothing else changes", async () => {
  const store = new RecordedStore();
  const client = offline(store);
  const calls: [string, () => Promise<unknown>][] = [
    ["recalls_us", () => client.recalls("2023_hyundai_kona")],
    ["recalls_ca", () => client.recalls("2023_hyundai_kona", { country: "ca" })],
    ["recalls_us", () => client.vinRecalls(KONA)],
    ["recalls_ca", () => client.recall("tc", "2023101")],
    ["recalls_us", () => client.searchRecalls()],
  ];
  for (const [dataset, call] of calls) {
    const error = await failure(call());
    assert.equal(error.code, "dataset_unavailable");
    assert.equal(error.message, `The ${dataset} dataset is not available.`);
    assert.deepEqual(error.details, { dataset });
    assert.equal(error.status, undefined);
  }
  // Only the data file was opened: the refusals read nothing.
  assert.equal(store.steps, 2);
  assert.deepEqual((await client.meta()).datasets, {});
  assert.deepEqual({ ok: await client.decodeVin(KONA) }, caseNamed("decode").answer);
});

test("the shop file is opened when a question first needs it, once", async () => {
  const { client, store, shop } = withShop();
  await client.decodeVin(KONA);
  await client.years();
  assert.equal(shop.steps, 0, "a decode does not read the shop file");
  const meta = await client.meta();
  assert.equal(shop.steps, 1, "one read of the shop file's meta table");
  assert.equal(meta.datasets["recalls_us"]?.as_of, "2026-10-11");
  const before = store.steps + shop.steps;
  const [first, second] = await Promise.all([client.recalls("2023_hyundai_kona"), client.recalls("2023_hyundai_kona")]);
  assert.deepEqual(first, second);
  assert.ok(store.steps + shop.steps > before);
  // Not opened a second time: the meta statement was run once.
  assert.equal(shop.statements.filter((statement) => statement.sql === "SELECT key, value FROM meta").length, 1);
});

test("a shop file that does not go with the data file is data_invalid for recalls, and a VIN still decodes", async () => {
  const { client, shop } = withShop();
  shop.alter = (rows, statement) =>
    statement.sql === "SELECT key, value FROM meta"
      ? rows.map((row) => (row[0] === "catalog_data_version" ? ["catalog_data_version", "2026.08"] : row))
      : rows;
  const error = await failure(client.recalls("2023_hyundai_kona"));
  assert.equal(error.code, "data_invalid");
  assert.match(error.message, /matched against data version 2026\.08, but the data file is version 2026\.09/);
  assert.equal((await failure(client.meta())).code, "data_invalid");
  assert.deepEqual({ ok: await client.decodeVin(KONA) }, caseNamed("decode").answer);
  // Put right, the next question opens it.
  shop.alter = (rows) => rows;
  assert.equal((await client.recalls("2023_hyundai_kona")).recalls.length, 3);
});

test("a shop store that fails is store_error for the question that needed it, and is tried again", async () => {
  const { client, shop } = withShop();
  const query = shop.query.bind(shop);
  let failing = true;
  shop.query = (statements) => {
    if (failing) throw new Error("the disk went away");
    return query(statements);
  };
  const error = await failure(client.searchRecalls({ limit: 3 }));
  assert.equal(error.code, "store_error");
  assert.deepEqual({ ok: await client.decodeVin(KONA) }, caseNamed("decode").answer);
  failing = false;
  assert.equal((await client.searchRecalls({ limit: 3 })).recalls.length, 3);
});

// ----- one vehicle in one answer, with the shop file and without -----

test("with no shop store an overview answers, its recalls are null, and no second database is asked for", async () => {
  const store = new RecordedStore();
  const client = offline(store);
  const overview = await client.overview("2023_hyundai_kona");
  assert.equal(overview.vehicle?.id, "2023_hyundai_kona");
  assert.equal(overview.decode, null);
  assert.equal(overview.recalls, null);
  assert.equal(overview.safety, null);
  assert.equal(overview.links.page, "https://open.wenmarpro.com/makes/hyundai/kona/2023");
  assert.equal(overview.links.maker, null);
  assert.deepEqual(overview.attribution, []);
  const byVin = await client.vinOverview(KONA);
  assert.equal(byVin.decode?.vin, KONA);
  assert.equal(byVin.vehicle?.id, "2023_hyundai_kona");
  assert.equal(byVin.recalls, null);
  const years = await client.modelYears("hyundai", "kona");
  assert.equal(years.may_apply, true);
  assert.deepEqual(years.as_of, { us: null, ca: null });
  assert.deepEqual(
    years.years.map((year) => [year.id, year.recalls]),
    [
      ["2023_hyundai_kona", null],
      ["2022_hyundai_kona", null],
    ],
  );
  assert.ok(store.statements.every((statement) => statement.db === undefined));
  // The recent list is recalls and nothing else: it needs the shop file.
  const error = await failure(client.recentRecalls({ since: "2023-03-05" }));
  assert.equal(error.code, "dataset_unavailable");
  assert.deepEqual(error.details, { dataset: "recalls_us" });
});

test("with a shop store an overview counts the recalls that may apply, and says no more", async () => {
  const { client, shop } = withShop();
  const overview = await client.vinOverview(KONA);
  assert.ok(shop.steps > 0, "the shop file was read");
  const recalls = overview.recalls;
  assert.ok(recalls !== null && recalls !== undefined);
  assert.equal(recalls.may_apply, true);
  assert.deepEqual(recalls.count, { us: 2, ca: 1 });
  assert.equal(recalls.do_not_drive, true);
  assert.equal(recalls.park_outside, true);
  assert.deepEqual(
    recalls.newest.map((recall) => recall.campaign),
    ["24V456000", "2023101", "23V123000"],
  );
  assert.equal(recalls.lookup.nhtsa, "https://www.nhtsa.gov/recalls");
  assert.match(recalls.lookup.tc, /^https:\/\/wwwapps\.tc\.gc\.ca\//);
  // The makers' own lookup and links are the hosted API's alone.
  assert.equal(recalls.lookup.maker, null);
  assert.equal(overview.links.maker, null);
  assert.equal(recalls.url, "https://open.wenmarpro.com/makes/hyundai/kona/2023/recalls");
  // The newest are the first of the whole list.
  assert.deepEqual(recalls.newest, (await client.recalls("2023_hyundai_kona")).recalls);
  assert.deepEqual(overview.attribution, ["Contains information licensed under the Open Government Licence - Canada."]);

  const years = await client.modelYears("hyundai", "kona");
  assert.deepEqual(
    years.years.map((year) => [year.id, year.recalls]),
    [
      ["2023_hyundai_kona", { us: 2, ca: 1 }],
      ["2022_hyundai_kona", { us: 0, ca: 0 }],
    ],
  );
  assert.equal(years.lookup.maker, null);

  const recent = await client.recentRecalls({ since: "2019-01-01", make: "chevy", country: "us" });
  assert.equal(recent.may_apply, true);
  assert.deepEqual(
    recent.recalls.map((entry) => [entry.recall.campaign, entry.vehicles_listed]),
    [["19V888000", 2]],
  );
  assert.equal(recent.recalls[0]?.vehicles[0]?.vehicle_id, "2019_chevrolet_silverado");
  assert.equal(recent.recalls[0]?.vehicles[1]?.vehicle_id, null);
});

test("an overview opens the shop file once, and a shop file that cannot be used fails it and never a decode", async () => {
  const { client, shop } = withShop();
  await client.overview("2019_honda_civic");
  await client.modelYears("hyundai", "kona");
  assert.equal(shop.statements.filter((statement) => statement.sql === "SELECT key, value FROM meta").length, 1);

  const other = withShop();
  other.shop.alter = (rows, statement) =>
    statement.sql === "SELECT key, value FROM meta"
      ? rows.map((row) => (row[0] === "catalog_data_version" ? ["catalog_data_version", "2026.08"] : row))
      : rows;
  const error = await failure(other.client.overview("2023_hyundai_kona"));
  assert.equal(error.code, "data_invalid");
  assert.deepEqual({ ok: await other.client.decodeVin(KONA) }, caseNamed("decode").answer);
});

// ----- crash test ratings, from the shop file -----

test("safety gives every rating NHTSA gave a model year, and none is picked", async () => {
  const { client, store, shop } = withShop();
  const safety = await client.safety("2023_hyundai_kona");
  assert.equal(safety.vehicle.id, "2023_hyundai_kona");
  assert.equal(safety.as_of, "2026-08-07");
  assert.equal(safety.source, "NHTSA New Car Assessment Program");
  assert.equal(safety.source_url, "https://www.nhtsa.gov/ratings");
  assert.deepEqual(safety.attribution, []);
  assert.deepEqual(
    safety.ratings.map((rating) => rating.description),
    ["KONA, SUV, AWD", "KONA, SUV, FWD", "KONA ELECTRIC, SUV, FWD"],
  );
  const [first, , electric] = safety.ratings;
  assert.ok(first !== undefined && electric !== undefined);
  assert.equal(first.match, "exact");
  assert.equal(first.stars.overall, 5);
  assert.equal(first.rollover_risk_percent, 14.8);
  assert.equal(first.curb_weight_kg, 1463);
  assert.equal(first.features.forward_collision_warning, "standard");
  // A vehicle NHTSA lists and has not tested: every star is null.
  assert.ok(Object.values(electric.stars).every((stars) => stars === null));
  // The ratings were read from the shop file's store, and nothing else was.
  assert.ok(shop.statements.some((statement) => statement.sql.includes("FROM rating_vehicle")));
  assert.ok(store.statements.every((statement) => !statement.sql.includes("rating")));
  // A model year NHTSA lists nothing for is an empty list, not an error.
  assert.deepEqual((await client.safety("2019_honda_civic")).ratings, []);
  assert.equal((await failure(client.safety("2019_honda_nothing"))).code, "not_found");
});

test("with no shop store safety is dataset_unavailable, and nothing is read for it", async () => {
  const store = new RecordedStore();
  const client = offline(store);
  const error = await failure(client.safety("2023_hyundai_kona"));
  assert.equal(error.code, "dataset_unavailable");
  assert.equal(error.message, "The ratings dataset is not available.");
  assert.deepEqual(error.details, { dataset: "ratings" });
  assert.equal(error.status, undefined);
  // Only the data file was opened.
  assert.equal(store.steps, 2);
  assert.deepEqual({ ok: await client.decodeVin(KONA) }, caseNamed("decode").answer);
});

test("an overview has the crash test ratings when the shop file holds them", async () => {
  const { client } = withShop();
  const overview = await client.overview("2023_hyundai_kona_se");
  const safety = await client.safety("2023_hyundai_kona");
  assert.deepEqual(overview.safety, safety);
  assert.equal(safety.ratings.length, 3);
  assert.deepEqual((await client.vinOverview(KONA)).safety, safety);
  // With no shop store the block is null, as `recalls` is.
  assert.equal((await offline().overview("2023_hyundai_kona")).safety, null);
});

test("close closes both stores", () => {
  const closed: string[] = [];
  const client = new WenmarOpenOffline({
    store: { query: () => [], close: () => closed.push("data") },
    shop: { query: () => [], close: () => closed.push("shop") },
  });
  client.close();
  assert.deepEqual(closed, ["data", "shop"]);
});
