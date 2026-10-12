---
name: wenmar-open
description: Decode VINs, look up vehicles by year, make, model, submodel and engine, and list the recall campaigns that may apply to a vehicle, with the wenmar-open command-line tool. Use when a task involves a VIN, identifying a vehicle, listing makes, models, trims or engines, or safety recalls. Free, no key, no account.
---

# wenmar-open

`wenmar-open` answers vehicle questions for auto repair work: what a VIN is, which years, makes, models, submodels and engines exist, and which recall campaigns may apply to a model year. The vehicle data is NHTSA's vPIC and the recalls are NHTSA's and Transport Canada's, the same data the hosted API at https://open.wenmarpro.com serves.

It needs no key and no account. It answers from a local data file when there is one, and from the hosted API otherwise.

## Output

- Piped, every command prints JSON on standard output: one object or one list, with no wrapper.
- `--jq EXPR` filters the JSON with a jq expression. Text results are printed without quotes.
- An error is JSON on standard error, `{ "error": { "code", "message", "details" } }`, with a non-zero exit code. `details.hint`, when present, says what to do.
- A field that is not known is left out. It is never `null` or empty. Recall and overview answers are the exception: see Recalls and Everything for one vehicle.
- The tool never asks a question and never waits for input.

Exit codes: 0 success, 2 the command line was wrong, 3 `not_found`, 4 `invalid_vin` or `validation_failed`, 5 `rate_limited`, 6 the API failed or answered with something else, 10 `network`, 11 the local data file is missing or unusable or the recalls are not held where the question was asked (`dataset_unavailable`), 1 anything else.

## Decode a VIN

```bash
wenmar-open vin decode KM8K2CAB4PU001140
wenmar-open vin decode KM8K2CAB4PU001140 --jq .catalog.entry.summary
wenmar-open vin decode KM8K2CAB4PU001140 --year 2023
```

The result has `year`, `make`, `model`, `trim`, `engine`, `transmission`, `safety` (ABS, TPMS and driver assistance, as the manufacturer reported them), `wheel_size_front` and `wheel_size_rear` (rim diameter in inches, where NHTSA has them), `manufacturer` and `plant`. `catalog.entry` is the matching catalog vehicle, and `catalog.entry.id` is its stable id.

A wrong check digit is not an error: the result has `"valid": false` and a warning, because many genuine VINs from outside North America fail the check. A VIN of the wrong length or with the letters I, O or Q is `invalid_vin`, and `details.suggestions` lists what was probably meant.

## Find a vehicle without a VIN

Free text, best match first:

```bash
wenmar-open vehicles search 2019 civic si
wenmar-open vehicles search chevy 1500 --limit 5
wenmar-open vehicles search f150 --jq '.[0].id'
```

Search finds more online than offline. From the local data file the text is read as a year, a make, a model and a submodel, in that order. The hosted API also finds a model from its own words in any order, such as `type r` or `hd 2500`. So an empty list from the data file does not mean the vehicle does not exist: ask again with `--online`, or write the model out, as in `civic type r`.

```bash
wenmar-open vehicles search type r --online
```

Or step by step. Each step offers only what is valid for the steps before it:

```bash
wenmar-open vehicles years
wenmar-open vehicles makes --year 2019
wenmar-open vehicles models --make honda --year 2019
wenmar-open vehicles submodels --make honda --model civic --year 2019
wenmar-open vehicles engines --make honda --model civic --year 2019 --submodel si
wenmar-open vehicles entry 2019_honda_civic_si
```

- `--make` and `--model` take a name, an alias or an id: `Chevrolet`, `chevy` and `chevrolet` are the same make, and `f150` finds `F-150`.
- `--term TEXT` keeps names that start with the text.
- `--scope` is `light` (cars, MPVs and trucks; the default), `all`, or a vPIC vehicle type id such as `6` for trailers.
- An unknown make or model gives an empty list, not an error.
- A vehicle id such as `2019_honda_civic_si` stays the same between data releases for as long as the names do, so it can be stored.

