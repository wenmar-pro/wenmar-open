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
- `open-server`: limits on what a client may send and hold open. An address over 8 KB is answered `414` with the new error code `uri_too_long`; a request head over 32 KB is refused; at most 512 requests are answered at once and 1,024 connections kept open; a connection that does not send its request head within 120 seconds is closed.
- `open-server`: request logs cut every part of an address that the caller chose to 11 characters, so a VIN typed at the wrong address is not logged whole either. The runbook says what the hosting proxy's own log records.
