# Tire fitments, read by hand

`open-data build-shop` builds these files into the `tire_fitment` table of the shop file, as the dataset `tires`. Nothing here is downloaded: a person read every figure from the door placard or the maker's own manual. The rules for such records are in "Curated data" in `CONTRIBUTING.md`; read them first.

The directory holds no record until a person has entered one. `_example.yaml` shows the shape with a vehicle that does not exist; the build leaves out every file whose name starts with `_`.

## One file per make and model

A file is named `<make>_<model>.yaml`, both in the catalog's id form: `ford_f-150.yaml`, as in `data/fluids/`. It starts with `make` and `model`, then a list `fitments`.

## A record

A record is one size the maker fitted, over a range of model years. A model year usually has several: record every one with the trims it covers. Never pick one.

| Field | Needed | Meaning |
|---|---|---|
| `year_from` | yes | The first model year the record is for. |
| `year_to` | no | The last. Leave out for one model year. Use a range only when the source you read covers every year in it. |
| `trims` | no | The trims or wheel options the source ties this size to, as the source names them, each in double quotes. Leave out when the source does not say. |
| `size` | yes | The size as printed, in double quotes. The front size when the rear differs. |
| `size_rear` | no | The rear size, only when it differs from the front. |
| `pressure_front`, `pressure_rear`, `pressure_unit` | no, but all three together | The cold pressures as printed, in double quotes, and their unit: `kpa`, `bar` or `psi`. |
| `lug_torque`, `lug_torque_unit` | no, but together | The wheel nut torque as printed, in double quotes, and its unit: `nm` or `lbft`. |
| `bolt_pattern` | no | The number of studs and the pitch circle, as the source gives them, in double quotes. |
| `market` | no | `us` or `ca`, when the source is for one country only. |
| `source` | yes | Where you read it: `door placard`, or the manual's name and its page. Name both when the record draws on both. |
| `checked` | yes | The day you read it, `2026-10-12`. |
| `reviewer` | yes | The GitHub name of the maintainer who compared the record with the source. Not the author. |

Trims are not checked against the catalog: its trims come from NHTSA's data and a short list of our own, and a placard names wheel options the catalog does not have.

## Units

Write a figure once, in the unit the source prints, and name the unit. Where the source prints both a metric and a US figure, write the metric one. The build works out the other with whole-number arithmetic from the exact definitions, and rounds half up to a whole number:

| Figure | Kept as written | Worked out | Exact definition |
|---|---|---|---|
| Pressure | kilopascals, bar or psi | kilopascals or psi | 1 psi = 4.4482216152605 N on 645.16 mm², 1 bar = 100 kPa |
| Torque | newton metres or pound-feet | the other | 1 lb·ft = 1.3558179483314004 N·m |

Pressures and torque are stored as whole numbers. A bar figure is stored as kilopascals; one that is not a whole number of kilopascals is refused.

## What the build refuses

The build stops, naming the file and the record, when a record has no `source`, `checked` or `reviewer`; names a unit it does not know; has a field it does not know; gives a figure that is not plain digits; names a make, a model or a model year the catalog does not have; has a `checked` day after the day of the build; or repeats another record for the same model year, size, trims and `market`.

It cannot tell whether a figure is true. That is the reviewer's work.

## What may not be copied

Record the figures. Never copy a table or a picture from the manual, and never take a size from a tire seller's fitment guide, another website or a paid database.
