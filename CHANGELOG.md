# Changelog

All notable changes to this project are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project will follow [Semantic Versioning](https://semver.org/) once it has a release.

## [Unreleased]

### Added

- Repository scaffolding: license, notices, contribution guide, security policy, and issue templates.
- `wenmar-vin` crate: VIN validation, check digit, model year, typo suggestions, vPIC pattern matching, and a decoder over a pluggable data source.
- `open-data`: builds the SQLite data file from NHTSA's vPIC plain-text dump.
- `wenmar-vin`: SQLite data source behind the `sqlite` feature; pattern selection and model-year rules now follow NHTSA's decoder.
- `open-data parity`: compares decodes with NHTSA's recorded answers for a committed corpus and fails if agreement drops.
- `wenmar-vin`: vehicles other than cars, MPVs and light trucks choose their model year the way NHTSA does; engine size is rounded to one decimal and also given in cubic centimetres.
- `mise.toml` with pinned tools and `check`, `data` and `parity` tasks.
- Decodes now include what NHTSA records per trim: transmission, ABS, ESC, TPMS, driver assistance, wheel sizes, seats, weight rating and base price.
- Data file schema version 2. Files built before this must be rebuilt.
- Vehicle catalog in the data file: years, makes, models, submodels and engines for every vPIC model year, with the eighth VIN character for an engine where the data settles it. Built from the vPIC release with no API calls.
- `wenmar-vehicles` crate: reads the catalog, searches it by free text such as `2019 civic si`, gives every vehicle a stable id, and finds the catalog entry for a decoded VIN.
- `open-data catalog` to look things up in a data file, and `open-data catalog-parity` to compare its model lists with NHTSA's recorded answers.
- `data/catalog/makes.yaml`, `data/catalog/names.yaml` and `data/presets.yaml`: popular makes and their aliases, spelling fixes, and hand-written trims.
- A monthly workflow that builds the data file and publishes it as a `data-YYYY.MM` release.
- Data file schema version 3. Files built before this must be rebuilt.
- `open-server`: the JSON API under `/v1`. VIN decoding (single and batches of 50) with the catalog entry each VIN reaches, the vehicle catalog step by step, free-text search, vehicles by id, and the data version. It reads the data file read-only and stores nothing.
- An OpenAPI description generated from the server's code at `/v1/openapi.json`, with a committed copy at `crates/open-server/openapi.json`.
- A keyless MCP endpoint at `/mcp` with two tools, `wenmar_vin` and `wenmar_vehicles`, and `/llms.txt`.
- A limit of 600 requests a minute for one address, answered with `429` and `Retry-After`.
- `mise run serve`, a `Dockerfile`, Kamal configuration and a deploy runbook in `docs/deploy.md`. Nothing is deployed yet.
- `open-server`: free-text searches use at most half of the connections to the data file, so they cannot keep VIN decodes waiting.
- `wenmar-vehicles`: free-text search no longer reads every model year for each statement. On the 2026.09 data file a search through the server takes 1 to 8 ms in a release build, where it took up to 530 ms. Results and their order are unchanged, and so is the data file.
- `open-server`: limits on what a client may send and hold open. An address over 8 KB is answered `414` with the new error code `uri_too_long`; a request head over 32 KB is refused; at most 512 requests are answered at once and 1,024 connections kept open; a connection that does not send its request head within 120 seconds is closed.
- `open-server`: request logs cut every part of an address that the caller chose to 11 characters, so a VIN typed at the wrong address is not logged whole either. The runbook says what the hosting proxy's own log records.
- The website, served by `open-server`: a home page with a VIN box and a year and make picker, a result page for a VIN, and reference pages for makes, model years and manufacturer codes, with docs, data and about pages. Pages are rendered on the server and work without JavaScript.
- A Markdown version of every reference page at the same address with `.md` added, listed in `/llms.txt`.
- `robots.txt` and sitemaps for the reference pages. Result pages for single VINs are never indexed, and neither are the pages of makes and model years that are not cars, MPVs or trucks.
- `open-server`: every `ETag`, and the address of the stylesheet and the script, names the build as well as the data version, so a deploy that changes a page reaches a visitor who holds the old one.
- `wenmar-open` command-line tool: `vin decode`, `vehicles years|makes|models|submodels|engines|search|entry`, JSON when piped and text at a terminal, `--json` and `--jq`, and the API's error codes on standard error with an exit code for each kind of failure.
- `wenmar-open data pull` and `data status`: the published data file, downloaded and checked before it replaces the one in place. Without a data file the tool asks the hosted API; `--offline` and `--online` force either.
- `wenmar-open mcp`: an MCP server on standard input and output with the hosted endpoint's two tools. `wenmar-open setup claude|codex` installs a skill file and prints the command that registers the server. `wenmar-open doctor` checks the data file and the API.
- `wenmar-open` with no command opens a one-screen terminal interface.
- `open-mcp` crate: the MCP tool definitions shared by the server and the command-line tool.
- `clients/js`: the `wenmar-open` npm package, a typed client for the hosted API. No dependencies; ES modules; Node 20 and later, browsers and edge runtimes. One error class with the API's code, message, details and status, a request timeout, an abort signal, and `Retry-After` on a `429`. Its types are generated from the OpenAPI description, and a test fails when they are out of date.
- `wenmar-open-turso` crate: opens a data file read-only through `turso`, decodes a VIN and reads the catalog from async code without blocking the runtime. It is the server's own data access, moved into a library.
- `open-server` reads the data file through `wenmar-open-turso`. Its behaviour is unchanged.
- One version for every crate and the npm package, set in `Cargo.toml` and `clients/js/package.json`. `wenmar-vin`, `wenmar-vehicles` and `wenmar-open-turso` can be published to crates.io.
- `mise run js` builds and tests the npm client. `mise run release-check` checks that the versions agree and the changelog has an entry, and runs `cargo publish --dry-run` and `npm pack --dry-run`.
- A release workflow: a `v*` tag on `main` publishes the three crates and the npm package through trusted publishing, with no stored token, and creates a GitHub release. `docs/releasing.md` is the runbook.
- Each release has the `wenmar-open` command-line tool built for macOS (arm64, x86_64) and Linux (x86_64, arm64).
- `docs/integrating.md`: how an application replaces calls to NHTSA's hosted API and a make and model sync job.
- Data releases are no longer marked as the repository's latest release, so `releases/latest` is always a release of the code.
