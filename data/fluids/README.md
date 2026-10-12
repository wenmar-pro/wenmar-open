# Fluids, read by hand

`open-data build-shop` builds these files into the `fluid` table of the shop file, as the dataset `fluids`. Nothing here is downloaded: a person read every figure from the maker's own document. The rules for such records are in "Curated data" in `CONTRIBUTING.md`; read them first.

The directory holds no record until a person has entered one. `_example.yaml` shows the shape with a vehicle that does not exist; the build leaves out every file whose name starts with `_`.

## One file per make and model

A file is named `<make>_<model>.yaml`, both in the catalog's id form: `ford_f-150.yaml`. That is a vehicle id without its year (`2018_ford_f-150`). Find the id forms with:

```bash
wenmar-open vehicles search 2018 f-150 --json
```

The file starts with `make` and `model`, the same two id forms, then a list `fluids`.

## A record

A record is one fluid, for one engine or for every engine, over a range of model years.

| Field | Needed | Meaning |
|---|---|---|
| `year_from` | yes | The first model year the record is for. |
| `year_to` | no | The last. Leave out for one model year. Use a range only when the source you read covers every year in it; a figure is never carried to a year nobody checked. |
| `kind` | yes | `engine_oil`, `transmission`, `coolant`, `refrigerant` or `brake`. |
| `engine` | for `engine_oil` | The engine's id form, as the catalog lists it for that model year: `2-7l-v6`. `wenmar-open vehicles engines --make ford --model f-150 --year 2018 --json` lists them, each with its `id`. Leave out on another kind when the fluid is the same for every engine. |
| `applies_to` | no | Which transmission or system the record is for, in your own words: `10-speed automatic`. |
| `grade` | `engine_oil` only | The viscosity the source gives, in double quotes. |
| `specification` | yes, except for `engine_oil` | The maker's specification, or the type of fluid, as the source names it. A name is a fact. |
| `capacity`, `capacity_unit` | `engine_oil` only | The capacity as the source prints it, in double quotes, and its unit: `l` or `qt` (US liquid quarts). |
| `with_filter` | with `capacity` | `true` when the capacity includes the filter. |
| `charge`, `charge_unit` | `refrigerant` only | The charge as printed, in double quotes, and its unit: `g`, `kg`, `oz` or `lb`. |
| `market` | no | `us` or `ca`, when the source is for one country only. |
| `source` | yes | The document you read, exactly enough for someone else to find the figure: the manual's name and its page, or `under-hood label`. |
| `checked` | yes | The day you read it, `2026-10-12`. |
| `reviewer` | yes | The GitHub name of the maintainer who compared the record with the source. Not the author. |

An `engine_oil` record needs at least one of `grade`, `specification` and `capacity`. Transmission fluid and coolant are recorded by specification only: their capacities depend on how the service is done.

## Units

Write a figure once, in the unit the source prints, and name the unit. Where the source prints both a metric and a US figure, write the metric one. The build works out the other with whole-number arithmetic from the exact definitions, and rounds half up:

| Figure | Kept as written | Worked out | Exact definition |
|---|---|---|---|
| Capacity | litres or US quarts, up to three decimals | the other, to one decimal | 1 US qt = 0.946352946 L |
| Charge | grams (whole), kilograms, ounces or pounds | grams to a whole number, ounces to one decimal | 1 oz = 28.349523125 g, 1 lb = 16 oz |

A kilogram figure is stored as grams and a pound figure as ounces.

## What the build refuses

The build stops, naming the file and the record, when a record has no `source`, `checked` or `reviewer`; names a unit or a `kind` it does not know; has a field it does not know; gives a figure that is not plain digits; names a make, a model, a model year or an engine the catalog does not have; has a `checked` day after the day of the build; or repeats another record for the same model year, kind, engine, `applies_to` and `market`.

It cannot tell whether a figure is true. That is the reviewer's work.

## What may not be copied

Record the figure and the name of the specification. Never copy a sentence, a table or a picture from the manual, and never take a figure from another website, an oil maker's product finder, a parts catalogue or a paid database.
