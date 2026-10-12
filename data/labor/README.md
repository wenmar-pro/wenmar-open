# Labour estimates

Each file here holds Wenmar's own estimates of the hours one job takes. `open-data build-shop` builds them into the shop file's `labor_estimate` table (dataset `labor`). They are published under **CC BY 4.0**, with the credit `Labour estimates by Wenmar Pro, CC BY 4.0`.

**An estimate is written by a person who does the work, from the work they have done.** It is never taken from a labour guide (MOTOR, Mitchell, ALLDATA), from a maker's warranty time, from shop software or from another website, and none of those may be open while you write. The labour allowance a bulletin mentions is not used either. Read `CONTRIBUTING.md` → "Curated data" first.

## One file per job

A file is named after its job: `brake-pads-front.yaml` holds the estimates for the job `brake-pads-front` of `data/jobs.yaml`. A file is created when its first estimate is written. A file whose name starts with `_` is not built; `_example.yaml` shows the shape. The job list must be reviewed (`status: reviewed`) before any estimate is built.

## An estimate

```yaml
job: test-job-a
estimates:
  - scope: class
    class: compact-car
    low: "97.5"
    typical: "98"
    high: "99.25"
    basis: expert
    notes: "Made-up note."
    author: your-github-name
    reviewer: their-github-name
    updated: 2026-10-12
```

| Field | Meaning |
|---|---|
| `scope` | What the figure is for: `class`, `model`, `generation` or `engine`. |
| `class` | For `class` only: one of the ids below. |
| `make`, `model` | For the other three: the id forms of a catalog make and model, as in a vehicle id (`ford`, `f-150`). |
| `year_from`, `year_to` | For `generation` and `engine`: the first and last model year the figure covers. Both are required, and every year between must be in the catalog. |
| `engine` | For `engine` only: the engine's id form, as `wenmar-open vehicles engines` prints it and a full vehicle id ends with. Every model year of the range must have that engine. |
| `low`, `typical`, `high` | Hours, each in double quotes, with at most two decimal places: `"1.5"`. From `0.01` to `999.99`, and low ≤ typical ≤ high. `typical` is a vehicle in ordinary condition; `high` allows for rust and seized fasteners. |
| `basis` | `expert`. (Measured figures are not written by hand: see `observed/README.md`, plan J6.) |
| `notes` | Optional, at most 300 characters: what makes this vehicle slower or faster, in your own words. |
| `author` | The GitHub name of the person who wrote it. |
| `reviewer` | The GitHub name of the person who approved it. Not the author. |
| `updated` | The day it was last changed, `2026-10-12`. |

Class ids: `subcompact-car`, `compact-car`, `midsize-car`, `large-car`, `two-seater`, `station-wagon`, `small-suv`, `standard-suv`, `suv`, `minivan`, `van`, `small-pickup`, `standard-pickup`. A model year's class comes from EPA's size class; a model year EPA never rated may have none, and a class figure then does not reach it.

## Which figure a vehicle gets

The narrowest scope that has one: the engine, then the run of model years, then the model, then the class. The answer says which it was. So write the class figure first; add a model, a generation or an engine only where you know the job is different there.

## What the build refuses

A file not named after its job, or for a job that is not in `data/jobs.yaml`; a `scope` that is not one of the four; a field the scope does not use, or a missing one it needs; a class that is not in the list; a make, a model, a model year or an engine the catalog does not have; hours that are not written as above, or out of order; two estimates of one job for the same scope, or two whose model years overlap; a `basis` other than `expert`; a missing `author`, `reviewer` or `updated`; an `author` and `reviewer` that are the same; a field this page does not name.

The build checks the shape of an estimate. It cannot check that the hours are yours, or right. The reviewer does.

## Checking a change

```bash
cargo test -p open-data --test shop labor::
```
