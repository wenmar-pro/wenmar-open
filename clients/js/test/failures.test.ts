// What the client does when the answer is not the one the description
// promises: an error from the API, an answer from something else, no answer.
import assert from "node:assert/strict";
import { after, before, test } from "node:test";

import { WenmarOpen, WenmarOpenError } from "wenmar-open";

import { errorBody, exampleResponse } from "./support/openapi.js";
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

/** Runs `call`, which must fail with a `WenmarOpenError`, and returns it. */
async function failure(call: () => Promise<unknown>): Promise<WenmarOpenError> {
  try {
    await call();
  } catch (error) {
    assert.ok(error instanceof WenmarOpenError, `not a WenmarOpenError: ${String(error)}`);
    assert.ok(error instanceof Error);
    assert.equal(error.name, "WenmarOpenError");
    return error;
  }
  assert.fail("the call did not fail");
}

test("an API error carries its code, message, details and status", async () => {
  api.next({
    status: 400,
    body: errorBody("invalid_vin", "a VIN has 17 characters, this has 16", {
      suggestions: ["KM8K2CAB4PU001140"],
      invalid_characters: [{ position: 12, character: "O" }],
    }),
  });
  const error = await failure(() => client.decodeVin("KM8K2CAB4PUO0114"));
  assert.equal(error.code, "invalid_vin");
  assert.equal(error.message, "a VIN has 17 characters, this has 16");
  assert.equal(error.status, 400);
  assert.deepEqual(error.details["suggestions"], ["KM8K2CAB4PU001140"]);
  assert.equal(error.retryAfter, undefined);
});

test("a 404 from the API is not_found", async () => {
  api.next({ status: 404, body: errorBody("not_found", "No manufacturer is registered for ZZZ.") });
  const error = await failure(() => client.decodeVin("ZZZK2CAB4PU001140"));
  assert.equal(error.code, "not_found");
  assert.equal(error.status, 404);
  assert.deepEqual(error.details, {});
});

test("a code this version has never heard of is passed on as it is", async () => {
  api.next({ status: 451, body: errorBody("region_blocked", "Not available here.", { region: "XX" }) });
  const error = await failure(() => client.meta());
  assert.equal(error.code, "region_blocked");
  assert.equal(error.status, 451);
  assert.deepEqual(error.details, { region: "XX" });
});

// ----- Review Focus 1: a non-JSON error body from a proxy -----

test("a proxy's HTML error page is bad_response with the status, and the page is not repeated", async () => {
  const page = "<html><body><h1>502 Bad Gateway</h1><p>nginx secret-internal-host</p></body></html>";
  api.next({ status: 502, headers: { "content-type": "text/html" }, body: page });
  const error = await failure(() => client.decodeVin(KONA));
  assert.equal(error.code, "bad_response");
  assert.equal(error.status, 502);
  assert.deepEqual(error.details, {});
  assert.doesNotMatch(error.message, /nginx|secret-internal-host|<html>/);
});

test("an empty error body is bad_response", async () => {
  api.next({ status: 504, body: "" });
  const error = await failure(() => client.meta());
  assert.equal(error.code, "bad_response");
  assert.equal(error.status, 504);
});

test("JSON of another shape with an error status is bad_response", async () => {
  api.next({ status: 500, body: { message: "Internal Server Error" } });
  assert.equal((await failure(() => client.meta())).code, "bad_response");
  api.next({ status: 500, body: { error: "a string, not an object" } });
  assert.equal((await failure(() => client.meta())).code, "bad_response");
  api.next({ status: 500, body: "null" });
  assert.equal((await failure(() => client.meta())).code, "bad_response");
});

test("a page answered with status 200 is bad_response, not a decode", async () => {
  api.next({ status: 200, headers: { "content-type": "text/html" }, body: "<html>Sign in to the Wi-Fi</html>" });
  const error = await failure(() => client.decodeVin(KONA));
  assert.equal(error.code, "bad_response");
  assert.equal(error.status, 200);
});

