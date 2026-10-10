# Using Wenmar Open from an application

This is for an application that decodes VINs or offers a year, make and model picker, and today calls NHTSA's hosted vPIC API to do it. It covers four ways to use Wenmar Open instead, what to cache, and the one limit.

| Way | Use it when | Needs |
|---|---|---|
| The hosted API | A browser, or a server that makes a modest number of lookups | Nothing. No key, no account. |
| The npm package, offline | A Node server, or a Cloudflare Worker with D1, that wants no network call and no limit | The data file: the `wenmar-open-data` package, 167 MB, or a D1 database |
| The Rust crates, in your process | A Rust server that wants no network call and no limit | The data file, 167 MB |
| The data file with the command-line tool | Any other language, offline | The data file and the `wenmar-open` binary |

All four give the same answers, because all four read the same data file.

## The hosted API

Base address: `https://open.wenmarpro.com`. Every route is under `/v1`, answers JSON with no wrapper object, and may be called from any web page (CORS allows every origin). The description is at `/v1/openapi.json`.

### In place of NHTSA's endpoints

| NHTSA vPIC | Wenmar Open |
|---|---|
| `DecodeVinValues/{vin}`, `DecodeVin/{vin}`, `DecodeVinValuesExtended/{vin}` | `GET /v1/vin/{vin}` |
| the `modelyear` parameter of those | `GET /v1/vin/{vin}?year=2019` |
| `DecodeVINValuesBatch` | `POST /v1/vin/batch` with `{ "vins": [...] }`, at most 50 |
| `GetModelsForMakeYear/make/{make}/modelyear/{year}` | `GET /v1/vehicles/models?make={make}&year={year}` |
| `GetMakesForVehicleType/{type}` | `GET /v1/vehicles/makes`, which lists cars, multipurpose vehicles and trucks unless `scope` says otherwise |
| nothing | `GET /v1/vehicles/years`, `/v1/vehicles/submodels`, `/v1/vehicles/engines`, `/v1/vehicles/search?q=`, `/v1/vehicles/{id}` |

### What is different in the answers

| | NHTSA | Wenmar Open |
|---|---|---|
| Shape | `{ "Count", "Message", "Results": [ { ... } ] }` | The object itself |
| A field with no value | An empty string | Left out |
| Numbers | Strings: `"ModelYear": "2019"` | Numbers: `"year": 2019` |
| Make | Upper case: `HONDA` | As written: `Honda` |
| A VIN that cannot be decoded | Status 200 with `ErrorCode` and `ErrorText` | Status 400 or 404 with `{ "error": { "code", "message", "details" } }` |
| A wrong check digit | An error code among others | Status 200 with `"valid": false` and a warning; the decode is still there |
| A mistyped VIN | No help | `details.suggestions` lists VINs that were probably meant |

The fields most applications read:

| NHTSA | Wenmar Open |
|---|---|
| `ModelYear` | `year` |
| `Make`, `Model`, `Series`, `Trim` | `make`, `model`, `series`, `trim` |
| `BodyClass` | `body` |
| `DriveType` | `drivetrain` |
| `TransmissionStyle`, `TransmissionSpeeds` | `transmission`, `transmission_speeds` |
| `DisplacementL`, `EngineCylinders`, `FuelTypePrimary`, `EngineModel` | `engine.displacement_l`, `engine.cylinders`, `engine.fuel`, `engine.model`; `engine.label` is a short name such as `2.0L Turbo` |
| `Manufacturer`, `PlantCity`, `PlantCountry` | `manufacturer.name`, `plant.city`, `plant.country` |
| `ABS`, `ESC`, `TPMS` and the driver-assistance fields | under `safety` |

A decode also has `catalog`: the entry of the vehicle catalog the VIN reaches, with ids for the year, make and model, the trim, and the engine. An application with a vehicle form can fill the form from it.

### From a browser

Before:

```js
const response = await fetch(`https://vpic.nhtsa.dot.gov/api/vehicles/DecodeVinValues/${vin}?format=json`);
const result = (await response.json()).Results?.[0];
if (!result || result.ErrorCode !== "0") {
  showMessage("VIN not recognised");
} else {
  fillForm(result.ModelYear, result.Make, result.Model, result.Trim);
}
```

After, with no library:

```js
const response = await fetch(`https://open.wenmarpro.com/v1/vin/${encodeURIComponent(vin)}`);
const body = await response.json();
if (!response.ok) {
  showMessage(body.error.message, body.error.details.suggestions);
} else {
  fillForm(body.year, body.make, body.model, body.trim);
}
```

After, with the npm package, which adds types, a timeout and one error class:

```js
import { WenmarOpen, WenmarOpenError } from "wenmar-open";

