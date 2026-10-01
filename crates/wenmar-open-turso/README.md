# wenmar-open-turso

Reads a [Wenmar Open](https://open.wenmarpro.com) data file through the [`turso`](https://crates.io/crates/turso) crate: VIN decoding and the vehicle catalog, in your own process, with no network.

It is the data access of the Wenmar Open server, as a library. Use it when your program already uses `turso` and tokio. If it does not, `wenmar-vin` and `wenmar-vehicles` with their `sqlite` feature do the same through `rusqlite`.

## Use

```toml
[dependencies]
wenmar-open-turso = "0.1"
wenmar-vin = "0.1"
wenmar-vehicles = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust,ignore
use std::path::Path;

use wenmar_open_turso::{Db, DecodeFailure};
use wenmar_vehicles::Scope;
use wenmar_vin::{DecodeError, DecodeOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Once, at startup. The second argument is the number of connections.
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

- **The file is only read.** It is opened with turso's `read_only` setting. Nothing is written to it, no journal, WAL or lock file is created beside it, and a write is refused.
- **It takes no lock.** Other programs, and other handles in the same program, can read the same file at the same time.
- **It does not block the async runtime.** Every query runs on tokio's blocking thread pool. `Db::open` must be called inside a tokio runtime.

One thing to know: turso does not honour SQLite's file locks. Never rewrite a data file in place while a program has it open. Write the new file beside it, rename it over the old one, and open it again (or restart).

## turso versions

This crate asks for `turso` 0.8 with no features, so your program's own choice of features decides how turso is built, and it is built once. A program on another minor version of turso needs the release of this crate made for it.

## License

MIT. Vehicle data comes from NHTSA's vPIC; its terms are in the repository's `NOTICE.md`.
