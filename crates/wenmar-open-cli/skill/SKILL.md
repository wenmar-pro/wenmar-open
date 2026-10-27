---
name: wenmar-open
description: Decode VINs, look up vehicles by year, make, model, submodel and engine, list the recall campaigns that may apply to a vehicle, and work out the Ontario payroll deductions a pay owes, with the wenmar-open command-line tool. Use when a task involves a VIN, identifying a vehicle, listing makes, models, trims or engines, safety recalls, or payroll deductions. Free, no key, no account.
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

The result has `year`, `make`, `model`, `trim`, `engine`, `transmission`, `safety` (ABS, TPMS and driver assistance, as the manufacturer reported them), `wheel_size_front` and `wheel_size_rear` (rim diameter in inches, where NHTSA has them), `manufacturer` and `plant`. `catalog.entry` is the matching catalog vehicle, and `catalog.entry.id` is its stable id. It also has `generation`, `engine.hp`, `brake_system`, `electric` (battery and charging), the manufacturer's `notes`, and `attributes`: every element NHTSA resolved, by NHTSA's code, with where it came from. When `year_conclusive` is `false` the VIN leaves the model year open: say so and ask for the year on the label.

Many VINs at once, from the arguments, a file or standard input (every 17-character run is a VIN, so a CSV export works as it is):

```bash
wenmar-open vin batch KM8K2CAB4PU001140 1HGCM82633A004352
wenmar-open vin batch --input fleet.csv --fields year,make,model,engine.label
wenmar-open vin batch --input fleet.csv --csv > decoded.csv
```

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

- `wenmar-open vehicles complaints <id>`: what owners reported to NHTSA about a model year, as counts by component with the median mileage owners gave. These are reports, not verified defects, and a count is not a rate.

- `--make` and `--model` take a name, an alias or an id: `Chevrolet`, `chevy` and `chevrolet` are the same make, and `f150` finds `F-150`.
- `--term TEXT` keeps names that start with the text.
- `--scope` is `light` (cars, MPVs and trucks; the default), `all`, or a vPIC vehicle type id such as `6` for trailers.
- An unknown make or model gives an empty list, not an error.
- A vehicle id such as `2019_honda_civic_si` stays the same between data releases for as long as the names do, so it can be stored.

## Fuel, specifications, fluids and tires

```bash
wenmar-open vehicles fuel 2018_ford_f-150
wenmar-open vehicles fuel 2018_ford_f-150 --country ca
wenmar-open vehicles specs 2018_ford_f-150
wenmar-open vehicles fluids 2018_ford_f-150
wenmar-open vehicles tires 2018_ford_f-150
wenmar-open vin fuel KM8K2CAB4PU001140
wenmar-open vin specs KM8K2CAB4PU001140
wenmar-open vin fluids KM8K2CAB4PU001140 --jq '.engine_oil_checked[] | [.grade, .specification, .capacity_l, .source]'
wenmar-open vin tires KM8K2CAB4PU001140
```

What is held about a model year beside its recalls and its crash test ratings.

- `fuel`: `ratings` from EPA (`us`) and Natural Resources Canada (`ca`), each with `l_100km` and `mpg` (miles per US gallon). `confidence` says how far a rating was matched: `model`, `engine` or `submodel`. `engine` is the catalog engine only when the match settled one; never present a `model` rating as that of one engine.
- `specs`: one entry per EPA rating, with transmission, drive, recommended fuel and more.
- `fluids`: `engine_oil` is the viscosity grade the maker certified the engine with, as EPA records it. It is not from the owner's manual. `engine_oil_checked`, `transmission`, `coolant`, `refrigerant` and `brake` are records a person read from the maker's own documents: say the `source` and the day `checked` with any of them. An empty list means nobody has recorded it. Never estimate a capacity or a specification, and never carry one over from another engine or model year.
- `tires`: `fitments` lists every original-equipment size on record with the `trims` it covers. Report every one; do not pick one. The door placard of the vehicle says which it has.
- By VIN, `fuel`, `specs` and `fluids` hold only what is for the decoded engine when `engine_settled` is `true`. `tires` are never narrowed.