const open = new WenmarOpen({ timeoutMs: 5000 });
try {
  const vehicle = await open.decodeVin(vin);
  fillForm(vehicle.year, vehicle.make, vehicle.model, vehicle.trim);
} catch (error) {
  if (!(error instanceof WenmarOpenError)) throw error;
  showMessage(error.message, error.details.suggestions);
}
```

Two things to change besides the call:

- **Content Security Policy.** If the page sends a `connect-src` directive, add `https://open.wenmarpro.com` to it and remove `https://vpic.nhtsa.dot.gov`.
- **Make and model names.** They arrive in display case. Code that turned NHTSA's upper case into display case is no longer needed; code that compares against stored upper-case values needs to compare without regard to case.

A decode is a convenience. When the request fails, for any reason, let the person type the vehicle in.

### In place of a job that copies makes and models

A common arrangement is a monthly job that asks NHTSA for every make, then for the models of each make in each year, and stores the result in a table that autocomplete reads. That is thousands of requests, and the table is as fresh as the last run.

The catalog routes answer the same questions directly, one step at a time, each limited to what is valid for the steps before it:

```text
GET /v1/vehicles/years
GET /v1/vehicles/makes?year=2019
GET /v1/vehicles/models?make=honda&year=2019
GET /v1/vehicles/submodels?make=honda&model=civic&year=2019
GET /v1/vehicles/engines?make=honda&model=civic&year=2019&submodel=si
```

Every step takes `term`, the text typed so far, so an autocomplete box can call the route as it is. `make` and `model` accept a name, an alias or an id: `Chevrolet`, `chevy` and `chevrolet` are the same make.

To switch:

1. Point the picker at these routes, from the browser or through your own server.
2. If your own records add to the suggestions (vehicles your users have entered that the catalog lacks), keep that part: ask the catalog, then add your own values to the list.
3. Stop the job. Keep its table until you are sure nothing else reads it.

If you would rather keep a local table, fill it once a month from the data file (below) instead of by crawling.

**What to store.** Store the names you show, and the vehicle id (`catalog.entry.id` from a decode, or `id` from a search). An id such as `2019_honda_civic_si` is made of the year and the names, and stays the same from one data release to the next for as long as the names do.

## Caching

An answer changes only when the data does, about once a month.

- Every successful `GET` under `/v1` is sent with `Cache-Control: public, max-age=86400` and an `ETag`. A browser caches it without any code. Errors are sent with `no-store`.
- Every answer carries the data version in an `X-Data-Version` header, and `GET /v1/meta` returns it as `data_version`.
- On a server, cache by request address. Either keep entries for a day, or keep them until the data version changes: read `/v1/meta` once an hour and drop the cache when `data_version` is new.
- A decode of a VIN is safe to store with your record of the vehicle. It will not change for that data version.
- Cache `invalid_vin` and `not_found` for as long as a success. Do not cache `rate_limited`, `unavailable`, or any failure to reach the API.

## The limit

One address may make 600 requests a minute. Over that the answer is `429` with the code `rate_limited` and a `Retry-After` header, in seconds. The limit exists so that one client cannot slow the service for everyone; ordinary use does not come near it.

- A server that calls the API on behalf of its users is one address. Cache catalog answers, as above.
- A batch of 50 VINs is one request. Decode a list with `POST /v1/vin/batch`, not with 50 requests.
- When the server is too busy to answer, the answer is `503` with the code `unavailable` and a `Retry-After` header.
- Retry only `429`, `503`, timeouts and connection failures, and wait for `Retry-After` when it is given. Never retry a `400` or a `404`: the answer will not change.
- If your volume cannot fit, run the decoder in your own process, or run your own copy of the server. Both use the same data file and have no limit.

## The npm package, offline

`wenmar-open/offline` has the methods of the hosted client and answers them from the data file, with the decoder the server runs, compiled to WebAssembly. Nothing leaves the process.

```bash
npm install wenmar-open wenmar-open-data
```

```js
import { openOffline } from "wenmar-open/offline/node";

const open = await openOffline();
const vehicle = await open.decodeVin("KM8K2CAB4PU001140");
```

