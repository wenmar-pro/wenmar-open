# Vehicle catalog

The data file holds a catalog for choosing a vehicle step by step: year, make, model, submodel, engine. It is built by `open-data build` from the same NHTSA vPIC release as the VIN decoding data. No NHTSA API is called during a build.

## Files here

| File | What it is |
|---|---|
| `makes.yaml` | Makes listed first, in order, and other names people type for them (`chevy`, `vw`). Every other make follows alphabetically. |
| `names.yaml` | Spellings to use for trims and series where vPIC's is poor. |
| `../presets.yaml` | Trims added by hand for popular models where vPIC lists few or none. |
| `nhtsa-models.json` | NHTSA's own model lists for a sample of makes and years, recorded from their public API. |
| `baseline.json` | How the catalog compared with those lists when the baseline was last recorded. |

The three YAML files are written by hand for this project. NHTSA's data is in the public domain.

## What is derived, and from what

- **Years, makes, models.** A model is listed for a year when one of its vPIC schemas applies to a manufacturer code that year. Years run from 1981 to the year after the build.
- **Submodels.** vPIC's trim where it has one, otherwise its series. A value that lists several trims (`S, SE, SEL`) becomes several submodels. Spellings that differ only in case become one.
- **Engines.** The same short label a VIN decode shows, such as `3.5L Turbo V6`.
- **Eighth VIN character.** Given for an engine when the patterns say that character means that engine and no other for the model year.
- **Vehicle ids.** `2019_honda_civic_si_1-5l-turbo`: the year and the names in lowercase with hyphens, joined by underscores. They do not change between data releases unless a name does. They are not taken from any other catalog.

To find which trims and engines go together, the build works through every combination of VIN positions 4 to 8 that a model's patterns can tell apart, and records what each would decode to.

## What vPIC does not provide

- **Trims are often missing.** Of 53,050 model years of cars, MPVs and trucks in the 2026.09 data, 13,984 have a trim from vPIC, 22,960 have only a series, and the rest have neither. The Ford F-150 has `Raptor` and `Police` and no `XL`, `XLT` or `Lariat`; those come from `presets.yaml`.
- **Engines can be listed for models that never had them.** When a manufacturer puts several models in one schema and marks the engine only by the eighth character, vPIC does not say which engine goes with which model, and each model lists them all. The 2021 Ram 1500 lists a 6.7L diesel for this reason. Heavy trucks can list dozens.
- **The eighth character settles the engine for about a third of model years.** 14,999 of the 47,163 model years with engines. Honda and Toyota, among others, put the engine elsewhere in the VIN.
- **"Trucks" includes heavy trucks.** vPIC's vehicle type does not separate a pickup from a highway tractor, so the default scope (cars, MPVs and trucks) includes makes such as Freightliner. They sort after the popular makes.
- **Details are as reported.** A turbocharger vPIC does not record is not in the label (the F-150's 2.7L shows as `2.7L V6`). Many cars have the drive type `4x2`.
- **A schema with no end year runs to the year after the build,** as it does in NHTSA's own model lists.
- **Body, drive and transmission** are shown for a model year or submodel only when every configuration agrees. Transmissions NHTSA records in its per-trim specification sheets are not read by the catalog.

## Look something up

```bash
cargo run --release -p open-data -- catalog --data data/build/wenmar-open-2026.09.sqlite3 makes --year 2019
cargo run --release -p open-data -- catalog --data data/build/wenmar-open-2026.09.sqlite3 engines --make ford --model f-150 --year 2019
cargo run --release -p open-data -- catalog --data data/build/wenmar-open-2026.09.sqlite3 search 2019 civic si
cargo run --release -p open-data -- catalog --data data/build/wenmar-open-2026.09.sqlite3 vin 1HGCM82633A004352
```

Add `--scope all` to include trailers, motorcycles, buses and the rest, or `--scope 6` for one vPIC vehicle type.

## Check against NHTSA

```bash
cargo run --release -p open-data -- catalog-parity --data data/build/wenmar-open-2026.09.sqlite3
```

This compares the catalog's models with NHTSA's recorded lists and fails if a model is newly missing or newly extra. Names are compared without regard to case.

## Known differences

As of the 2026.09 data and 144 make-years recorded on 2026-10-01: 2,849 models agree, one is missing from the catalog, and none is extra.

- **2026 Lexus GX** is in NHTSA's list and not in the catalog. NHTSA's API answers from newer data than the monthly release. In the 2026.09 release the newest GX schema covers 2025 only, so the catalog stops there. It should appear once a release carries the 2026 schema; record the lists again then.

## Refresh

```bash
python3 tools/catalog/record.py data/catalog/nhtsa-models.json
cargo run --release -p open-data -- catalog-parity --data data/build/wenmar-open-2026.09.sqlite3 --update-baseline
```

Recording makes 144 requests to NHTSA's API, one a second. Read every difference before committing a new baseline.

## Changing the lists

A change to `makes.yaml`, `names.yaml` or `presets.yaml` takes effect at the next build. Renaming a submodel in `names.yaml` changes its id.
