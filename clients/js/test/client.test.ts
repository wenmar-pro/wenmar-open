import assert from "node:assert/strict";
import { after, before, test } from "node:test";

import { DEFAULT_BASE_URL, MAX_BATCH, WenmarOpen, WenmarOpenError, isBatchError } from "wenmar-open";

import { errorBody, exampleResponse, routes } from "./support/openapi.js";
import { ApiServer } from "./support/server.js";

let api: ApiServer;
let client: WenmarOpen;

before(async () => {
  api = await ApiServer.start();
  client = new WenmarOpen({ baseUrl: api.url });
});

after(async () => {
  await api.close();
});

const KONA = "KM8K2CAB4PU001140";

/**
 * One call for every operation of the description, by its operationId, with
 * the request it must make.
 */
const calls: Record<string, { call: () => Promise<unknown>; method: string; url: string }> = {
  decode: {
    call: () => client.decodeVin(KONA, { year: 2023 }),
    method: "GET",
    url: `/v1/vin/${KONA}?year=2023`,
  },
  batch: {
    call: () => client.decodeVins([KONA, "1HGCM82633A004352"]),
    method: "POST",
    url: "/v1/vin/batch",
  },
  years: {
    call: () => client.years({ scope: "all", term: "201" }),
    method: "GET",
    url: "/v1/vehicles/years?scope=all&term=201",
  },
  makes: {
    call: () => client.makes({ year: 2019, term: "mer", limit: 5 }),
    method: "GET",
    url: "/v1/vehicles/makes?year=2019&term=mer&limit=5",
  },
  models: {
    call: () => client.models({ make: "honda", year: 2019 }),
    method: "GET",
    url: "/v1/vehicles/models?make=honda&year=2019",
  },
  submodels: {
    call: () => client.submodels({ make: "honda", model: "civic", year: 2019 }),
    method: "GET",
    url: "/v1/vehicles/submodels?make=honda&model=civic&year=2019",
  },
  trims: {
    call: () => client.trims({ make: "honda", model: "civic", year: 2019 }),
    method: "GET",
    url: "/v1/vehicles/trims?make=honda&model=civic&year=2019",
  },
  engines: {
    call: () => client.engines({ make: "honda", model: "civic", year: 2019, submodel: "si" }),
    method: "GET",
    url: "/v1/vehicles/engines?make=honda&model=civic&year=2019&submodel=si",
  },
  search: {
    call: () => client.search({ q: "2019 civic si", limit: 3 }),
    method: "GET",
    url: "/v1/vehicles/search?q=2019+civic+si&limit=3",
  },
  entry: {
    call: () => client.vehicle("2019_honda_civic_si"),
    method: "GET",
    url: "/v1/vehicles/2019_honda_civic_si",
  },
  meta: {
    call: () => client.meta(),
    method: "GET",
    url: "/v1/meta",
  },
};

test("there is a method for every operation of the description, and no other", () => {
  assert.deepEqual(Object.keys(calls).sort(), routes.map((route) => route.operationId).sort());
});

for (const [operationId, expected] of Object.entries(calls)) {
  test(`${operationId}: asks ${expected.method} ${expected.url} and returns the documented body`, async () => {
    const answer = await expected.call();
    assert.equal(api.last.method, expected.method);
    assert.equal(api.last.url, expected.url);
    assert.deepEqual(answer, exampleResponse(operationId));
  });
}

test("a decode is typed: the documented fields are there by name", async () => {
  const decode = await client.decodeVin(KONA);
  assert.equal(decode.vin, KONA);
  assert.equal(decode.make, "Hyundai");
  assert.equal(decode.year, 2023);
  assert.equal(decode.check_digit.expected, "4");
  assert.equal(decode.manufacturer.wmi, "KM8");
  assert.equal(decode.catalog?.vehicle_id, "2023_hyundai_kona");
});

test("a batch is sent as { vins } in JSON and keeps its order", async () => {
  api.next({
    body: [
      exampleResponse("decode"),
      errorBody("invalid_vin", "a VIN has 17 characters, this has 3", { suggestions: [] }),
    ],
  });
  const items = await client.decodeVins([KONA, "ABC"]);
  assert.equal(api.last.headers["content-type"], "application/json");
  assert.deepEqual(JSON.parse(api.last.body), { vins: [KONA, "ABC"] });
  const [first, second] = items;
  assert.ok(first !== undefined && second !== undefined);
  assert.equal(isBatchError(first), false);
  assert.ok(isBatchError(second));
  assert.equal(second.error.code, "invalid_vin");
});

test("a batch over the limit is refused without a request", async () => {
  const before = api.received.length;
  const vins = Array.from({ length: MAX_BATCH + 1 }, () => KONA);
  await assert.rejects(client.decodeVins(vins), (error: unknown) => {
    assert.ok(error instanceof WenmarOpenError);
    assert.equal(error.code, "validation_failed");
    assert.equal(error.status, undefined);
    assert.deepEqual(error.details, { field: "vins", max: 50, received: 51 });
    return true;
  });
  assert.equal(api.received.length, before);
});

test("blank and missing query values are left out", async () => {
  await client.makes({ term: "", scope: "light" });
  assert.equal(api.last.url, "/v1/vehicles/makes?scope=light");
  await client.years();
  assert.equal(api.last.url, "/v1/vehicles/years");
  await client.decodeVin(KONA);
  assert.equal(api.last.url, `/v1/vin/${KONA}`);
});

test("the base URL is the hosted API by default, and a path and trailing slashes are handled", async () => {
  assert.equal(new WenmarOpen().baseUrl, DEFAULT_BASE_URL);
  assert.equal(DEFAULT_BASE_URL, "https://open.wenmarpro.com");
  const prefixed = new WenmarOpen({ baseUrl: `${api.url}/open//` });
  assert.equal(prefixed.baseUrl, `${api.url}/open`);
  api.next({ body: exampleResponse("meta") });
  await prefixed.meta();
  assert.equal(api.last.url, "/open/v1/meta");
});

test("a request says nothing about its sender", async () => {
  await client.decodeVin(KONA);
  const names = Object.keys(api.last.headers).sort();
  // What the platform's fetch adds on its own, and Accept. No key, no
  // cookie, no client name or version, no identifier.
  const allowed = new Set([
    "accept",
    "accept-encoding",
    "accept-language",
    "connection",
    "host",
    "sec-fetch-mode",
    "user-agent",
  ]);
  assert.deepEqual(
    names.filter((name) => !allowed.has(name)),
    [],
  );
  assert.equal(api.last.headers.accept, "application/json");
  assert.doesNotMatch(String(api.last.headers["user-agent"]), /wenmar/i);
});

test("a custom fetch is used, and is called as a plain function", async () => {
  const seen: string[] = [];
  const custom = new WenmarOpen({
    baseUrl: "https://vehicles.example",
    fetch: async function (this: unknown, url, init) {
      assert.equal(this, undefined);
      seen.push(`${init.method} ${url}`);
      return {
        status: 200,
        headers: { get: () => null },
        text: async () => JSON.stringify(exampleResponse("meta")),
      };
    },
  });
  const meta = await custom.meta();
  assert.deepEqual(seen, ["GET https://vehicles.example/v1/meta"]);
  assert.equal(meta.data_version, "2026.09");
});
