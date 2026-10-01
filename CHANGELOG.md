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
