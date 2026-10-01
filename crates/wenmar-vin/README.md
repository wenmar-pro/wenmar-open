# wenmar-vin

VIN decoding for auto repair shops. The decoder behind [Wenmar Open](https://open.wenmarpro.com), by [Wenmar Pro](https://wenmarpro.com).

- Validates a VIN and explains what is wrong with a bad one.
- Checks the check digit and suggests the likely typo.
- Works out the model year, including the 30-year ambiguity.
- Matches NHTSA vPIC patterns to describe the vehicle, including safety equipment.

## Install

```toml
[dependencies]
wenmar-vin = "0.1"
```

The pure parts (validation, check digit, model year, typo suggestions) need no data, and the crate then has two dependencies, `serde` and `thiserror`. Decoding a whole vehicle needs a [Wenmar Open data file](https://github.com/wenmar-pro/wenmar-open/releases): the `sqlite` feature reads one with `rusqlite`, and the `wenmar-open-turso` crate reads one with `turso`.

## Example

```rust
use wenmar_vin::{check_digit, suggest, Vin};

let vin = Vin::parse("1HGCM8Z633A004352").unwrap();
assert!(!check_digit::check(&vin).valid);
assert_eq!(suggest::for_check_digit(&vin), vec!["1HGCM82633A004352"]);
```

Decoding a full vehicle needs a data source that implements `VinData`. See the crate documentation.

## License

MIT. Vehicle data terms and third-party notices are in the repository's `NOTICE.md`.