## Crash test ratings

```bash
wenmar-open vehicles safety 2018_ford_f-150
wenmar-open vehicles safety 2018_ford_f-150 --jq '.ratings[] | [.description, .stars.overall]'
```

NHTSA's crash test ratings for a model year, from its New Car Assessment Program. NHTSA rates a vehicle by body style and drive, so `ratings` often has several entries and their stars can differ. `description` says which vehicle each is for, in NHTSA's words. Report every one, or the one whose body style and drive are known to be the vehicle's; do not pick one otherwise.

- `stars` has `overall`, `frontal`, `side` and `rollover`, and the single tests behind them, each from 1 to 5. A rating with every star `null` is a vehicle NHTSA lists and has not tested. `ratings` is empty when NHTSA lists none for the model year.
- Each rating also has `rollover_risk_percent`, `curb_weight_kg`, `features` (such as `esc` and `forward_collision_warning`: `standard`, `optional`, `both` or `no`) and `notes`, NHTSA's own remarks.
- `as_of` is the day NHTSA's file was current, and `source_url` is where NHTSA shows a rating in full.
- In this answer every field is always there, and one that is not known is `null`.
- A submodel or an engine in the id is set aside: ratings are listed by year, make and model.

Offline, the ratings are in the shop data file. Without it the error is `dataset_unavailable` and `details.hint` is `Run: wenmar-open data pull --shop`.

## Recalls

Recall campaigns from NHTSA (United States) and Transport Canada. A campaign lists a year, a make and a model, so every answer is about a model year and has `"may_apply": true`. A recall listed may apply to a vehicle; it is never a statement about one VIN. Neither government publishes recalls by VIN as data. To confirm, check the VIN with the manufacturer or at the addresses in `lookup` (`lookup.nhtsa` for the United States, `lookup.tc` for Canada). Say "may apply" when you report these.

```bash
wenmar-open recalls vehicle 2018_ford_f-150
wenmar-open recalls vehicle 2018_ford_f-150 --country ca
wenmar-open recalls vin KM8K2CAB4PU001140
wenmar-open recalls vin KM8K2CAB4PU001140 --jq '.recalls[] | [.date, .campaign, .summary]'
wenmar-open recalls campaign nhtsa 19V888000
wenmar-open recalls search wiring harness --make hyundai --year 2023
wenmar-open recalls search --component "air bags" --limit 5
```

