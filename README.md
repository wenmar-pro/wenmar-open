# Wenmar Open

Free vehicle data for auto repair shops, starting with VIN decoding. No API key, no signup.

Wenmar Open is built and hosted by [Wenmar Pro](https://wenmarpro.com), shop management software for independent auto repair shops.

> **Status: pre-release.** The decoder, the data build and the API server exist and run locally. Nothing is published or hosted yet. Watch the repo or check [CHANGELOG.md](CHANGELOG.md) for the first release.

## What it will be

- **A website** at `open.wenmarpro.com` where a service advisor or tech can paste a VIN and get the vehicle back.
- **A JSON API** at `open.wenmarpro.com/v1`, open to any origin, with no key and no account.
- **A Rust crate**, `wenmar-vin`, for decoding VINs in-process and offline.
- **An npm package**, a small typed client for the hosted API, with an optional offline mode that decodes locally.

VIN decoding and a year/make/model/trim/engine catalog come first. More shop data follows; see the [roadmap](#roadmap).

## Why another VIN decoder

Most decoders are built for car listings. This one is built for the service counter:

- It returns the equipment NHTSA records for each trim: ABS, ESC, TPMS type, air bags, driver assistance, wheel sizes and seat count. Those decide whether a job needs sensor service or a calibration.
- It tells you when a VIN is mistyped, not just that it is invalid.
- It is Canadian-first where the data allows, and treats US and Canadian vehicles as equals.

## API

Every route is under `/v1`. Responses are plain JSON with no wrapper object, any website may call them from a browser, and every response carries the data version in an `X-Data-Version` header.

```
GET  /v1/vin/{vin}               Decode one VIN (?year= to override the model year)
POST /v1/vin/batch               Decode up to 50 VINs: { "vins": [...] }
GET  /v1/vehicles/years          Model years, newest first
GET  /v1/vehicles/makes          Makes, optionally for a year
GET  /v1/vehicles/models         Models of a make
GET  /v1/vehicles/submodels      Trims of a model year (also served as /v1/vehicles/trims)
GET  /v1/vehicles/engines        Engines of a model year
GET  /v1/vehicles/search         Free text: ?q=2019+civic+si
GET  /v1/vehicles/{id}           One vehicle by its id, such as 2019_honda_civic_si
GET  /v1/meta                    Data version, vPIC release, build time
GET  /v1/openapi.json            OpenAPI description, generated from the code
```

An error is `{ "error": { "code", "message", "details" } }` with a fitting HTTP status. A VIN with a wrong check digit is not an error: the decode has `valid: false` and a warning.

One address may make 600 requests a minute. Over that the answer is `429` with a `Retry-After` header. The limit exists so that one client cannot slow the service for everyone.

Fields and endpoints are only ever added.

### For AI agents

- `/mcp` is a Model Context Protocol endpoint (Streamable HTTP, no key) with two tools: `wenmar_vin` and `wenmar_vehicles`.
- `/llms.txt` describes the service for language models.

## Running the server

```bash
mise run data     # once: download vPIC and build the data file
mise run serve    # http://localhost:3000
curl http://localhost:3000/v1/vin/1HGCM82633A004352
```

Without mise: `OPEN_DATA=data/build/wenmar-open-2026.09.sqlite3 cargo run -p open-server`. The server opens the data file read-only and stores nothing. Settings and the deploy procedure are in [docs/deploy.md](docs/deploy.md).

## Repository layout

`crates/wenmar-vin`, `crates/wenmar-vehicles`, `crates/open-data` and `crates/open-server` exist so far. The rest is planned.

| Path | What it is |
|---|---|
| `crates/wenmar-vin` | Decoder library: VIN parsing, check digit, model year, pattern matching |
| `crates/wenmar-vehicles` | Catalog library: years, makes, models, submodels, engines, search, stable vehicle ids |
| `crates/open-data` | Builds the SQLite data file from NHTSA's vPIC release |
| `crates/open-server` | The JSON API and the MCP endpoint, served from one read-only data file |
| `clients/js` | npm client for the hosted API |

## Building the data file

```bash
cargo run --release -p open-data -- fetch
cargo run --release -p open-data -- build --dump data/build/vPICList_lite_2026_09.sql --out data/build/wenmar-open-2026.09.sqlite3
cargo run --release -p open-data -- decode --data data/build/wenmar-open-2026.09.sqlite3 1HGCM82633A004352
```

`fetch` downloads NHTSA's newest vPIC release (about 73 MB) and prints the path of the extracted file to pass to `build`. No database server is needed.

With [mise](https://mise.jdx.dev) installed, `mise run data` does the fetch and build in one step, `mise run parity` runs the comparison below, and `mise run check` runs what CI runs.

## Checking against NHTSA

```bash
cargo run --release -p open-data -- parity --data data/build/wenmar-open-2026.09.sqlite3
```

Decodes a corpus of VINs and compares each field with NHTSA's own recorded answers. See [data/corpus/README.md](data/corpus/README.md). `mise run catalog-parity` does the same for the catalog's model lists.

## The vehicle catalog

The data file also holds a catalog for choosing a vehicle without a VIN: year, make, model, submodel, engine. It comes from the same NHTSA release.

```bash
cargo run --release -p open-data -- catalog --data data/build/wenmar-open-2026.09.sqlite3 search 2019 civic si
```

What it covers, what it cannot, and how it is checked against NHTSA's own model lists: [data/catalog/README.md](data/catalog/README.md).

## Data releases

A workflow builds the data file each month from NHTSA's newest release, runs both checks against NHTSA's recorded answers, and attaches the file to a GitHub release tagged `data-YYYY.MM`. A build that fails a check is not released. Models NHTSA added to the newest model years after the answers were recorded are noted in the release and do not fail it; see [data/catalog/README.md](data/catalog/README.md).

## Data sources

Vehicle data comes from the [NHTSA Product Information Catalog and Vehicle Listing (vPIC)](https://vpic.nhtsa.dot.gov/), published by the US National Highway Traffic Safety Administration.

vPIC describes vehicles as manufacturers reported them to NHTSA. It can be incomplete or wrong, especially for vehicles never sold in the United States. If a decode is wrong, please [report it](../../issues/new/choose).

## Roadmap

1. VIN decoding and the vehicle catalog: website, API, Rust crate, npm client.
2. Safety recalls from NHTSA and Transport Canada.
3. Camera VIN scanning on the website, and more coverage of Canada-only vehicles.
4. Further shop reference data as open sources allow.
5. Labor times.

Labor times depend on finding a source that can be redistributed freely. The standard commercial labor guides are licensed products and cannot be.

## Contributing

Corrections to decodes are the most useful contribution. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Code is [MIT](LICENSE). Third-party notices and data terms are in [NOTICE.md](NOTICE.md).