Offline, all four are in the shop data file. Without it the error is `dataset_unavailable` and `details.hint` is `Run: wenmar-open data pull --shop`.

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

## Trouble codes

```bash
wenmar-open codes get P0420
wenmar-open codes search --system powertrain/emissions --query sensor
wenmar-open codes vehicle 2018_ford_f-150 P0420
```

Generic OBD-II trouble codes, described in Wenmar Open's own words. `codes get` gives `title`, `meaning`, `common_causes`, `checks`, the `components` it concerns and `related` codes. `codes search` lists the described codes by code; pass `next_cursor` back as `--cursor`. `codes vehicle` takes a vehicle id and any code and gives the `description` (or `null`), the `bulletins` of that model year whose NHTSA summary names the code, and under `related` the bulletins and recalls of that model year about the same components.

- Only codes every maker uses the same way are described: `P0`, `P2`, `P3400` and up, `B0`, `C0`, `U0`, `U3`. For any other code `codes get` fails with `not_found`, and `codes vehicle` still answers, with `description: null`.
- A description is general. It is not a diagnosis and does not say which part to replace. A recall under `related` may apply to the model year; confirm by VIN with the manufacturer.
- Bulletins are on the hosted service only. Offline, `bulletins` and `related.bulletins` are `null`; that is not an error. Ask again with `--online` for them.
- Offline, the codes are in the shop data file. Without it the error is `dataset_unavailable` and `details.hint` is `Run: wenmar-open data pull --shop`.

## Labour estimates

```bash
wenmar-open labor jobs
wenmar-open labor job brake-pads-front
wenmar-open labor vehicle 2018_ford_f-150 --job brake-pads-front
wenmar-open labor vin 1FTEW1EP5JFA00000
```

Wenmar's own estimates of the hours a repair job takes, written by people who do the work and never taken from a labour guide. `labor jobs` lists the jobs; `labor job` gives one, with what it `includes` and `excludes`. `labor vehicle` and `labor vin` give `estimates`: one entry for each job that has a figure covering the vehicle, with `hours.low`, `hours.typical` and `hours.high`.

- **An estimate, not a quote.** Say so, and never turn hours into a price without the shop's own rate.
- `scope` says what a figure is for: `engine`, `generation`, `model` or `class`. A `class` figure is typical for vehicles of that class (`class_name`) and was not worked out on this vehicle: say that too.
- `basis` is `expert` (written by a person) or `observed` (measured from repair orders; `sample` says how many jobs and shops).
- A job that is missing from `estimates` has no figure for the vehicle. Do not guess one.
- Credit the data as `attribution` says: `Labour estimates by Wenmar Pro, CC BY 4.0`.
- Offline, the estimates are in the shop data file. Without it the error is `dataset_unavailable` and `details.hint` is `Run: wenmar-open data pull --shop`.

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

## Bulletins

Manufacturer communications NHTSA publishes: service bulletins, service campaigns, warranty extensions and over-the-air updates. A bulletin is not a recall, and it does not say a vehicle is affected: a bulletin lists model years. The summary is NHTSA's; the document is the manufacturer's and is linked at NHTSA, never quoted.

```bash
wenmar-open bulletins vehicle 2018_ford_f-150
wenmar-open bulletins vehicle 2023_hyundai_kona --code P0420
wenmar-open bulletins vin KM8K2CAB4PU001140 --type warranty_extension
wenmar-open bulletins get 10152456
wenmar-open bulletins search --make ford --query "water pump" --limit 5
```

