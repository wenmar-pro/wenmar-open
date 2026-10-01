# wenmar-vin

VIN decoding for auto repair shops. The decoder behind [Wenmar Open](https://open.wenmarpro.com), by [Wenmar Pro](https://wenmarpro.com).

- Validates a VIN and explains what is wrong with a bad one.
- Checks the check digit and suggests the likely typo.
- Works out the model year, including the 30-year ambiguity.
- Matches NHTSA vPIC patterns to describe the vehicle, including safety equipment.

## Status

Pre-release. This version ships the decoder and an in-memory data source. The SQLite data source and the data file built from NHTSA's vPIC release are not published yet.

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
