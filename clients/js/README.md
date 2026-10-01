# wenmar-open

A typed client for the [Wenmar Open](https://open.wenmarpro.com) API: VIN decoding and a year, make, model, trim and engine catalog for auto repair shops. The API needs no key and no account.

- No dependencies. It uses the platform's `fetch`.
- Runs in Node 20 and later, browsers, Cloudflare Workers, Deno and Bun.
- Types are generated from the API's OpenAPI description.

```bash
npm install wenmar-open
```

## Decode a VIN

```ts
import { WenmarOpen } from "wenmar-open";

const open = new WenmarOpen();

const vehicle = await open.decodeVin("KM8K2CAB4PU001140");
console.log(vehicle.year, vehicle.make, vehicle.model, vehicle.trim);
// 2023 Hyundai Kona SE
console.log(vehicle.engine?.label, vehicle.drivetrain, vehicle.safety?.tpms);
```

Spaces, dashes and lowercase letters in the VIN are accepted. A field the data does not have is left out, so every field except `vin`, `valid`, `check_digit`, `manufacturer`, `plant` and `warnings` may be `undefined`.

A wrong check digit is not an error. The decode comes back with `valid: false` and a warning, because many genuine VINs from outside North America fail the check:

```ts
import { WenmarOpen } from "wenmar-open";

const vehicle = await new WenmarOpen().decodeVin("KM8K2CAB0PU001140");
if (!vehicle.valid) {
  for (const warning of vehicle.warnings) {
    console.log(warning.code, warning.message, warning.suggestions);
  }
}
```

If you know the model year, pass it: `open.decodeVin(vin, { year: 2023 })`.

## Errors

Every method throws a `WenmarOpenError`. It has the API's `code`, `message` and `details`, and the HTTP `status`.

```ts
import { WenmarOpen, WenmarOpenError } from "wenmar-open";

const open = new WenmarOpen();

try {
  await open.decodeVin("KM8K2CAB4PUO01140"); // a letter O where a zero belongs
} catch (error) {
  if (!(error instanceof WenmarOpenError)) throw error;
  if (error.code === "invalid_vin") {
    console.log(error.message); // what is wrong with it
    console.log(error.details.suggestions); // VINs you may have meant
  } else if (error.code === "not_found") {
    console.log("No manufacturer is registered for this VIN.");
  } else if (error.code === "rate_limited" || error.code === "unavailable") {
    console.log(`Try again in ${error.retryAfter ?? 60} seconds.`);
  } else {
    throw error;
  }
}
```

| `code` | `status` | Meaning |
|---|---|---|
| `invalid_vin` | 400 | Wrong length or characters. `details.suggestions` lists likely corrections. |
| `validation_failed` | 400 | A parameter is missing or wrong. `details.field` names it. |
| `not_found` | 404 | No manufacturer for the VIN, or no vehicle with that id. |
| `rate_limited` | 429 | Over 600 requests a minute from one address. `retryAfter` is in seconds. |
| `unavailable` | 503 | The server is busy. `retryAfter` is in seconds. |
| `internal_error` | 500 | Something went wrong on the server. |
| `timeout` | none | No complete answer within `timeoutMs`. |
| `aborted` | none | Your `signal` was aborted. |
| `network` | none | The request could not be sent. `cause` has the reason. |
| `bad_response` | any | Something answered, but not the API: a proxy's error page, a sign-in page. |

The API may add codes. Treat a code you do not know as a failure.

The client does not retry. If you retry, do it only for `rate_limited`, `unavailable`, `timeout` and `network`, and wait `retryAfter` seconds when it is set.

## Decode many VINs

```ts
import { WenmarOpen, isBatchError } from "wenmar-open";

const open = new WenmarOpen();
const vins = ["KM8K2CAB4PU001140", "1HGCM82633A004352", "NOT-A-VIN"];

const items = await open.decodeVins(vins);
items.forEach((item, index) => {
  if (isBatchError(item)) {
    console.log(vins[index], "failed:", item.error.code);
  } else {
    console.log(item.vin, item.year, item.make, item.model);
  }
});
```

A batch holds at most 50 VINs (`MAX_BATCH`). The answer has one item per VIN, in the order sent. One bad VIN does not fail the batch.

## Choose a vehicle without a VIN

Each step offers only what is valid for the steps before it.

```ts
import { WenmarOpen } from "wenmar-open";

const open = new WenmarOpen();

const years = await open.years(); // newest first
const makes = await open.makes({ year: 2019 }); // popular makes first
const models = await open.models({ make: "honda", year: 2019 });
const trims = await open.submodels({ make: "honda", model: "civic", year: 2019 });
const engines = await open.engines({ make: "honda", model: "civic", year: 2019, submodel: "si" });

console.log(years[0], makes[0]?.name, models[0]?.name, trims[0]?.name, engines[0]?.label);
```

`make` and `model` take a name, an alias or an id: `Chevrolet`, `chevy` and `chevrolet` are the same make. `term` filters any step by what someone has typed so far, for autocomplete: `open.makes({ term: "che" })`.

Free text works too, and every vehicle has an id you can store:

```ts
import { WenmarOpen } from "wenmar-open";

const open = new WenmarOpen();

const [best] = await open.search({ q: "2019 civic si" });
console.log(best?.id, best?.summary);

const same = await open.vehicle("2019_honda_civic_si");
console.log(same.summary);
```

A decoded VIN carries the same ids in `catalog`, so a VIN can fill in a vehicle form: `vehicle.catalog?.entry.id`.

## Options

```ts
import { WenmarOpen } from "wenmar-open";

const open = new WenmarOpen({
  baseUrl: "http://localhost:3000", // your own server; default https://open.wenmarpro.com
  timeoutMs: 5_000, // default 10,000; 0 for no limit
});

// Per request: a time limit, and a signal to cancel with.
const controller = new AbortController();
const makes = await open.makes({ term: "ho" }, { signal: controller.signal, timeoutMs: 2_000 });
console.log(makes.length);
```

`fetch` replaces the platform's `fetch`, for a Workers service binding or a test:

```ts
import { WenmarOpen } from "wenmar-open";
import type { FetchLike } from "wenmar-open";

const logged: FetchLike = async (url, init) => {
  console.log(init.method, url);
  return fetch(url, init);
};
const open = new WenmarOpen({ fetch: logged });
console.log((await open.meta()).data_version);
```

## Caching and limits

Answers change only when the data does, about once a month. `open.meta()` returns the `data_version`; anything you cache can be kept until it changes. In a browser the HTTP cache already does this.

One address may make 600 requests a minute. A server that calls the API for many users shares one address, so cache catalog answers there. To decode a list of VINs, use `decodeVins`: a batch of 50 counts as one request.

## CommonJS

The package is an ES module. `require("wenmar-open")` works in Node 20.19 and later and in Node 22.12 and later. In older Node 20 releases use `await import("wenmar-open")`.

## Types

Every response type is exported: `VinDecode`, `Entry`, `Make`, `Model`, `Submodel`, `EngineOption`, `Meta`, `BatchItem`, `ErrorBody`. The generated `paths`, `operations` and `components` types are exported as well.

The API only ever adds fields and endpoints. A response may carry fields this version's types do not name; they are passed through untouched.

## License

MIT. Vehicle data comes from NHTSA's vPIC and is described in the repository's [NOTICE.md](https://github.com/wenmar-pro/wenmar-open/blob/main/NOTICE.md).
