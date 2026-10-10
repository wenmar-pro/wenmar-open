# wenmar-open-db

Reads a [Wenmar Open](https://open.wenmarpro.com) data file through the [`rusqlite`](https://crates.io/crates/rusqlite) crate: VIN decoding and the vehicle catalog, in your own process, with no network.

It is the data access of the Wenmar Open server, as a library. Use it when your program already uses tokio and wants one data file held open, read-only, for any number of callers. If it does not, `wenmar-vin` and `wenmar-vehicles` with their `sqlite` feature read the same data file without async around it.

## Use

```toml
[dependencies]
wenmar-open-db = "0.2"
wenmar-vin = "0.2"
wenmar-vehicles = "0.2"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust,ignore
use std::path::Path;

use wenmar_open_db::{Db, DecodeFailure};
use wenmar_vehicles::Scope;
use wenmar_vin::{DecodeError, DecodeOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Once, at startup. The second argument counts pool slots; each slot
    // holds two read-only connections, one for the catalog and one for
    // raw queries.
    let db = Db::open(Path::new("wenmar-open-2026.09.sqlite3"), 4).await?;
    println!("data {}", db.meta().data_version);

    match db.decode_vin("KM8K2CAB4PU001140", DecodeOptions::default()).await {
        Ok(decode) => {
            println!("{:?} {:?} {:?}", decode.decoded.year, decode.decoded.make, decode.decoded.model);
            if let Some(catalog) = &decode.catalog {
                println!("{}", catalog.entry.summary);
            }
        }
        Err(DecodeFailure::Decode(DecodeError::InvalidVin { error, suggestions })) => {
            println!("{error}; did you mean {suggestions:?}?");
        }
        Err(DecodeFailure::Decode(DecodeError::UnknownManufacturer { wmi })) => {
            println!("no manufacturer is registered for {wmi}");
        }
        Err(other) => return Err(other.into()),
    }

    // The catalog is read inside a closure, which runs off the async runtime.
    let makes = db
        .catalog(|catalog| catalog.makes(Some(2019), Scope::Light, "", 50))
        .await??;
    println!("{} makes", makes.len());
    Ok(())
}
```

`Decode` serializes to the same JSON as the hosted API's `GET /v1/vin/{vin}`.

## The data file

Data files are published monthly as `data-YYYY.MM` releases of [wenmar-pro/wenmar-open](https://github.com/wenmar-pro/wenmar-open/releases), as `wenmar-open-<version>.sqlite3.gz`. Unpack it with `gunzip`. It is a plain SQLite file of about 160 MB.

A version of this crate reads data files of one schema version, and `Db::open` refuses any other with a message that names both. The changelog says when the schema version changes.

## What it guarantees

- **The file is only read.** It is opened read-only. Nothing is written to it, no journal, WAL or lock file is created beside it, and a write is refused.
- **It takes no lock worth naming.** SQLite's own read locks are honoured, and they let any number of readers share the file, so other programs, and other handles in the same program, can read it at the same time.
- **It does not block the async runtime.** Every query runs on tokio's blocking thread pool, on a connection of its own. `Db::open` must be called inside a tokio runtime.
- **The second argument of `Db::open` counts pool slots, not connections.** Each slot holds two read-only connections — one for the catalog, one for raw queries — plus one more briefly while the file's `meta` table is read, so `n` slots is `2n` connections at rest. All of them map the same file pages, one shared OS mapping, not a copy each; where the platform or filesystem cannot map the file, each connection falls back to its own SQLite page cache (about 2 MiB by default), and that is what a larger slot count really costs.
- **The data file may be memory-mapped.** `Db::open` hints to SQLite that the whole file may be mapped, which is only a hint: where the platform or filesystem cannot, reading goes on through the page cache as before.

One thing to know: SQLite honours file locks, but a reader keeps reading the inode it opened. Never rewrite a data file in place while a program has it open. Write the new file beside it, rename it over the old one, and open it again (or restart): a reader that held the old file open goes on reading the old, complete contents until it reopens, instead of half of a new file.

## License

MIT. Vehicle data comes from NHTSA's vPIC; its terms are in the repository's `NOTICE.md`.
