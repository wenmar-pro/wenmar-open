---
name: wenmar-open
description: Decode VINs and look up vehicles by year, make, model, submodel and engine with the wenmar-open command-line tool. Use when a task involves a VIN, identifying a vehicle, or listing makes, models, trims or engines. Free, no key, no account.
---

# wenmar-open

`wenmar-open` answers vehicle questions for auto repair work: what a VIN is, and which years, makes, models, submodels and engines exist. The data is NHTSA's vPIC, the same data the hosted API at https://open.wenmarpro.com serves.

It needs no key and no account. It answers from a local data file when there is one, and from the hosted API otherwise.

## Output

- Piped, every command prints JSON on standard output: one object or one list, with no wrapper.
- `--jq EXPR` filters the JSON with a jq expression. Text results are printed without quotes.
- An error is JSON on standard error, `{ "error": { "code", "message", "details" } }`, with a non-zero exit code. `details.hint`, when present, says what to do.
- A field that is not known is left out. It is never `null` or empty.
- The tool never asks a question and never waits for input.

Exit codes: 0 success, 2 the command line was wrong, 3 `not_found`, 4 `invalid_vin` or `validation_failed`, 5 `rate_limited`, 6 the API failed or answered with something else, 10 `network`, 11 the local data file is missing or unusable, 1 anything else.

## Decode a VIN

```bash
wenmar-open vin decode KM8K2CAB4PU001140
wenmar-open vin decode KM8K2CAB4PU001140 --jq .catalog.entry.summary
wenmar-open vin decode KM8K2CAB4PU001140 --year 2023
```

The result has `year`, `make`, `model`, `trim`, `engine`, `transmission`, `safety` (ABS, TPMS and driver assistance, as the manufacturer reported them), `manufacturer` and `plant`. `catalog.entry` is the matching catalog vehicle, and `catalog.entry.id` is its stable id.

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

## Online and offline

```bash
wenmar-open data status
wenmar-open data pull
wenmar-open doctor
```

`data pull` downloads the data file (about 30 MB) so that every command works without a connection. `data status` says whether it is there. `doctor` checks the data file and the API and says which of them answers.

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

This runs an MCP server on standard input and output with two tools: `wenmar_vin` (actions `decode` and `batch`) and `wenmar_vehicles` (actions `years`, `makes`, `models`, `submodels`, `engines`, `search` and `entry`). The results are the same JSON as the commands above. `wenmar-open setup claude` and `wenmar-open setup codex` print the command that registers it.

<!-- wenmar-open-skill: managed. Written by `wenmar-open setup`; the next setup replaces it. -->
