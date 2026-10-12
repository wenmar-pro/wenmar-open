# Trouble codes

Each file here holds trouble codes described **in this project's own words**. `open-data build-shop` builds them into the shop file's `code` table (dataset `codes`), and the site, the API and the tools show them.

A code number and the system it belongs to are facts. Sentences about a code are text, and the official sentences (SAE J2012) are sold. Nothing here may be copied or reworded from them, from another website or app, or from a code list in another project. Read `CONTRIBUTING.md` → "Curated data" before writing a record.

## One file per range

| File | Codes |
|---|---|
| `p0.yaml` | `P0000` to `P0FFF` |
| `p2.yaml` | `P2000` to `P2FFF` |
| `p3.yaml` | `P3400` to `P3FFF` |
| `b0.yaml` | `B0000` to `B0FFF` |
| `c0.yaml` | `C0000` to `C0FFF` |
| `u0.yaml` | `U0000` to `U0FFF` |
| `u3.yaml` | `U3000` to `U3FFF` |

These are the ranges every maker uses the same way. No other code may have a record: the meaning of `P1xxx`, `B1xxx` and the rest depends on the maker. A file is created when its first record is written. A file whose name starts with `_` is not built; `_example.yaml` shows the shape.

## A record

```yaml
codes:
  - code: P0001
    system: powertrain/fuel-air
    title: "Made-up title one"
    meaning: >-
      Made-up meaning one.
    common_causes:
      - "Made-up cause A"
    checks:
      - "Made-up check A"
    components: [engine]
    related: [P0002]
    author: your-github-name
    reviewer: their-github-name
    checked: 2026-10-12
```

| Field | Meaning |
|---|---|
| `code` | The number, in capitals. Records in a file are in ascending order. |
| `system` | Where the index page lists it. For a `P` code: `powertrain`, or one of `powertrain/fuel-air`, `powertrain/ignition`, `powertrain/emissions`, `powertrain/speed-idle`, `powertrain/computer`, `powertrain/transmission`, `powertrain/hybrid`. For a `B` code `body`, for a `C` code `chassis`, for a `U` code `network`. |
| `title` | One line, at most 120 characters, for a list and a page heading. |
| `meaning` | What the code tells a service advisor, in plain words. At most 1,500 characters. |
| `common_causes` | 1 to 10 entries, each at most 200 characters. General, not about one make. |
| `checks` | 1 to 10 entries: what a technician usually verifies first. |
| `components` | Up to 8 ids from `data/components.yaml`. They link the code to the recalls and bulletins about those parts on a vehicle. May be left out. |
| `related` | Up to 10 other codes. Each must have a record. May be left out. |
| `author` | The GitHub name of the person who wrote the record. |
| `reviewer` | The GitHub name of the person who approved it. Not the author. |
| `checked` | The day the reviewer approved it, `2026-10-12`. |

## What the build refuses

A code outside the ranges above; a code in the wrong file; two records for one code; records out of order; a `system` that is not in the list or not under the code's letter; a missing or empty field; text over its limit; a component that is not in `data/components.yaml`; a related code that has no record, or is the code itself; an `author` and `reviewer` that are the same; a `checked` that is not a date; a field this page does not name.

The build checks the shape of a record. It cannot check that the words are yours, or that they are true. The reviewer does.

## Checking a change

```bash
cargo test -p open-data --test shop codes
```
