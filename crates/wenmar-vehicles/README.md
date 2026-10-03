# wenmar-vehicles

Reads and searches the vehicle catalog in a [Wenmar Open](https://open.wenmarpro.com) data file: years, makes, models, submodels and engines.

```toml
[dependencies]
wenmar-vehicles = { version = "0.1", features = ["sqlite"] }
```

The catalog is in a [Wenmar Open data file](https://github.com/wenmar-pro/wenmar-open/releases), built from NHTSA's vPIC release. What it covers and what it cannot is described in the repository's `data/catalog/README.md`. The `sqlite` feature reads a data file with `rusqlite`; the `wenmar-open-turso` crate reads one with `turso`.

`vin_rows::fetch` reads everything `wenmar-vin`'s decoder may ask about one VIN through a `Source`, for a database that cannot be handed to the decoder directly. `wenmar-open-turso` and the WebAssembly build both decode this way.

## Example

```rust,ignore
use wenmar_vehicles::sqlite::SqliteSource;
use wenmar_vehicles::{Catalog, Scope};

let catalog = Catalog::new(SqliteSource::open("wenmar-open-2026.09.sqlite3")?)?;
let makes = catalog.makes(Some(2019), Scope::Light, "", 50)?;
let found = catalog.search("2019 civic si", Scope::Light, 10)?;
```

Vehicle ids such as `2019_honda_civic_si` are built from the year and the names. They stay the same from one data release to the next for as long as the names do.

## License

MIT. Vehicle data terms and third-party notices are in the repository's `NOTICE.md`.
