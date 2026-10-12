# wenmar-open

A typed client for the [Wenmar Open](https://open.wenmarpro.com) API: VIN decoding, a year, make, model, trim and engine catalog, and the recall campaigns that may apply to a model year, for auto repair shops. The API needs no key and no account.

- No dependencies. It uses the platform's `fetch`.
- An [offline mode](#offline) that decodes from a data file with no network.
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
| `dataset_unavailable` | 404 | The server holds no data of that kind, such as recalls. `details.dataset` names it. |
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

## Recalls

Recall campaigns from NHTSA (United States) and Transport Canada. A campaign lists a year, a make and a model, so an answer is about a model year and never about one vehicle. Every answer has `may_apply: true`, and `lookup` gives the official addresses where a VIN can be checked. Neither government publishes recalls by VIN as data: say "may apply" where you show these, and send people to `lookup` to confirm.

```ts
import { WenmarOpen } from "wenmar-open";

const open = new WenmarOpen();

// By vehicle id, or by VIN. The VIN only names the model year.
const byId = await open.recalls("2018_ford_f-150", { country: "us" });
const byVin = await open.vinRecalls("KM8K2CAB4PU001140");

for (const recall of byVin.recalls) {
  console.log(recall.date, recall.source, recall.campaign, recall.component);
  if (recall.do_not_drive) console.log("NHTSA advises not driving it until it is repaired.");
}
console.log(`${byVin.recalls.length} campaigns may apply. Check the VIN: ${byVin.lookup.nhtsa}`);
console.log(byId.as_of.us, byId.attribution);
```

`country` is `us` or `ca`; left out, both. `as_of` is the day each country's data was current, so an empty list means none was found as of that day. `attribution` lists the credit lines the data's licences ask for: show them where you show the data. A VIN that decodes to no vehicle in the catalog is not an error: `vehicle` is `null`, the list is empty, and a warning has the code `no_catalog_vehicle`.

One campaign with every vehicle it lists, and the list of campaigns, a page at a time:

```ts
import { WenmarOpen } from "wenmar-open";

const open = new WenmarOpen();

const campaign = await open.recall("nhtsa", "19V888000");
console.log(campaign.summary, campaign.remedy, campaign.vehicles.length);

let page = await open.searchRecalls({ make: "ford", year: 2018, q: "tailgate", limit: 25 });
console.log(page.recalls.map((recall) => recall.campaign));
if (page.next_cursor) {
  page = await open.searchRecalls({ make: "ford", year: 2018, q: "tailgate", limit: 25, cursor: page.next_cursor });
}
```

A server run without recall data answers these four methods with `dataset_unavailable`, and `details.dataset` names what is missing. `open.meta()` lists what a server holds in `datasets`.

```ts
import { WenmarOpen, WenmarOpenError } from "wenmar-open";

const open = new WenmarOpen({ baseUrl: "http://localhost:3000" });

try {
  const answer = await open.recalls("2018_ford_f-150");
  console.log(answer.recalls.length, "campaigns may apply");
} catch (error) {
  if (error instanceof WenmarOpenError && error.code === "dataset_unavailable") {
    console.log(`This server does not hold ${String(error.details["dataset"])}.`); // recalls_us or recalls_ca
  } else {
    throw error;
  }
}
```

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

## Offline

`wenmar-open/offline` answers the same questions with no network. It runs the same decoder the API runs, compiled to WebAssembly and shipped inside this package, over a Wenmar Open data file: a plain SQLite database of about 167 MB. Recalls come from a second, smaller file ([below](#recalls-offline)). The methods, their answers and the error codes are those of `WenmarOpen`, with three more codes for the data ([below](#switching-between-the-api-and-the-data-file)), so code written for one works with the other.

The hosted client does not load any of this. An application that imports only `wenmar-open` gets the small client and nothing else.

### In Node

```bash
npm install wenmar-open wenmar-open-data
```

`wenmar-open-data` is the data file as an npm package. It is 49 MB to download and 167 MB on disk, and a new version is published each month.

```ts
import { WenmarOpenError } from "wenmar-open/offline";
import { openOffline } from "wenmar-open/offline/node";

const open = await openOffline();

try {
  const vehicle = await open.decodeVin("KM8K2CAB4PU001140");
  console.log(vehicle.year, vehicle.make, vehicle.model);
  console.log(await open.models({ make: "honda", year: 2019 }));
} catch (error) {
  if (error instanceof WenmarOpenError) console.error(error.code, error.message);
  else throw error;
} finally {
  open.close();
}
```

`openOffline()` uses Node's own SQLite, so it needs Node 22.16 or later. It opens the file read-only and checks it before it returns. To read a file from somewhere else, such as one downloaded with `wenmar-open data pull`, pass its path:

```ts
import { openOffline } from "wenmar-open/offline/node";

const open = await openOffline({ path: "/var/lib/wenmar-open/wenmar-open-2026.09.sqlite3" });
console.log((await open.meta()).data_version);
```

On Node 20, or to use a database you opened yourself, give the client a store. `syncStore` takes a `DatabaseSync` of `node:sqlite` or a database of `better-sqlite3`:

```js
import Database from "better-sqlite3";
import { path } from "wenmar-open-data";
import { WenmarOpenOffline, syncStore } from "wenmar-open/offline";

const database = new Database(path, { readonly: true });
const open = new WenmarOpenOffline({ store: syncStore(database) });
```

### Recalls offline

The recalls are in a second file, the shop file, published as `wenmar-open-data-shop` (17 MB to download, 77 MB on disk, a new version each week):

```bash
npm install wenmar-open wenmar-open-data wenmar-open-data-shop
```

```ts
import { openOffline } from "wenmar-open/offline/node";

const open = await openOffline();

const answer = await open.vinRecalls("KM8K2CAB4PU001140");
console.log(`${answer.recalls.length} recall campaigns may apply.`);
console.log("Check the VIN:", answer.lookup.nhtsa, answer.lookup.tc);
console.log((await open.meta()).datasets);
open.close();
```

`openOffline()` uses the shop file of `wenmar-open-data-shop` when that package is installed. Pass `{ shopPath }` to read one from somewhere else, such as the file `wenmar-open data pull --shop` downloads, or `{ shopPath: false }` to open none. The four recall methods are those of `WenmarOpen` and give the same answers, with one difference listed below. Without a shop file they fail with `dataset_unavailable`, and everything else works.

A shop file holds the ids of one data file's catalog, so it is read only with the data file it was built against: `wenmar-open-data-shop` names that exact version of `wenmar-open-data` as its peer. A pair that does not go together is refused with `data_invalid`, by `openOffline()` before it returns and otherwise by the first recall question. Update the two packages together.

With a store of your own, the shop file is a second store:

```ts
import { WenmarOpenOffline } from "wenmar-open/offline";
import type { Store } from "wenmar-open/offline";

declare const data: Store;
declare const shop: Store;

export const open = new WenmarOpenOffline({ store: data, shop });
```

Each statement goes to the store of its file; a statement for the shop file is marked `db: "shop"`, which a store may ignore. The shop file is opened when a question first needs it, so a decode never reads it.

### One vehicle in one answer, offline

`overview(id)`, `vinOverview(vin)`, `modelYears(make, model)` and `recentRecalls({ since })` are those of `WenmarOpen`:

```ts
import { openOffline } from "wenmar-open/offline/node";

const open = await openOffline();

const overview = await open.vinOverview("KM8K2CAB4PU001140");
console.log(overview.vehicle?.id, overview.links.page);
if (overview.recalls) {
  const { us, ca } = overview.recalls.count;
  console.log(`${(us ?? 0) + (ca ?? 0)} recall campaigns may apply. Check the VIN: ${overview.recalls.lookup.nhtsa}`);
} else {
  console.log("No shop file: no recalls to count.");
}

const years = await open.modelYears("hyundai", "kona");
console.log(years.years.map((year) => [year.id, year.recalls]));
open.close();
```

`overview`, `vinOverview` and `modelYears` answer with no shop file too: `recalls` is then `null`, in the overview and in each year of `modelYears`. `recentRecalls` needs the shop file, as the other recall methods do. A block the files do not give is `null`, as it is from a server that does not hold it: see the list below.

### Fuel, specifications, fluids and tires offline

`fuel`, `specs`, `fluids` and `tires`, and `vinFuel`, `vinSpecs`, `vinFluids` and `vinTires`, are those of `WenmarOpen`, read from the shop file.

`code(code)`, `codes(query)` and `vehicleCode(id, code)` are those of `WenmarOpen`: generic OBD-II trouble codes described in this project's own words, read from the shop file. Offline, `vehicleCode` gives the description and the recalls about the same components; its `bulletins` and `related.bulletins` are always `null`, because bulletins are on the hosted service only.

`jobs()`, `job(job)`, `labor(id, job?)` and `vinLabor(vin, job?)` are those of `WenmarOpen`: the job list and Wenmar's labour estimates, read from the shop file. Each figure has `hours` (`low`, `typical`, `high`), a `basis` and the `scope` it is for. An estimate, not a quote; credit it as `attribution` says: `Labour estimates by Wenmar Pro, CC BY 4.0`.

`safety(id)` is that of `WenmarOpen`: NHTSA's crash test ratings for a model year, read from the shop file.

```ts
import { openOffline } from "wenmar-open/offline/node";

const open = await openOffline();
const safety = await open.safety("2023_hyundai_kona");
for (const rating of safety.ratings) {
  console.log(rating.description, rating.stars.overall ?? "no overall rating");
}
console.log(`NHTSA's file of ${safety.as_of}: ${safety.source_url}`);
open.close();
```

NHTSA rates a vehicle by body style and drive, so `ratings` often has several entries and their stars can differ: `description` says which vehicle each is for, in NHTSA's words. Show them all, or the one whose body style and drive you know. A rating with every star `null` is a vehicle NHTSA lists and has not tested, and `ratings` is empty when NHTSA lists none. Without a shop file, or with one that holds no ratings, `safety` fails with `dataset_unavailable` and `details.dataset` is `ratings`. The answer is the hosted API's, field for field.

`complaints(id)` is that of `WenmarOpen` too: what owners reported to NHTSA about a model year, as counts by component with the median mileage they gave. These are owners' reports and are not verified, and a count is not a rate. Without a shop file that holds the complaints, it fails with `dataset_unavailable` and `details.dataset` is `complaints`.

### Switching between the API and the data file

Both clients have the same methods, so the choice can be one line:

```ts
import { WenmarOpen } from "wenmar-open";
import type { VinDecode } from "wenmar-open";
import type { WenmarOpenOffline } from "wenmar-open/offline";
import { openOffline } from "wenmar-open/offline/node";

async function client(offline: boolean): Promise<WenmarOpen | WenmarOpenOffline> {
  return offline ? openOffline() : new WenmarOpen();
}

const open = await client(true);
const vehicle: VinDecode = await open.decodeVin("KM8K2CAB4PU001140");
console.log(vehicle.catalog?.vehicle_id);
```

What differs offline:

- `error.status` is always `undefined`: there is no HTTP answer. Branch on `error.code`.
- Three more codes: `no_data` (no data file where one was looked for), `data_invalid` (the database is not a data file this version reads) and `store_error` (the database failed while it was read, including while it was first opened, so a retry may succeed; `data_invalid` is for rows that were read and are not a data file, and for a schema version this client does not read).
- `search` reads the text as a year, a make, a model and a submodel, in that order. The API also finds a model from its words in any order, with an index the data file does not have. `search({ q: "civic type r" })` finds the Civic Type R in both; `search({ q: "type r" })` finds it only through the API.
- `meta().server_version` is the version of this package.
- `meta().datasets` lists what the shop file holds, and is `{}` when no shop file is open: the data file holds the VIN decoder and the catalog only.
- `lookup.maker` in a recall answer is always `null`. The makers' own recall lookups are part of the hosted service and are in neither file. `lookup.nhtsa` and `lookup.tc` are always there.
- `links.maker` in an overview is always `null`, for the same reason. `specs`, `fuel`, `fluids` and `tires` in an overview are what `specs(id)`, `fuel(id)`, `fluids(id)` and `tires(id)` give when the shop file holds them, and `null` otherwise. `safety` is what `safety(id)` gives when the shop file holds the ratings, and `null` otherwise.
- A recall's `url` is its page on `https://open.wenmarpro.com`, whichever server you would otherwise ask. So are `links.page` and every other address in an overview or in `modelYears`.
- `decodeVins` takes no `include`: the recall count of a batch is the hosted API's alone. Ask `vinOverview` for each VIN instead.
- `timeoutMs` and `signal` are checked each time the client goes back to the database, not while the database is working. There is no time limit unless you set one.
- The current year, which bounds the model years a VIN can have, comes from the device's clock. Pass `currentYear` to set it.

### In Cloudflare Workers

A Worker cannot carry a 167 MB file. Import the data into a D1 database once (see the [`wenmar-open-data` README](https://www.npmjs.com/package/wenmar-open-data)), or use the hosted API. For recalls, import the shop file into a second D1 database (see the [`wenmar-open-data-shop` README](https://www.npmjs.com/package/wenmar-open-data-shop)) and pass it as `shop: d1Store(env.SHOP_DB)`.

```js
import { WenmarOpenOffline, d1Store } from "wenmar-open/offline";
import wasm from "wenmar-open/offline.wasm";

let open;

export default {
  async fetch(request, env) {
    open ??= new WenmarOpenOffline({ store: d1Store(env.DB), wasm });
    const vin = new URL(request.url).pathname.slice(1);
    return Response.json(await open.decodeVin(vin));
  },
};
```

Workers do not compile WebAssembly from bytes, so the module is imported as a file and passed as `wasm`. Everywhere else the option is left out.

A decode reads D1 6 or 7 times and runs 10 to 15 statements, each of which counts as one D1 query. The first question an isolate answers also reads about 12,000 rows of makes, once. `years()` reads every model-year row of the catalog, about 490,000, on every call: keep its answer.

### A store of your own

Anything that can run a `SELECT` on the data file can be a store: an object with one method, which is given a list of statements and returns the rows of each, in order, as arrays of values. It may return them at once or as a promise.

```ts
import { WenmarOpenOffline } from "wenmar-open/offline";
import type { Statement, Store } from "wenmar-open/offline";

declare function run(sql: string, params: readonly (string | number | null)[]): Promise<unknown[][]>;

const store: Store = {
  query: (statements: readonly Statement[]) => Promise.all(statements.map(({ sql, params }) => run(sql, params))),
};
export const open = new WenmarOpenOffline({ store });
```

Rows must be arrays, not objects keyed by column name: two columns of one statement may share a name. Integers may be numbers or BigInts.

### Data versions

A version of `wenmar-open` reads data files of one schema version, and `wenmar-open-data`'s major version is that schema version: `wenmar-open-data@3.202609.0` is the data of September 2026 in schema version 3. A file of another schema version is refused with `data_invalid` and a message naming both versions. The shop file has a schema version of its own, and `wenmar-open-data-shop@1.20261012.0` is the shop file of 12 October 2026 in schema version 1.

| `wenmar-open` | reads `wenmar-open-data` | reads `wenmar-open-data-shop` |
|---|---|---|
| 0.2 | 3.x | 1.x |

### Size

The decoder is 703 kB of WebAssembly (238 kB compressed). The package holds it twice, as a file and as text inside a module, so the package is 1.9 MB unpacked. None of it is loaded by `import "wenmar-open"`.

In a browser the page's content security policy must allow WebAssembly (`'wasm-unsafe-eval'`). The data file is too large to send to a browser; use the hosted API there.

## CommonJS

The package is an ES module. `require("wenmar-open")` works in Node 20.19 and later and in Node 22.12 and later. In older Node 20 releases use `await import("wenmar-open")`.

## Types

Every response type is exported: `VinDecode`, `Entry`, `Make`, `Model`, `Submodel`, `EngineOption`, `Meta`, `BatchItem`, `ErrorBody`, and for recalls `VehicleRecalls`, `VinRecalls`, `RecallCampaign`, `RecallList`, `Recall` and `RecallLookup`. So are the query types, such as `RecallSearchQuery`. The generated `paths`, `operations` and `components` types are exported as well.

The API only ever adds fields and endpoints. A response may carry fields this version's types do not name; they are passed through untouched.

## License

MIT. Vehicle data comes from NHTSA's vPIC and is described in the repository's [NOTICE.md](https://github.com/wenmar-pro/wenmar-open/blob/main/NOTICE.md).
