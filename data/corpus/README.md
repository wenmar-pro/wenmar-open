# Parity corpus

`nhtsa.json` holds NHTSA's own answers for a set of VINs, recorded from their public API. `baseline.json` holds how closely this project's decoder agreed with them when the baseline was last updated. NHTSA's data is in the public domain.

The VINs are synthetic. Each is generated from vPIC's patterns so that it describes a real model. The serial number is `000001` wherever the patterns leave it free; a few low-volume manufacturers encode part of their code in those positions. Four VINs already published as examples are added. None comes from a customer or a real registration.

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

As of the 2026.09 data and 518 VINs, the model year agrees on all 518, including 60 heavy vehicles whose manufacturer code is used in both 30-year cycles. The make agrees on 517.

The differences that remain, by cause:

- **Sources not read yet** account for every `missing` value and for 12 `displacement_cc` differences. NHTSA fills transmission, drive type, ABS, ESC, TPMS, the driver-assistance fields, and exact cubic centimetres from its vehicle-specification tables, engine-model patterns and defaults. Reading those is planned.
- **Names spelled differently in the live API than in the monthly release:** 9 models (`Promaster` here, `ProMaster` at NHTSA; `LAND CRUISER`, `Land Cruiser`), 17 body classes (`Off-road Vehicle`, `Off-Road Vehicle`), 3 transmissions and 3 engine configurations. Only capitalisation differs.
- **Data that changed after the release:** `JH2JA5557NY000001` is body `Motorcycle - Underbone` in the release and `Dual-Sport` live. `2C6WX38AX6A000001` decodes from the release but NHTSA's live API reports no detailed data for it (error code 8), which accounts for every `extra` value on make, model, series, body, doors and drive type. `1BAM2CXA4BB000001` has trim `Front Engine` in the release and none live.

Not covered by this corpus: for heavy vehicles NHTSA also prefers the model year with fewer invalid or unexplained characters. That check is not implemented, and elements filled from the sources above do not yet count towards the choice. A wider probe during review found about 1 in 70 heavy vehicles with schemas in both cycles affected, mostly trailers. Cars, MPVs and light trucks are not affected.

## Refresh

```bash
python3 tools/corpus/generate.py data/build/wenmar-open-2026.09.sqlite3 data/build/corpus-vins.json
python3 tools/corpus/record.py data/build/corpus-vins.json data/corpus/nhtsa.json
cargo run --release -p open-data -- parity --data data/build/wenmar-open-2026.09.sqlite3 --update-baseline
```

The generator samples with a fixed seed from rows read in a fixed order, so the same data file gives the same VINs. Recording makes about 11 requests to NHTSA's API. The check compares VIN by VIN: review every VIN that newly differs before committing a new baseline.