// ----- Review Focus 2: a timeout -----

test("a server that never answers is a timeout, after the time set", async () => {
  api.next({ hang: true });
  const started = Date.now();
  const error = await failure(() => client.decodeVin(KONA, { timeoutMs: 50 }));
  assert.equal(error.code, "timeout");
  assert.equal(error.status, undefined);
  assert.deepEqual(error.details, { timeout_ms: 50 });
  assert.ok(Date.now() - started < 2_000, "the timeout took far longer than it was set to");
});

test("the time limit covers the body: headers and then silence is a timeout", async () => {
  api.next({ stall: true, body: exampleResponse("meta") });
  const slow = new WenmarOpen({ baseUrl: api.url, timeoutMs: 50 });
  assert.equal((await failure(() => slow.meta())).code, "timeout");
});

test("the caller's signal aborts a request in flight, and one already aborted sends nothing", async () => {
  api.next({ hang: true });
  const controller = new AbortController();
  const pending = failure(() => client.meta({ signal: controller.signal }));
  setTimeout(() => controller.abort(), 20);
  const error = await pending;
  assert.equal(error.code, "aborted");

  const before = api.received.length;
  const already = await failure(() => client.meta({ signal: AbortSignal.abort() }));
  assert.equal(already.code, "aborted");
  assert.equal(api.received.length, before);
});

test("an address nothing listens on is a network error with its cause", async () => {
  const closed = await ApiServer.start();
  const url = closed.url;
  await closed.close();
  const nowhere = new WenmarOpen({ baseUrl: url });
  const error = await failure(() => nowhere.meta());
  assert.equal(error.code, "network");
  assert.equal(error.status, undefined);
  assert.ok(error.cause !== undefined);
});

test("no timer and no listener is left behind by a request", async () => {
  const controller = new AbortController();
  let listeners = 0;
  const signal = controller.signal;
  const add = signal.addEventListener.bind(signal);
  const remove = signal.removeEventListener.bind(signal);
  signal.addEventListener = ((...args: Parameters<typeof add>) => {
    listeners += 1;
    add(...args);
  }) as typeof signal.addEventListener;
  signal.removeEventListener = ((...args: Parameters<typeof remove>) => {
    listeners -= 1;
    remove(...args);
  }) as typeof signal.removeEventListener;
  await client.meta({ signal });
  api.next({ status: 404, body: errorBody("not_found", "nothing") });
  await failure(() => client.meta({ signal }));
  assert.equal(listeners, 0);
});

// ----- Review Focus 3: a 429 with and without Retry-After -----

test("a 429 exposes Retry-After in seconds", async () => {
  api.next({
    status: 429,
    headers: { "retry-after": "17" },
    body: errorBody("rate_limited", "Too many requests from this address. The limit is per minute.", {
      retry_after: 17,
    }),
  });
  const error = await failure(() => client.decodeVin(KONA));
  assert.equal(error.code, "rate_limited");
  assert.equal(error.status, 429);
  assert.equal(error.retryAfter, 17);
});

test("a 429 whose header a proxy dropped falls back to the body's retry_after", async () => {
  api.next({
    status: 429,
    body: errorBody("rate_limited", "Too many requests from this address.", { retry_after: 9 }),
  });
  assert.equal((await failure(() => client.meta())).retryAfter, 9);
});

test("a 429 that says nothing about when to retry has no retryAfter", async () => {
  api.next({ status: 429, body: errorBody("rate_limited", "Too many requests from this address.") });
  const error = await failure(() => client.meta());
  assert.equal(error.code, "rate_limited");
  assert.equal(error.retryAfter, undefined);
});