- `bulletins vehicle` and `bulletins vin` list the bulletins of a model year, newest first, with `counts` by type, component, trouble code and topic. Narrow with `--type`, `--component`, `--code`, `--topic maintenance` or `--query`. `--limit` is at most 100, and `next_cursor` goes in `--cursor`.
- `bulletins get` gives one bulletin with every vehicle it lists. `bulletins search` lists all of them, narrowed by `--make`, `--model` (needs `--make`) and `--year`.
- Bulletins come from the hosted API only. Offline the error is `dataset_unavailable` and `details.hint` says to run the command again with `--online`.

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
- A block with no data is `null`. `safety` is what `vehicles safety` gives, where the crash test ratings are held, online and offline. `specs`, `fuel`, `fluids` and `tires` are what the commands of those names give, where the data is held, online and offline. `bulletins` is filled by the hosted API where its server holds bulletins, and is `null` offline. `labor` is `null` everywhere today. Offline, `recalls` is `null` when there is no shop data file. That is not an error.
- A VIN that decodes to no vehicle in the catalog gives `"vehicle": null`, every block `null` and a warning with the code `no_catalog_vehicle`.
- `vehicles model-years` lists every model year of one model, newest first, each with its vehicle id and `recalls`, the number of campaigns that may apply (`null` when no recalls are held).
- `recalls recent` lists the campaigns reported on or after `--since`, a day written like `2026-09-01`, newest first, each with `vehicles`, the vehicles it lists (at most 200; `vehicles_listed` is how many there are in all). Compare `vehicles[].vehicle_id` with your own list. `--make`, `--country`, `--limit` and `--cursor` are as in `recalls search`. Offline it needs the shop data file.
- In these answers every field is always there, as in a recall answer. `links.maker` and `lookup.maker` are filled only by the hosted API; offline they are `null`.

## Payroll deductions

Ontario employee and employer deductions for one pay, worked out on this computer by the same code as the hosted API. No data file and no connection are needed.

```bash
wenmar-open payroll deductions --frequency biweekly --gross 2400 --pay-date 2026-07-15
wenmar-open payroll deductions --frequency weekly --gross 800 --pay-date 2026-07-15 --federal-claim 1000 --provincial-claim 1000
wenmar-open payroll deductions --frequency biweekly --gross 2400 --ytd-cpp 1347.90 --ytd-ei 391.20
```

- The answer is `employee` (`federal_tax`, `provincial_tax`, `total_tax`, `cpp`, `cpp2`, `ei`, `total`), `net_pay`, `employer` (`cpp`, `cpp2`, `ei`, `total`), `note` and `pdoc_url`. It is an estimate, not a pay stub: say so.
- `--frequency` is `weekly` or `biweekly`. `--gross` is the gross pay in dollars and is the only required field.
- `--pay-date` is a day written like `2026-07-15`. Left out, it is today; a year with no held tables is `validation_failed`.
- `--federal-claim` and `--provincial-claim` are annual claim amounts. Left out, the basic claim is used.
- `--ytd-cpp`, `--ytd-cpp2` and `--ytd-ei` are year-to-date deduction amounts, for a pay that is not the first of the year. `--ytd-pensionable` is year-to-date pensionable earnings.
- `--online` asks the hosted API for the same answer instead of working it out here.
- The tool is also `wenmar_payroll_deductions`, with the same fields and no `action`.

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

This runs an MCP server on standard input and output with seven tools: `wenmar_vin` (actions `decode`, `batch`, `overview`, `fuel`, `specs`, `fluids` and `tires`), `wenmar_vehicles` (actions `years`, `makes`, `models`, `submodels`, `engines`, `search`, `entry`, `overview`, `model_years`, `safety`, `fuel`, `specs`, `fluids` and `tires`), `wenmar_recalls` (actions `vehicle`, `vin`, `campaign`, `search` and `recent`), `wenmar_bulletins` (actions `vehicle`, `vin`, `get` and `search`), `wenmar_labor` and `wenmar_payroll_deductions`. The first six take an `action`; `wenmar_payroll_deductions` takes its fields, as in `payroll deductions`. The results are the same JSON as the commands above. `wenmar-open setup claude` and `wenmar-open setup codex` print the command that registers it.

<!-- wenmar-open-skill: managed. Written by `wenmar-open setup`; the next setup replaces it. -->
