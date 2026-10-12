# Curated files for the shop data

`open-data build-shop` reads these when it matches a source's vehicle names to the catalog. They are written by hand for this project.

## `models.yaml`

Recall, bulletin and fuel files name vehicles in their own words. The build matches a name to a catalog model in this order, and records which step matched in the `match` column:

1. `exact`: the make, then a model of that make with the same name. Case and punctuation are ignored, so `F-150`, `F150` and `f 150` are one name.
2. `base`: the longest model name of that make that the source's name starts with, if it has at least three letters or digits. `SILVERADO 1500` is the catalog's `Silverado`.
3. `curated`: an entry in this file.
4. Otherwise the row is kept unmatched, with the source's own text.

A model only matches for a model year the catalog lists it in.

| Field | Meaning |
|---|---|
| `make` | The source's make text. It must be a make in the catalog, or one of its aliases in `data/catalog/makes.yaml`. |
| `model` | The source's model text. |
| `to` | One or more catalog model names of that make, as `open-data catalog --data <data file> models --make <make>` prints them. A family name lists every model in the family. |

Put every `model` and `to` name in double quotes. YAML reads a bare `1500` or `330e` as a number, and the build then refuses the file.

An entry is needed only when steps 1 and 2 fail. Run `open-data coverage` to see the commonest unmatched names; that list is where entries come from.

The build stops if an entry names a make or a model the catalog does not have, or if two entries have the same make and model.

Do not copy text from any source into this file. Names are facts; descriptions are not needed here.

## Checking the match rate

```bash
bin/shop-data     # build a shop file from the newest data file
bin/coverage      # report and check
```

`open-data coverage` prints, for each dataset with vehicle rows, how many rows matched and how (`exact`, `base`, `curated`), and the 25 commonest names that matched nothing. Only rows whose make is a light make in the catalog and whose model year is 2000 or later are measured: half of all recall rows are heavy trucks, buses and trailers, which the catalog's light scope leaves out by design.

It fails when a dataset's matched share is below its minimum in `data/coverage.yaml`, when a dataset has no minimum, or when the file names a dataset the shop file lacks. After adding entries to `models.yaml`, run `open-data coverage --shop <file> --catalog <file> --write` to record the higher figures, read the diff, and commit it.

## Recalls

`bin/shop-data` adds the recall datasets when their sources have been fetched:

```bash
cargo run --release -p open-data -- fetch nhtsa-recalls   # 22 MB, United States
cargo run --release -p open-data -- fetch tc-recalls      # 204 MB, Canada
bin/shop-data
```

A campaign is one row of `recall`; each year, make and model it lists is a row of `recall_vehicle`, with the catalog ids it matched and how. A vehicle that matched nothing is kept with the source's own words, so it can still be found from its campaign. Transport Canada's rows for tires, child seats, equipment, snowmobiles and off-road vehicles are left out; the list is `CATEGORIES` in `crates/open-data/src/shop/tc_recalls.rs`, and a category that is not in it stops the build until someone decides.

## Crash test ratings

`bin/shop-data` adds the `ratings` dataset when its source has been fetched:

```bash
cargo run --release -p open-data -- fetch nhtsa-ratings   # 10 MB
bin/shop-data
```

NHTSA rates a vehicle by body style and drive, so one model year often has several ratings. Each row of NHTSA's file is one row of `rating`, with the model, body style and drive in NHTSA's words, and `rating_vehicle` ties it to the catalog models its name matched. Every rating is kept, a vehicle NHTSA lists and has not tested among them; none is picked. A feature NHTSA's file writes in words the build does not read is stored as NULL, never guessed: the words it reads are in `fitted` and `evaluation` in `crates/open-data/src/shop/nhtsa_ratings.rs`.

`data/coverage.yaml` has a minimum for `ratings`, so `open-data coverage` fails on a shop file built without them.

## `oil-grades.yaml`

Engine oil spellings in EPA's Fuel Economy Guide files that `open-data` cannot read by pattern. The pattern (`crates/open-data/src/shop/oil_grade.rs`) reads `0W20`, `SAE 0W-20`, `ow-20`, `5W/30, VW50400` and the like; this file is for the few that are misspelt or name no grade.

| Field | Meaning |
|---|---|
| `text` | The spelling, as the guide file has it. Case and extra blanks are ignored. |
| `grades` | The viscosity grades it means, each written like `"5W-30"`. An empty list says the text names no grade. |

The build stops when a guide file has a spelling that neither the pattern nor this file knows, and names it. A person then reads it and adds an entry. Never add a grade the text does not plainly mean: with any doubt, give it an empty list. The file refuses an entry the pattern already reads.

## Bulletins

`bin/shop-data` adds the `bulletins` dataset when its source has been fetched:

```bash
cargo run --release -p open-data -- fetch nhtsa-mfr-comms   # 87 MB
bin/shop-data
```

A manufacturer communication (a service bulletin, a service campaign, a warranty extension, an over-the-air update) is one row of `bulletin`, keyed by NHTSA's id number, with NHTSA's summary. The document itself is the manufacturer's and is never stored. The vehicles are compacted, because the communications list more than four million between them: `bulletin_model` holds each make and model text once for each catalog model it matched, and `bulletin_vehicle` holds one row for each communication, name and run of consecutive model years.

Bulletins add 144 MB, so only the hosted service carries them. `bin/shop-data` writes two files: `data/build/shop/wenmar-open-shop-<version>.sqlite3` with every dataset, which the server reads, and `data/build/shop/offline/wenmar-open-shop-<version>.sqlite3` without bulletins, which is what the command-line tool and the npm package download. The two differ only in the bulletin tables and in the `bulletins` key of `datasets`.

## Components, codes and topics

After the sources are stored the build reads `data/components.yaml` (see `data/components.md`) and records which components each recall and each bulletin is about, in `recall_component` and `bulletin_component`: a text such as `SERVICE BRAKES, HYDRAULIC:FOUNDATION COMPONENTS:DISC:PADS` is about `brake-pad` and about `brakes`. It also reads each bulletin's summary for trouble codes (`bulletin_code`: `P0420`, five characters) and for the phrases that mark a maintenance bulletin (`bulletin_topic`). The rules that keep a model name such as `C1500` or a campaign number from being taken for a code are in `crates/open-data/src/shop/bulletin_text.rs`.

## Siblings

After the sources are stored the build fills `vehicle_sibling`: for a model year, the other models that share recall campaigns and bulletins with it. Only documents that list two to four catalog models are counted; a pair is kept from five shared documents up, when they are at least a fifth of the model year's own. It says two models have documents in common. It is not fitment and is never shown as such.

## Complaints

`bin/shop-data` adds the `complaints` dataset when its source has been fetched:

```bash
cargo run --release -p open-data -- fetch nhtsa-complaints   # 374 MB
bin/shop-data
```

NHTSA's file holds every complaint an owner has filed since 1995, with what the owner wrote, where they live and part of the VIN. The build keeps none of that. It reads eleven of each record's 51 fields (the list is `READ` in `crates/open-data/src/shop/complaints.rs`) and stores one table, `complaint_count`: for each catalog model year and each component of `data/components.yaml`, the number of complaints, how many of them report a crash, a fire or an injury, and the median mileage at failure. The row with an empty `component_id` counts every complaint of the model year. A test fails if the table gains a column.
