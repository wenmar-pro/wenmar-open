# Parity corpus

`nhtsa.json` holds NHTSA's own answers for a set of VINs, recorded from their public API. `baseline.json` holds how closely this project's decoder agreed with them when the baseline was last updated. NHTSA's data is in the public domain.

The VINs are synthetic. Each is generated from vPIC's patterns so that it describes a real model. The serial number is `000001` wherever the patterns leave it free; a few low-volume manufacturers encode part of their code in those positions. Four VINs already published as examples are added. None comes from a customer or a real registration.

## Check agreement

```bash
cargo run --release -p open-data -- parity --data data/build/wenmar-open-2026.10.sqlite3
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

As of the 2026.10 data and 518 VINs, the model year agrees on all 518, including 60 heavy vehicles whose manufacturer code is used in both 30-year cycles. The make agrees on 517 and the manufacturer on all 518. Transmission, drive type, cylinders, the driver-assistance fields, wheel sizes and base price have no missing value, and nothing differs on them beyond the capitalisation below. Fuel, electrification, seats and the plant fields differ only on the VIN the live API has lost.

The differences that remain, by cause:

- **Defaults not read yet:** 48 `gvwr` values. When nothing else sets a weight class, NHTSA gives every motorcycle and low-speed vehicle `Class 1A: 3,000 lb or less`. That accounts for 47 motorcycles and 1 low-speed vehicle here. Reading the per-vehicle-type defaults is planned.
- **Very small engines:** 3 `displacement_l` values, on `1HFAF1606HA000001`, `JS1JA13A7SA000001` and `MLHAR02088A000001`. NHTSA reports a 49 cc engine as 0.049 L. This project rounds litres to one decimal, so below 0.05 L it gives cubic centimetres only.
- **Names spelled differently in the live API than in the monthly release:** 9 models (`Promaster` here, `ProMaster` at NHTSA; `LAND CRUISER`, `Land Cruiser`), 17 body classes (`Off-road Vehicle`, `Off-Road Vehicle`), 3 transmissions (`Dual-clutch`, `Dual-Clutch`) and 3 engine configurations (`Horizontally opposed`, `Horizontally Opposed`). Only capitalisation differs.
- **Data the live API has lost:** `4S9M2A1E9PA569001` (2023 Kandi low-speed vehicle) decodes from the release but NHTSA's live API reports no detailed data for it (error code 8). That accounts for every `extra` value on model, body, fuel, electrification, plant city, state and country, and seats.

Not covered by this corpus: for heavy vehicles NHTSA also prefers the model year with fewer invalid or unexplained characters. That check is not implemented, and the defaults above do not yet count towards the choice. A wider probe during review found about 1 in 70 heavy vehicles with schemas in both cycles affected, mostly trailers. Cars, MPVs and light trucks are not affected.

## Refresh

```bash
python3 tools/corpus/generate.py data/build/wenmar-open-2026.10.sqlite3 data/build/corpus-vins.json
python3 tools/corpus/record.py data/build/corpus-vins.json data/corpus/nhtsa.json
cargo run --release -p open-data -- parity --data data/build/wenmar-open-2026.10.sqlite3 --update-baseline
```

The generator samples with a fixed seed from rows read in a fixed order, so the same data file gives the same VINs. Recording makes about 11 requests to NHTSA's API. The check compares VIN by VIN: review every VIN that newly differs before committing a new baseline.