- `recalls vehicle` takes a vehicle id from `vehicles search` or `vehicles entry`. A submodel or an engine in the id is set aside: recalls are listed by year, make and model.
- `recalls vin` decodes the VIN and lists the recalls of its model year. `decode` is what the VIN decoded to, and `vehicle` is the model year. A VIN that decodes to no vehicle in the catalog gives `"vehicle": null`, an empty list and a warning with the code `no_catalog_vehicle`, not an error.
- `recalls campaign` gives one campaign, `nhtsa` or `tc`, with its summary, consequence, remedy and every vehicle it lists.
- `recalls search` lists campaigns newest first. The words are looked for anywhere in a campaign. `--make`, `--model` (needs `--make`), `--year`, `--component` and `--country` narrow the list. `--limit` is at most 100. When `next_cursor` is not `null`, pass it as `--cursor` for the next page.
- `--country` is `us` or `ca`. Left out, both.
- Each campaign has `source` (`nhtsa` or `tc`), `campaign`, `date`, `component`, `summary`, `mfr_campaign` (the manufacturer's own number, which a dealer looks a recall up by) and `url`. `do_not_drive` and `park_outside` are NHTSA's advisories; they are `null` for a Canadian campaign. In an answer about one model year, `match` says how the campaign's wording reached the model: `exact`, `base` (the campaign names a series or trim of it) or `curated`.
- `as_of` is the day each country's data was current. An empty list means none was found as of that day.
- In a recall answer every field is always there, and one that is not known is `null`.
- `attribution` lists the credit lines the data's licences ask for. Show them where you show the data.

Offline, the recalls are in a second file, the shop data file. Without it the error is `dataset_unavailable` and `details.hint` is `Run: wenmar-open data pull --shop`. `lookup.maker`, the manufacturer's own lookup, is filled only by the hosted API; offline it is `null`.

## Everything for one vehicle

One answer with everything held for a model year, asked for by vehicle id or by VIN; every model year of a model; and the recall campaigns reported since a day.

```bash
wenmar-open vehicles overview 2018_ford_f-150
wenmar-open vin overview KM8K2CAB4PU001140
wenmar-open vin overview KM8K2CAB4PU001140 --jq .recalls.count
wenmar-open vehicles model-years --make ford --model f-150
wenmar-open recalls recent --since 2026-09-01
wenmar-open recalls recent --since 2026-09-01 --make ford --country ca
```

- `vehicles overview` and `vin overview` give `vehicle`, `decode` (what the VIN decoded to in brief; `null` when asked by vehicle id), `recalls`, `links` and `attribution`. `recalls` has `count` (`us` and `ca`), `do_not_drive`, `park_outside`, the three `newest` campaigns, `as_of`, `lookup` and `"may_apply": true`. It counts the campaigns that may apply to the year, make and model, and is never a statement about one VIN. The whole list is `recalls vehicle`.
- A block with no data is `null`. `specs`, `fuel`, `fluids` and `safety` (crash test ratings) are filled by the hosted API where its server holds them; from local files they are `null` for now. `tires`, `labor` and `bulletins` are `null` everywhere today. Offline, `recalls` is `null` when there is no shop data file. That is not an error.
- A VIN that decodes to no vehicle in the catalog gives `"vehicle": null`, every block `null` and a warning with the code `no_catalog_vehicle`.
- `vehicles model-years` lists every model year of one model, newest first, each with its vehicle id and `recalls`, the number of campaigns that may apply (`null` when no recalls are held).
- `recalls recent` lists the campaigns reported on or after `--since`, a day written like `2026-09-01`, newest first, each with `vehicles`, the vehicles it lists (at most 200; `vehicles_listed` is how many there are in all). Compare `vehicles[].vehicle_id` with your own list. `--make`, `--country`, `--limit` and `--cursor` are as in `recalls search`. Offline it needs the shop data file.
- In these answers every field is always there, as in a recall answer. `links.maker` and `lookup.maker` are filled only by the hosted API; offline they are `null`.

## Online and offline

```bash
wenmar-open data status
wenmar-open data pull
wenmar-open data pull --shop
wenmar-open data status --shop
wenmar-open doctor
```

`data pull` downloads the data file (about 30 MB) so that VIN decoding and the catalog work without a connection. `data pull --shop` then downloads the shop data file (about 20 MB), which holds the recalls and goes with the data file in place: pull the data file first, and pull the shop data again after the data file changes. `data status` and `data status --shop` say whether each is there. `doctor` checks the data file and the API and says which of them answers.

`--offline` answers only from the data file and fails with `no_data` if there is none. `--online` answers only from the API. `--api URL` or `WENMAR_OPEN_API` names another server.

```bash
wenmar-open vin decode KM8K2CAB4PU001140 --offline
wenmar-open vehicles years --online
```

Online, one address may make 600 requests a minute. Over that the error is `rate_limited` and `details.retry_after` is the number of seconds to wait.

## As an MCP server

```bash
wenmar-open mcp
```

This runs an MCP server on standard input and output with three tools: `wenmar_vin` (actions `decode`, `batch` and `overview`), `wenmar_vehicles` (actions `years`, `makes`, `models`, `submodels`, `engines`, `search`, `entry`, `overview`, `model_years` and `safety`) and `wenmar_recalls` (actions `vehicle`, `vin`, `campaign`, `search` and `recent`). The results are the same JSON as the commands above. `wenmar-open setup claude` and `wenmar-open setup codex` print the command that registers it.

<!-- wenmar-open-skill: managed. Written by `wenmar-open setup`; the next setup replaces it. -->
