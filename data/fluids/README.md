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

## Links to the manual itself

While the maker's page for the model year is open, record its address. The file may carry a second list, `manuals`, beside `fluids`:

| Field | Needed | Meaning |
|---|---|---|
| `year_from`, `year_to` | `year_from` | As for a fluid. A range only when the one address serves every year in it. |
| `kind` | yes | `owner_manual` or `maintenance_schedule`. |
| `url` | yes | The address on the maker's own site, starting `https://`. |
| `market` | no | `us` or `ca`, when the page is for one country only. |
| `checked` | yes | The day you opened it. |
| `reviewer` | yes | As for a fluid. |

A link has no `source`: the address is where it leads. A model year may have one link of each kind for each market. The build checks the shape of the address and never requests it; `mise run check-links` does that, and reports. Never download a manual into the repository, and never link to a copy on a site that is not the maker's.

## Entering a record, step by step

1. See what is missing: `cargo run -q -p open-data -- hand-coverage --catalog data/build/wenmar-open-2026.09.sqlite3` lists the makes in the order of `data/catalog/makes.yaml` and, for each, the models with no record. The repository has no sales figures, so choosing which model to do first is yours.
2. Get the id forms: `wenmar-open vehicles search 2018 f-150 --json` for the make and model, and `wenmar-open vehicles engines --make ford --model f-150 --year 2018 --json` for the engines.
3. Open the maker's own document for that model year. Read the figure; write it in the file with its unit, the document and page in `source`, and today's date in `checked`.
4. Check the shape: `cargo test -p open-data --test shop hand`.
5. Check it against the catalog and look at the page: `bin/shop-data`, then `mise run serve`, and open `http://localhost:3000/makes/<make>/<model>/<year>`.
6. Open a pull request with at most 20 records. Say how the reviewer can see each source. The reviewer opens the source, compares every figure, and puts their GitHub name in `reviewer`.

The same steps serve `data/tires/`.
