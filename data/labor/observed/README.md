# Measured labour times

`observed.yaml` in this directory, when there is one, is a table of labour times measured from the repair orders of Wenmar Pro shops that have opted in. Wenmar Pro produces it; this repository only receives it, checks it and publishes it, under CC BY 4.0 with the credit `Labour estimates by Wenmar Pro, CC BY 4.0`.

Nobody edits this file by hand, and nothing here is written from a labour guide. A new table replaces the old one.

## The file

```yaml
generated: 2027-02-01
rows:
  - job: test-job-a
    scope: class
    class: compact-car
    median: 9800
    low: 9750
    high: 9925
    jobs: 40
    shops: 6
```

| Field | Meaning |
|---|---|
| `generated` | The day the table was produced. It is the only date in the file. |
| `job` | An id of `data/jobs.yaml`. |
| `scope` and its fields | `class` with `class`; `model` with `make` and `model`; `generation` adds `year_from` and `year_to`; `engine` adds `engine`. The same as in `data/labor/README.md`. |
| `median` | The median hours of the repair orders counted, in whole hundredths of an hour: `9800` is 98 hours. |
| `low`, `high` | The 10th and the 90th percentile of the same repair orders, in hundredths. Not the least and the most. |
| `jobs` | How many repair orders were counted. At least 20. |
| `shops` | How many shops they came from. At least 5. |

## What may never be in the file

A row holds the fields above and nothing else. No shop's name, number or place. No customer. No VIN, plate or unit number. No order or invoice number. No date but `generated`. No list of single jobs or their hours. The build refuses the whole table if it finds any other field, a text that looks like a VIN, or a row with fewer than 5 shops or 20 repair orders behind it.

## What the table does

For a job and a vehicle, a measured figure is given before a written one **of the same scope**. A written figure for a narrower scope (a model, an engine) still answers first. Every answer says `"basis": "observed"` and how many repair orders and shops stand behind the figure.

## Checking a table

```bash
cargo test -p open-data --test shop labor::
cargo run -q -p open-data -- build-shop --catalog data/build/wenmar-open-2026.09.sqlite3 --out target/tmp/check.sqlite3 --data-version 2026.10.12
```
