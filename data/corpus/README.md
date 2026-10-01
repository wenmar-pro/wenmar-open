# Parity corpus

`nhtsa.json` holds NHTSA's own answers for a set of VINs, recorded from their public API. `baseline.json` holds how closely this project's decoder agreed with them when the baseline was last updated. NHTSA's data is in the public domain.

The VINs are synthetic. Each is generated from vPIC's patterns so that it describes a real model, with the serial number `000001`, plus a few VINs already published as examples. None comes from a customer or a real registration.

## Check agreement

```bash
cargo run --release -p open-data -- parity --data data/build/wenmar-open-2026.09.sqlite3
```

This prints a table and fails if any field agrees less than the baseline. Per field:

| Column | Meaning |
|---|---|
| agree | Both have the same value |
| differ | Both have a value and they are not the same |
| missing | NHTSA has a value and we do not |
| extra | We have a value and NHTSA does not |
| both empty | Neither has a value |

## Known differences

As of the 2026.09 data and 443 VINs, make and model agree on 442 (the other is empty on both sides) and year on 442. No difference found so far comes from the decoding rules.

- **Sources not read yet** account for almost every `missing` value: NHTSA fills transmission (24 VINs), drive type (9), ABS (77), ESC (78), TPMS (132) and the driver-assistance fields from its vehicle-specification tables, engine-model patterns and defaults. Reading those is planned.
- **A heavy vehicle whose year NHTSA settles by its error check:** 1 VIN. `3WKKJHAA6HC000001` is 1987 here and 2017 at NHTSA. NHTSA prefers the year with fewer character errors, using a table of valid characters per manufacturer and year that this project does not carry. The same VIN accounts for the one `fuel` difference and the `extra` engine values.
- **NHTSA's live data is newer than the monthly release:** 1 VIN. `1N6CM0K55KA000001` has body `Cargo Van` in the 2026.09 release and `Van` from the live API.
- **A synthetic VIN that NHTSA corrected before decoding:** 1 VIN. `JH2JA5556NA000001` (error code 4) gets a different motorcycle body class.

## Refresh

```bash
python3 tools/corpus/generate.py data/build/wenmar-open-2026.09.sqlite3 data/build/corpus-vins.json
python3 tools/corpus/record.py data/build/corpus-vins.json data/corpus/nhtsa.json
cargo run --release -p open-data -- parity --data data/build/wenmar-open-2026.09.sqlite3 --update-baseline
```

Regenerating changes the VINs only if the data file changed. Recording asks NHTSA's API about 10 requests. Review the table before committing a new baseline: it should only ever get better.
