# Wenmar Open

Free vehicle data for auto repair shops, starting with VIN decoding. No API key, no signup.

Wenmar Open is built and hosted by [Wenmar Pro](https://wenmarpro.com), shop management software for independent auto repair shops.

> **Status: pre-release.** This repository is being set up. Nothing is published or hosted yet, and everything below describes what is being built, not what you can use today. Watch the repo or check [CHANGELOG.md](CHANGELOG.md) for the first release.

## What it will be

- **A website** at `open.wenmarpro.com` where a service advisor or tech can paste a VIN and get the vehicle back.
- **A JSON API** at `open.wenmarpro.com/v1`, open to any origin, with no key and no account.
- **A Rust crate**, `wenmar-vin`, for decoding VINs in-process and offline.
- **An npm package**, a small typed client for the hosted API.

VIN decoding and a year/make/model/trim/engine catalog come first. More shop data follows; see the [roadmap](#roadmap).

## Why another VIN decoder

Most decoders are built for car listings. This one is built for the service counter:

- It keeps the safety-equipment fields that NHTSA publishes (ABS, TPMS type, airbags, driver-assistance features). Those decide whether a job needs sensor service or a calibration.
- It tells you when a VIN is mistyped, not just that it is invalid.
- It is Canadian-first where the data allows, and treats US and Canadian vehicles as equals.

## Planned API

The shape below is the current design and may change before the first release.

```
GET  /v1/vin/{vin}            Decode one VIN
POST /v1/vin/batch            Decode a list of VINs
GET  /v1/vehicles/years       Years
GET  /v1/vehicles/makes       Makes, optionally for a year
GET  /v1/vehicles/models      Models for a make
GET  /v1/vehicles/trims       Trims for a model
GET  /v1/vehicles/engines     Engines for a model
GET  /v1/openapi.json         OpenAPI description of the API
```

Responses are plain JSON with no wrapper object. Each response reports the version of the data it was decoded from.

## Repository layout

Planned. None of these exist yet.

| Path | What it is |
|---|---|
| `crates/wenmar-vin` | Decoder library: VIN parsing, check digit, model year, pattern matching |
| `crates/open-data` | Builds the SQLite data file from NHTSA's vPIC release |
| `crates/open-server` | The website and JSON API |
| `clients/js` | npm client for the hosted API |

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

## Acknowledgements

The decoding approach is informed by [Corgi](https://github.com/cardog-ai/corgi), an ISC-licensed TypeScript VIN decoder by Cardog. See [NOTICE.md](NOTICE.md).

## License

Code is [MIT](LICENSE). Third-party notices and data terms are in [NOTICE.md](NOTICE.md).
