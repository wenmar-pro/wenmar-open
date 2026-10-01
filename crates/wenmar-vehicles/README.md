# wenmar-vehicles

Reads and searches the vehicle catalog in a [Wenmar Open](https://open.wenmarpro.com) data file: years, makes, models, submodels and engines.

Not published yet. The catalog is built by `open-data build` from NHTSA's vPIC release; what it covers and what it cannot is described in the repository's `data/catalog/README.md`.

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