test("a 429 from a proxy, with a date and no API body, still exposes Retry-After", async () => {
  const date = new Date(Date.now() + 30_000).toUTCString();
  api.next({ status: 429, headers: { "retry-after": date, "content-type": "text/plain" }, body: "slow down" });
  const error = await failure(() => client.meta());
  assert.equal(error.code, "bad_response");
  assert.equal(error.status, 429);
  assert.ok(error.retryAfter !== undefined && error.retryAfter >= 28 && error.retryAfter <= 31, String(error.retryAfter));
});

test("a Retry-After that is neither seconds nor a date is ignored", async () => {
  api.next({ status: 503, headers: { "retry-after": "soon" }, body: errorBody("unavailable", "The server is busy.") });
  const error = await failure(() => client.meta());
  assert.equal(error.code, "unavailable");
  assert.equal(error.retryAfter, undefined);
});

test("a 503 from the API exposes its Retry-After too", async () => {
  api.next({ status: 503, headers: { "retry-after": "1" }, body: errorBody("unavailable", "The server is busy.") });
  assert.equal((await failure(() => client.meta())).retryAfter, 1);
});

// ----- Review Focus 4: a VIN with characters that need URL-encoding -----

test("a VIN with spaces, dashes, slashes and query characters stays one path segment", async () => {
  await client.decodeVin(" km8k2-cab4 pu001140 ");
  assert.equal(api.last.url, "/v1/vin/km8k2-cab4%20pu001140");
  await client.decodeVin("KM8/K2?year=1990#x&y=%41é");
  assert.equal(api.last.url, "/v1/vin/KM8%2FK2%3Fyear%3D1990%23x%26y%3D%2541%C3%A9");
  await client.vehicle("2019_honda_civic/../../meta");
  assert.equal(api.last.url, "/v1/vehicles/2019_honda_civic%2F..%2F..%2Fmeta");
});

test("a VIN or id that a URL would treat as a direction is refused without a request", async () => {
  const before = api.received.length;
  for (const input of ["", "   ", ".", ".."]) {
    const error = await failure(async () => client.decodeVin(input));
    assert.equal(error.code, "validation_failed");
    assert.deepEqual(error.details, { field: "vin" });
    assert.equal((await failure(async () => client.vehicle(input))).details["field"], "id");
  }
  assert.equal(api.received.length, before);
});

test("query values are encoded, whatever they hold", async () => {
  await client.search({ q: "chevy 1500 & more=1 #2 é" });
  assert.equal(api.last.url, "/v1/vehicles/search?q=chevy+1500+%26+more%3D1+%232+%C3%A9");
  assert.equal(new URL(api.last.url, api.url).searchParams.get("q"), "chevy 1500 & more=1 #2 é");
});

// ----- Review Focus 5: fields the types do not know -----

test("fields and codes added to the API after this version pass through untouched", async () => {
  const body = {
    ...(exampleResponse("decode") as Record<string, unknown>),
    recalls: [{ id: "24V-123" }],
    engine: { label: "2.0L", horsepower: 147 },
    warnings: [{ code: "a_code_added_later", message: "Something new.", severity: "low" }],
  };
  api.next({ body });
  const decode = await client.decodeVin(KONA);
  assert.equal(decode.vin, KONA);
  assert.equal(decode.warnings[0]?.code, "a_code_added_later");
  assert.deepEqual((decode as Record<string, unknown>)["recalls"], [{ id: "24V-123" }]);
  // Compared last: deepEqual narrows the type of what it is given.
  assert.deepEqual(decode, body);
});

test("a decode with only the required fields is returned as it is", async () => {
  const body = {
    vin: KONA,
    valid: false,
    check_digit: { valid: false, expected: "4", actual: "0" },
    manufacturer: { wmi: "KM8", name: "Hyundai Motor Co" },
    plant: { code: "U" },
    warnings: [],
  };
  api.next({ body });
  const decode = await client.decodeVin(KONA);
  assert.equal(decode.year, undefined);
  assert.equal(decode.catalog, undefined);
  assert.deepEqual(decode, body);
});
