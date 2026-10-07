# Wenmar Open

Free vehicle data and shop calculators for auto repair shops, starting with VIN decoding. No API key, no signup.

Wenmar Open is built and hosted by [Wenmar Pro](https://wenmarpro.com), shop management software for independent auto repair shops.

> **Status: early.** The website and the API are live at [open.wenmarpro.com](https://open.wenmarpro.com). The crates and the npm client are published at `0.1.0`. The npm package's offline mode and the `wenmar-open-data` package are not released yet; see [CHANGELOG.md](CHANGELOG.md).

## What it will be

- **A website** at `open.wenmarpro.com` where a service advisor or tech can paste a VIN and get the vehicle back, and where a shop owner can use free calculators: a parts markup matrix, a labor rate, gross profit, and Canadian invoice tax and tire fees.
- **A JSON API** at `open.wenmarpro.com/v1`, open to any origin, with no key and no account.
- **A Rust crate**, `wenmar-vin`, for decoding VINs in-process and offline.
- **An npm package**, [`wenmar-open`](clients/js/README.md): a small typed client for the hosted API, and an offline mode, `wenmar-open/offline`, that runs the same decoder as WebAssembly over the data file. The data file is its own package, [`wenmar-open-data`](clients/data/README.md).

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

- `GET /v1/vin/{vin}` needs no key and no header: an assistant that can fetch an address can decode a VIN.
- `/mcp` is a Model Context Protocol endpoint (Streamable HTTP, no key) with two tools, `wenmar_vin` and `wenmar_vehicles`. It answers protocol revision 2026-07-28 and the revisions that open with `initialize`. The site's own address, `https://open.wenmarpro.com`, answers MCP messages too, so a connector given the bare domain works.
- `/llms.txt` describes the service for language models, and `/llms-full.txt` is the documentation as one Markdown file.
- `/.well-known/api-catalog` points to the OpenAPI description (RFC 9727).
- `robots.txt` welcomes AI crawlers on everything except single VINs.
- `server.json` describes the MCP endpoint to the MCP Registry.

What to do by hand so that search engines and assistants find a deployed copy: [docs/ai-discovery.md](docs/ai-discovery.md).

## Using it from an application

- From JavaScript or TypeScript: the [`wenmar-open`](clients/js/README.md) npm package. The hosted client has no dependencies and runs in Node 20 and later, browsers, Cloudflare Workers, Deno and Bun. Its offline mode needs the data file: from [`wenmar-open-data`](clients/data/README.md) in Node, or imported into Cloudflare D1 for a Worker.
- From Rust, with no network: `wenmar-vin` and `wenmar-vehicles` read a data file through `rusqlite`, and [`wenmar-open-turso`](crates/wenmar-open-turso/README.md) reads one through `turso` from async code.
- Replacing calls to NHTSA's hosted API, or a job that copies makes and models: [docs/integrating.md](docs/integrating.md).

Releases are described in [docs/releasing.md](docs/releasing.md).

## Running the server

```bash
mise run data     # once: download vPIC and build the data file
mise run serve    # http://localhost:3000
curl http://localhost:3000/v1/vin/1HGCM82633A004352
```

Open `http://localhost:3000` for the website: a VIN box, a year and make picker, reference pages for every make, model year and manufacturer code, and five short guides to reading a VIN. The pages are rendered on the server and work with JavaScript turned off. They load nothing from any other host: the two typefaces, DM Sans and JetBrains Mono, are served from the site itself. Every reference page has a Markdown version at the same address with `.md` added, such as `/makes/honda/civic/2019.md`.

Without mise: `OPEN_DATA=data/build/wenmar-open-2026.09.sqlite3 cargo run -p open-server`. The server opens the data file read-only and stores nothing. Settings and the deploy procedure are in [docs/deploy.md](docs/deploy.md).

## Development scripts

`bin/` has a short script for each common task, so a contributor need not remember the mise task names. Run any of them from anywhere in the checkout.

| Script | What it does |
|---|---|
| `bin/setup` | Prepare a fresh checkout: the pinned Rust toolchain, its components and the workspace's dependencies, then a build. The one script that does not need mise. |
| `bin/dev` | Run the website and the API on http://localhost:3000 |
| `bin/tui` | Open the one-screen terminal interface |
| `bin/data` | Download vPIC and build the data file |
| `bin/check` | Format check, lints and tests: what CI runs |
| `bin/parity`, `bin/catalog-parity` | Compare the data and the catalog with NHTSA's answers |
| `bin/js` | Build and test the npm client |
| `bin/release-check` | Check that a release would work |
| `bin/doctor` | Check whether the data file and the API answer |

Each is a thin wrapper over the matching `mise run` task, and `mise run` remains the way to pass a task options.

## Repository layout

`crates/wenmar-vin`, `crates/wenmar-vehicles`, `crates/wenmar-open-turso`, `crates/open-data`, `crates/wenmar-open-cli`, `crates/open-mcp`, `crates/open-server`, `crates/wenmar-open-wasm`, `crates/shop-math`, `clients/js` and `clients/data` exist so far. The rest is planned.

| Path | What it is |
|---|---|
| `crates/wenmar-vin` | Decoder library: VIN parsing, check digit, model year, pattern matching |
| `crates/wenmar-vehicles` | Catalog library: years, makes, models, submodels, engines, search, stable vehicle ids |
| `crates/open-data` | Builds the SQLite data file from NHTSA's vPIC release |
| `crates/wenmar-open-cli` | The `wenmar-open` command-line tool |
| `crates/open-mcp` | The MCP tool definitions the server and the command-line tool share |
| `crates/open-server` | The website, the JSON API and the MCP endpoint, served from one read-only data file |
| `crates/wenmar-open-turso` | The data file read through `turso`: what the server uses, as a library for other async Rust programs |
| `crates/wenmar-open-wasm` | The decoder and the catalog behind one JSON call, built as WebAssembly for the npm package's offline mode. Not published as a crate |
| `crates/shop-math` | The arithmetic of the shop calculators: money, percentages, gross profit targets, the parts matrix, the labor rate, the gross profit check and the taxes and tire fees on a Canadian invoice. Not published as a crate |
| `clients/js` | The `wenmar-open` npm package: a typed client for the hosted API, and `wenmar-open/offline` |
| `clients/data` | The `wenmar-open-data` npm package: the data file, and a script that writes it as SQL for Cloudflare D1 |

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

Free-text search finds more online than offline. The data file reads the text as a year, a make, a model and a submodel, in that order. The hosted API also finds a model from its own words in any order, so `vehicles search type r` finds the Civic Type R with `--online` and nothing from the data file, where `vehicles search civic type r` finds it.

Piped, the output is JSON; at a terminal it is text. `--json` prints JSON at a terminal too, and `--jq EXPR` filters it with a jq expression. An error is JSON on standard error, in the API's shape, with a non-zero exit code.

For AI agents: `wenmar-open mcp` is an MCP server on standard input and output with two tools, `wenmar_vin` and `wenmar_vehicles`. `wenmar-open setup claude` or `wenmar-open setup codex` writes a skill file that describes the commands and prints the command that registers the MCP server. `wenmar-open doctor` says whether the data file and the API can answer.

Run with no command at a terminal, `wenmar-open` opens a one-screen interface: type a VIN or a vehicle such as `2019 civic si`, press Enter, and read the answer. Esc clears the line, and Esc again leaves.

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