That needs Node 22.16 or later. For Node 20 with `better-sqlite3`, for Cloudflare Workers with D1, and for what differs from the hosted client (free-text search finds less; there is no HTTP status), see the [package's README](../clients/js/README.md#offline).

Update the data by updating the package: `npm update wenmar-open-data` each month. Its major version is the data file's schema version, so an update never brings a file your version of `wenmar-open` cannot read.

## The Rust crates, in your process

Three crates are on crates.io.

**`wenmar-vin`** decodes. Its pure parts need no data and bring two dependencies, `serde` and `thiserror`, so they fit anywhere, a phone or a browser included:

```rust
use wenmar_vin::{Vin, check_digit, suggest};

let vin = Vin::parse("1HGCM8Z633A004352")?;      // length and characters
let check = check_digit::check(&vin);             // position 9
if !check.valid {
    let meant = suggest::for_check_digit(&vin);   // likely corrections
}
```

Describing the vehicle needs the data file.

**With `rusqlite`** (add the `sqlite` feature to both crates):

```rust
use wenmar_vehicles::sqlite::SqliteSource;
use wenmar_vehicles::{Catalog, Scope};
use wenmar_vin::sqlite::SqliteData;
use wenmar_vin::{DecodeOptions, Decoder};

let decoder = Decoder::new(SqliteData::open("wenmar-open-2026.09.sqlite3")?);
let decoded = decoder.decode("KM8K2CAB4PU001140", DecodeOptions::default())?;

let catalog = Catalog::new(SqliteSource::open("wenmar-open-2026.09.sqlite3")?)?;
let makes = catalog.makes(Some(2019), Scope::Light, "", 50)?;
let entry = catalog.selection(&decoded)?;        // the catalog entry the VIN reaches
```

These calls block. In an async server, run them on a blocking thread.

**From async code**, in a tokio program, use `wenmar-open-db`. It does the blocking-thread work for you:

```rust
use std::path::Path;
use wenmar_open_db::Db;
use wenmar_vehicles::Scope;
use wenmar_vin::DecodeOptions;

let db = Db::open(Path::new("wenmar-open-2026.09.sqlite3"), 4).await?;
let decode = db.decode_vin("KM8K2CAB4PU001140", DecodeOptions::default()).await?;
let makes = db.catalog(|catalog| catalog.makes(Some(2019), Scope::Light, "", 50)).await??;
```

`decode` serializes to the same JSON as `GET /v1/vin/{vin}`. The file is opened read-only: nothing is written to it or beside it. SQLite's read locks are honoured, and they let any number of readers share one file, so several programs can read it at the same time.

## The data file

Each month's file is a GitHub release of [wenmar-pro/wenmar-open](https://github.com/wenmar-pro/wenmar-open/releases) tagged `data-YYYY.MM`, with two assets: `wenmar-open-<version>.sqlite3.gz` and its `.sha256`.

```bash
version=2026.09
base=https://github.com/wenmar-pro/wenmar-open/releases/download/data-$version
curl -fsSLO "$base/wenmar-open-$version.sqlite3.gz"
curl -fsSLO "$base/wenmar-open-$version.sqlite3.gz.sha256"
sha256sum -c "wenmar-open-$version.sqlite3.gz.sha256"
gunzip "wenmar-open-$version.sqlite3.gz"
```

- It is a plain SQLite file. Open it read-only.
- **Replacing it.** Never write a new file over one a program has open. Put the new file beside the old one, rename it over the old one, and have the program open it again (or restart it). A program that built the file into its container image gets the new one with its next deploy.
- **Schema version.** The file's `meta` table has a `schema_version`. A version of the crates reads one schema version and refuses any other, with a message that names both. The changelog says when it changes; take the new crates and the new file together.
- The tables are this project's own and can change with the schema version. Read them through the crates or the command-line tool, not with your own SQL.

**From another language**, the `wenmar-open` command-line tool reads the same file and prints JSON:

```bash
wenmar-open data pull
wenmar-open --offline vin decode KM8K2CAB4PU001140
```

`data pull` downloads the newest data release and puts it in place safely. A program can run the tool and read its output, which is the API's JSON.

## Running your own server

The server is one binary and one data file, with no database to administer and no state: see `docs/deploy.md`. Point the npm client at it with `new WenmarOpen({ baseUrl: "https://vehicles.example.com" })`.
