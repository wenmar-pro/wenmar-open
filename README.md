# Wenmar Open

Free vehicle data for auto repair shops, starting with VIN decoding. No API key, no signup.

Wenmar Open is built and hosted by [Wenmar Pro](https://wenmarpro.com), shop management software for independent auto repair shops.

> **Status: pre-release.** The decoder, the data build, the API and the website exist and run locally. Nothing is published or hosted yet. Watch the repo or check [CHANGELOG.md](CHANGELOG.md) for the first release.

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

One address may make 600 requests a minute. Over that the answer is `429` with a `Retry-After` header. The limit exists so that one client cannot slow the service for everyone. For the same reason an address (path and query string) may be 8 KB and a request's headers 32 KB, and when the server is too busy to answer the answer is `503` with a `Retry-After` header.

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

Open `http://localhost:3000` for the website: a VIN box, a year and make picker, and reference pages for every make, model year and manufacturer code. The pages are rendered on the server and work with JavaScript turned off. Every reference page has a Markdown version at the same address with `.md` added, such as `/makes/honda/civic/2019.md`.

Without mise: `OPEN_DATA=data/build/wenmar-open-2026.09.sqlite3 cargo run -p open-server`. The server opens the data file read-only and stores nothing. Settings and the deploy procedure are in [docs/deploy.md](docs/deploy.md).

## Repository layout

`crates/wenmar-vin`, `crates/wenmar-vehicles`, `crates/open-data` and `crates/open-server` exist so far. The rest is planned.

| Path | What it is |
|---|---|
| `crates/wenmar-vin` | Decoder library: VIN parsing, check digit, model year, pattern matching |
| `crates/wenmar-vehicles` | Catalog library: years, makes, models, submodels, engines, search, stable vehicle ids |
| `crates/open-data` | Builds the SQLite data file from NHTSA's vPIC release |
| `crates/wenmar-open-cli` | The `wenmar-open` command-line tool |
| `crates/open-mcp` | The MCP tool definitions the server and the command-line tool share |
| `crates/open-server` | The website, the JSON API and the MCP endpoint, served from one read-only data file |
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

## Command-line tool

`wenmar-open` does the same lookups from a terminal. It needs no key and no account.

```bash
cargo build -p wenmar-open-cli
./target/debug/wenmar-open vin decode 1HGCM82633A004352
./target/debug/wenmar-open vehicles search 2019 civic si
./target/debug/wenmar-open vehicles models --make honda --year 2019
```

It answers from a local data file when there is one and from the hosted API otherwise. `wenmar-open data pull` downloads the data file, after which nothing needs a connection. `--offline` and `--online` force one or the other, and `--api URL` or `WENMAR_OPEN_API` names another server.

Piped, the output is JSON; at a terminal it is text. `--json` prints JSON at a terminal too, and `--jq EXPR` filters it with a jq expression. An error is JSON on standard error, in the API's shape, with a non-zero exit code.

For AI agents: `wenmar-open mcp` is an MCP server on standard input and output with two tools, `wenmar_vin` and `wenmar_vehicles`. `wenmar-open setup claude` or `wenmar-open setup codex` writes a skill file that describes the commands and prints the command that registers the MCP server. `wenmar-open doctor` says whether the data file and the API can answer.

`open-data` is the maintainers' tool for building the data file. `wenmar-open` is the one for using it.

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
