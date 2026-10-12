# Plan: recalls, service bulletins, trouble codes and fuel data

Status: **draft for planning; sources measured on 2026-10-12.** The first draft was written without network access. The NHTSA, Transport Canada and EPA files have since been downloaded and measured; what was found is in "What the real files say" below, and it overrides the rest of this document wherever the two disagree. Items still marked **Verify** have not been checked.

Audience: a Claude Code session (or a contributor) with network access that will refine this plan and then implement it, one phase at a time.

## What the real files say (2026-10-12)

Measured from the files as downloaded that day, matched against `wenmar-open-2026.09.sqlite3`. "Exact" means `text::normalize` equality on make then model, which is steps 1 to 3 of the matcher in 0.3 without the alias and rename tables doing much.

| Source | Files | Size | Rows | Distinct records |
|---|---|---|---|---|
| NHTSA recalls | `static.nhtsa.gov/odi/ffdd/rcl/FLAT_RCL_PRE_2010.zip` and `FLAT_RCL_POST_2010.zip`; layout `RCL.txt` | 22 MB zipped, 395 MB unzipped | 327,565 | 26,394 vehicle campaigns; 160,619 campaign × make × model × year |
| Transport Canada recalls | `opendatatc.tc.canada.ca/vrdb_full_monthly.csv` | 204 MB, not compressed | about 115,000 distinct | 17,987 campaigns |
| NHTSA manufacturer communications | `static.nhtsa.gov/odi/ffdd/tsbs/TSBS_RECEIVED_<from>-<to>.zip`, seven files; layout `TSBS.txt` | 87 MB zipped, 2.3 GB unzipped | 5,877,087 | 260,220 communications; 4,348,354 communication × make × model × year |
| EPA fuel economy | `fueleconomy.gov/feg/epadata/vehicles.csv.zip` | 2 MB zipped, 22 MB unzipped | 50,407 (84 columns) | 26,508 year × make × model |
| NRCan fuel ratings | not downloaded | | | |

Every recall and bulletin file was last modified the day before, so NHTSA and Transport Canada both refresh daily, not monthly.

**Recalls, NHTSA.** Tab-separated, no header, 29 fields, plus five junk lines with one field that the parser must skip. `RCLTYPECD` = `V` is confirmed (286,415 of the rows; the rest are tires, equipment and child restraints). The advisory flags are fields 28 and 29, `DO_NOT_DRIVE` and `PARK_OUTSIDE`, holding `Yes` or `No`: 202 and 101 campaigns. There is no "park it" flag. Field 6, `MFGCAMPNO`, is the manufacturer's own campaign number. `api.nhtsa.gov/recalls/recallsByVehicle` also answers live, with no key.

**Recalls, Transport Canada.** One CSV with a header and 15 columns: `RECALL_NUMBER_NUM`, `YEAR` (written `1975.0`), `MANUFACTURER_RECALL_NO_TXT`, `CATEGORY_ETXT/FTXT`, `MAKE_NAME_NM`, `MODEL_NAME_NM`, `UNIT_AFFECTED_NBR`, `SYSTEM_TYPE_ETXT/FTXT`, `NOTIFICATION_TYPE_ETXT/FTXT`, `COMMENT_ETXT/FTXT`, `RECALL_DATE_DTE`. One campaign spans many rows, one per year × make × model. Differences from the draft:

- There is one free-text field, `COMMENT`, not a separate summary, consequence and remedy.
- There is no manufacturer column and no NHTSA campaign number.
- French text is missing ("Translation not available") for 851 campaigns.
- The categories include tires, child seats, snowmobiles, ATVs and trailers, so the file needs a category filter as the NHTSA file needs `V`.
- `recalls_variables.txt` is not at the guessed address, and the licence was not read first-hand. Both remain **Verify**.

**Bulletins.** Tab-separated, no header, 14 fields, every row well-formed. By type: 250,475 service bulletins, 2,998 service campaigns, 1,486 over-the-air, 728 warranty extensions, 364 emissions, 4,169 other. About 12,800 new communications a year. A 2016 Honda Civic has 327 and a 2018 Ford F-150 has 418, so a model-year page cannot list them all; it needs paging and filters from the start. Summaries average 220 characters. The newest archive is named for the years it holds so far (`TSBS_RECEIVED_2025-2026.zip`), so its name changes each January and `fetch` must find it by probing, because `nhtsa.gov/nhtsa-datasets-and-apis` answers scripts with `403`. One bulletin document was found at `static.nhtsa.gov/odi/tsbs/2026/MC-11038432-0001.pdf`; whether the year is always the year received and the suffix always `0001` is still **Verify**.

**Matching is harder than the draft assumed.** Rows for model years 2000 and later:

| Dataset | Make is not a light make in the catalog | Of the rest: exact | Exact or prefix |
|---|---|---|---|
| NHTSA recalls | 50% | 60% | 78% |
| Transport Canada recalls | 45% | 66% | 80% |
| Bulletins | 5% | 65% | 90% |
| EPA (all years, distinct year × make × model) | 1% | 21% | not measured |

- Half of the recall rows are heavy trucks, buses, motorhomes and trailers (Freightliner, Blue Bird, Newmar, Thor). The site indexes light vehicles only, so these stay unmatched by design and are reachable only from their campaign page.
- The sources name the trim or series where the catalog names the model: `SILVERADO 1500` against `Silverado`, `3 SERIES` against the catalog's individual models, `SIERRA DENALI` against `Sierra`. The matcher needs a base-model step (see 0.3), and a curated file alone will not close a gap of this size.
- EPA puts body and drive in the model name (`F150 Pickup 2WD`, `4Runner 4WD`), so phase 4 needs its own name parser before the shared matcher runs.
- `Scion` did not resolve as a make at all. Check whether that is a gap in `catalog_make` before phase 1.

**Size.** Loaded naively into SQLite, with make and model as text and one row per vehicle:

| Table | Size |
|---|---|
| The whole data file today | 167 MB |
| `recall`, both countries, with text | 45 MB |
| `recall_vehicle` | 9 MB |
| `bulletin`, with summaries | 90 MB |
| `bulletin_vehicle` | 140 MB |

Integer ids and year ranges in place of one row per year should cut `bulletin_vehicle` to roughly a third (not measured). Even so, bulletins alone would about double the file. This settles Q1, Q3 and Q5; see 0.1.

## Goal

Add four kinds of free shop data. Each must be reachable the same way the VIN decoder and the catalog are reachable today:

| Surface | Today | Must also cover the new data |
|---|---|---|
| Website, server-rendered, works without JavaScript, with a `.md` twin per reference page | yes | yes |
| JSON API under `/v1`, in `openapi.json` | yes | yes |
| Hosted MCP at `/mcp` and local `wenmar-open mcp` | yes | yes |
| CLI (`wenmar-open`), local data file or hosted API, identical JSON | yes | yes |
| npm client: online and offline (WASM) | yes | yes |
| `llms.txt`, `llms-full.txt`, sitemap, JSON-LD | yes | yes |

The purpose is marketing Wenmar Pro to shop owners, service advisors and technicians. Each page should answer a real counter or bay question better than the alternatives, and carry the site's single, quiet Wenmar Pro link, as `vin.html` and `tool.html` do. No new banners or pop-ups.

## Order of work

1. **Recalls**, US and Canada. The README roadmap already promises them.
2. **Service bulletins**: NHTSA manufacturer communications, covering service bulletins, warranty extensions and over-the-air updates.
3. **Trouble codes**: generic OBD-II code pages, written in our own words.
4. **Fuel and specs**: EPA fueleconomy.gov and NRCan fuel consumption ratings, matched to catalog vehicles.
5. **Engine oil and tires**: the oil grade from EPA's yearly guide files for model years 2012 on (5a), then capacity, specification and original-equipment tire fitments collected by hand from the automakers' manuals for the best-selling vehicles (5b).

Phase 0 is shared groundwork that phase 1 needs. Each phase ships on its own: data, API, website, MCP, CLI, npm, docs and tests together. A phase is not done while any surface is missing.

---

## Phase 0: groundwork every dataset needs

### 0.1 Datasets that a data file may or may not hold

Today every reader refuses a data file whose `schema_version` differs from its own (`wenmar_vin::sqlite::SCHEMA_VERSION` and `wenmar_vehicles::schema::SCHEMA_VERSION`, both `"3"`). Adding tables in four phases would mean four schema bumps and four forced rebuilds for every npm and CLI user.

**Decided by the measurements: the new datasets go in a second file, and the existing file does not change.** The file today is 167 MB and `wenmar-open-data` on npm is the same size unpacked. Recalls and bulletins would add 150 to 280 MB, and they change daily where vPIC changes monthly. So:

- `wenmar-open-YYYY.MM.sqlite3` stays as it is: VIN patterns and the catalog, schema `3`, no bump, nothing for an npm or CLI user to rebuild.
- A new file, `wenmar-open-shop-YYYY.MM.DD.sqlite3`, holds every dataset in this plan, with its own `schema_version` starting at `1` and the `datasets` meta key described below. It stores catalog ids, so its `meta` also records the `data_version` of the catalog it was matched against, and a reader refuses a pair that disagree.
- The server opens both and `ATTACH`es the second. A weekly workflow can rebuild and hot-swap the shop file alone (Q3).
- Neither file is committed to git. `.gitignore` already excludes `*.sqlite3` and `/data/build/`; the files ship as GitHub release assets and npm packages, as today. What is committed is the build code, the curated YAML and the small parser fixtures.
- Offline: the CLI gets `wenmar-open data pull --shop`, and npm gets an optional second package, `wenmar-open-data-shop`. Offline VIN decoding stays the default and stays small. Bulletins are in the offline shop file only if they add less than about 100 MB once compacted (Q5); otherwise the hosted API serves them and the offline file holds recalls, codes, fuel and oil.

The original proposal, kept for the `datasets` mechanism, which now applies to the shop file:

- ~~Bump the schema once, to `4`, in phase 1.~~
- Add a `meta` key `datasets` listing the optional datasets the file holds and each one's source release, for example `{"recalls_us":"2026-10-05","recalls_ca":"2026-10-04"}`.
- Readers accept schema 4 and check `datasets` before running a dataset's queries. A dataset missing from the file gives one clear error: in the API, a `404` with code `dataset_unavailable`; in MCP, a tool error; in the CLI, a message telling the user to run `wenmar-open data pull`. It never surfaces as a SQL failure.
- Later phases add tables and a `datasets` entry with no schema bump. Bump the shop file's schema only when an existing table changes shape.
- Readers accept either file's schema and check `datasets` before running a dataset's queries.

**Decide during planning:** whether the new tables' DDL lives in `wenmar_vehicles::schema` or in a new crate (see 0.3).

### 0.2 More than one source in a build

`open-data build` takes only the vPIC dump (`--dump`), and `fetch.rs` only knows vPIC. Generalise both:

- `open-data fetch` gets a source argument: `fetch vpic`, which stays the default and keeps today's behaviour, plus `fetch nhtsa-recalls`, `fetch tc-recalls`, `fetch nhtsa-mfr-comms`, `fetch epa-fuel` and `fetch nrcan-fuel`. Each downloads to `data/build/sources/<source>/`, prints the path and records the release date.
- `open-data build` gets optional inputs: `--recalls-us <path>`, `--recalls-ca <path>`, `--bulletins <path>`, `--fuel-us <path>` and `--fuel-ca <path>`. Each one present adds a `shape_<dataset>` step after `catalog::build`, which needs the catalog for matching, and before `meta` is written.
- Update the `mise run data` task and `.github/workflows/data-release.yml` to fetch every source. A source that fails to download fails the release, as vPIC does today. Never ship a file that silently lacks a dataset the previous release had.
- Freshness: recalls change daily, but the data file is monthly. In phase 1, show an "as of" date everywhere. Then decide whether to add a weekly workflow that rebuilds only the recall tables (see open question Q3).

### 0.3 Matching government make and model names to the catalog

Recalls, bulletins and fuel data all name vehicles in free text: `"HONDA" / "CIVIC"`, `"FORD" / "F-150"`, `"Mercedes-Benz" / "C300 4MATIC"`. One shared matcher should turn `(year, make text, model text)` into `(catalog_make.id, catalog_model.id)`.

1. Normalise with `wenmar_vehicles::text::normalize`.
2. Resolve the make through `catalog_make.norm` and `catalog_alias`.
3. Resolve the model through `catalog_model.norm` within that make, then `catalog_rename`.
4. If that fails, try the base model: the longest catalog model in that make whose normalised name is a prefix of the source's (`silverado1500` → `Silverado`), then the catalog's submodel names. Record the match as `base`, so a page can say "Silverado (all series)" instead of claiming more than the source said. Measured, this step lifts light-vehicle recall matching from about 60% to about 80%, and bulletins from 65% to 90%. A family name such as BMW `3 SERIES` covers several catalog models and needs a curated one-to-many entry.
5. If that fails, look the name up in a new curated file per source, such as `data/catalog/recall-models.yaml`, in the style of `names.yaml`.
6. If that also fails, keep the row unmatched with its original text, so it can still be found by search and on its campaign page.

Add a coverage command, `open-data coverage --data <file>`. It reports matched and unmatched rows per dataset and per make, with the most common unmatched names first, and fails when coverage falls below a threshold kept in `data/coverage.yaml`. Run it in `data-release.yml` next to `parity`. Unmatched names are how the curated files grow.

Put the matcher and the new readers in a new crate, `crates/wenmar-shopdata`, built over the existing `Source` trait so that rusqlite, turso and the WASM replay all run the same SQL, as `Catalog<S: Source>` does. Alternatively add modules to `wenmar-vehicles`. **Decide during planning;** the new crate keeps `wenmar-vehicles` focused on the catalog.

### 0.4 The wiring checklist for every new endpoint

The existing feature shows what each new query or page needs. Copy this checklist into each phase's PR description.

- [ ] SQL `const`s and a reader method generic over `S: Source`, with serde result types.
- [ ] `open-data`: fetch, shape step, `datasets` meta key, coverage numbers.
- [ ] Server `*_op(state, query)` function plus a thin `#[utoipa::path]` handler in `crates/open-server/src/api/`, with types in `api/types.rs`.
- [ ] `crates/open-server/openapi.json` regenerated with `UPDATE_OPENAPI=1 cargo test -p open-server --test it openapi`, and the path list in `tests/it/openapi.rs` updated.
- [ ] MCP:
  - `open-mcp`: `TOOL_NAMES`, `tools()`, `INSTRUCTIONS` and the tool-count test.
  - `open-server/src/mcp.rs`: a dispatch arm.
  - `wenmar-open-cli/src/request.rs`: `from_tool`.
- [ ] CLI:
  - a subcommand in `cli.rs` and a `Request` variant;
  - `local.rs` and `remote.rs` must give identical JSON, with a test asserting it;
  - `render.rs` output, plus a TUI view if it fits;
  - `skill/SKILL.md`.
- [ ] Website:
  - Askama template;
  - a route in `site::router()`;
  - a `.md` twin;
  - JSON-LD;
  - breadcrumbs;
  - a sitemap entry in `site/seo.rs`;
  - one page of the new kind added to `PAGES` in `tests/it/site_design.rs`;
  - `assert_basics`, `assert_head` and `assert_targets` passing.
- [ ] `llms.rs::text`: summary, the MCP tool list, the `## API` list and `## Reference pages`. Also `site/pages.rs` `docs()` and `data()` (data sources and licences), plus the `discovery.rs` tests.
- [ ] npm:
  - `npm run generate` to regenerate `src/schema.ts`;
  - methods in `client.ts` and `offline/client.ts`;
  - a WASM `ops.rs` function and an `engine.rs` match arm;
  - fixture rows in `test/fixtures/offline.sql` and `offline-cases.json`;
  - a README example, which `scripts/readme-examples.mjs` checks.
- [ ] README: the API list, "For AI agents", "Data sources", and the roadmap.
- [ ] `docs/integrating.md`: map NHTSA's own endpoints to ours.
- [ ] `server.json` description; `docs/deploy.md` smoke checks; CHANGELOG `[Unreleased]`.
- [ ] Attribution and disclaimer text on every page and in every API answer that needs it (see each phase).

---

## Phase 1: recalls (NHTSA and Transport Canada)

### Sources (Verify every item)

| Source | What | Licence | Notes |
|---|---|---|---|
| NHTSA recalls flat files, `FLAT_RCL_PRE_2010.zip` and `FLAT_RCL_POST_2010.zip` under `static.nhtsa.gov/odi/ffdd/rcl/` | Every recall campaign; one row per campaign × make × model × year | US government work, public domain | Tab-separated, no header, 29 fields. The layout is `RCL.txt` in the same directory. Pin the column order in code with a test. Keep only vehicle records (`RCLTYPECD` = `V`, confirmed), not tires or child seats, at first. |
| Transport Canada Vehicle Recalls Database (VRDB): the full monthly CSV at `opendatatc.tc.canada.ca`, field list in `recalls_variables.txt`; dataset page https://open.canada.ca/data/en/dataset/1ec92326-47ef-4110-b7ca-959fab03f96d | Canadian recalls, English and French | Open Government Licence – Canada: attribution required, commercial use allowed | Carry both languages: store `_en` and `_fr` text columns, even though the site is English today. Find out whether one campaign spans several rows. |

Neither government offers a public recall lookup by VIN. **Never say a recall applies to a VIN.** Say a recall *may apply* to the year, make and model, and link to the official VIN lookups: https://www.nhtsa.gov/recalls for the US and the Transport Canada recall search for Canada. Also link the automaker's own VIN lookup where the curated make data has one; add an optional `recall_lookup_url` to `data/catalog/makes.yaml`.

### Tables (draft)

```sql
CREATE TABLE recall (
  id            INTEGER PRIMARY KEY,
  source        TEXT NOT NULL,          -- 'nhtsa' | 'tc'
  campaign      TEXT NOT NULL,          -- '19V182000' | '2019-123'
  manufacturer  TEXT,
  report_date   TEXT,                   -- ISO date
  component     TEXT,                   -- source's component text
  summary_en    TEXT,                   -- NHTSA DESC_DEFECT | TC COMMENT_ETXT
  consequence_en TEXT, remedy_en TEXT,  -- NHTSA only; TC has one comment field
  summary_fr    TEXT,                   -- TC COMMENT_FTXT; NULL when 'Translation not available'
  mfr_campaign  TEXT,                   -- NHTSA MFGCAMPNO | TC MANUFACTURER_RECALL_NO_TXT (see Q4)
  units         INTEGER,               -- potentially affected, if given
  do_not_drive  INTEGER, park_outside INTEGER,              -- NHTSA fields 28 and 29
  UNIQUE (source, campaign)
);
CREATE TABLE recall_vehicle (
  recall_id  INTEGER NOT NULL,
  year       INTEGER,
  make_id    INTEGER,                  -- catalog_make.id, NULL if unmatched
  model_id   INTEGER,                  -- catalog_model.id, NULL if unmatched
  make_text  TEXT NOT NULL,
  model_text TEXT NOT NULL
);
CREATE INDEX recall_vehicle_ymm ON recall_vehicle (make_id, model_id, year);
-- Linking a US and a Canadian campaign for the same defect: see open question Q4.
```

### API

| Route | Answer |
|---|---|
| `GET /v1/vehicles/{id}/recalls?country=us\|ca` (no `country` = both) | Recalls that may apply to that model year: `{ vehicle, as_of: {us, ca}, recalls: [...], lookup: {nhtsa, tc, maker} }` |
| `GET /v1/vin/{vin}/recalls` | Decode, then the same answer as above for the decoded year, make and model, with the `decode` summary included. A VIN that does not decode to a catalog vehicle gives an empty list and a warning, not an error. |
| `GET /v1/recalls/{source}/{campaign}` | One campaign with every vehicle it lists |
| `GET /v1/recalls?make=&model=&year=&component=&q=&country=&limit=&cursor=` | List and search |

Response field names stay in English, following the API's rule that fields are only ever added.

### MCP

Add a third gateway tool, `wenmar_recalls`, with actions `vehicle` (by catalog `id`), `vin`, `campaign` and `search`. Its description must say "may apply; confirm by VIN with the manufacturer". This keeps the server's one-tool-per-noun pattern. Phases 2 and 3 add one tool each, `wenmar_bulletins` and `wenmar_codes`, for five tools in total. Fuel adds an action to `wenmar_vehicles` rather than a sixth tool.

### Website

| Page | Content |
|---|---|
| `/makes/{make}/{model}/{year}/recalls` (+ `.md`) | Recalls for that model year, US and Canada side by side, newest first. A component filter that works without JavaScript (links). An "as of" date, the official VIN lookups, and attribution. Indexed for light vehicles only, matching the existing rule. |
| `/recalls/{source}/{campaign}` (+ `.md`) | One campaign: summary, consequence, remedy and affected vehicles, each linked to its model-year page. JSON-LD as an `Article` whose subject is the vehicles. |
| `/recalls` | "Check recalls": a VIN box and the year/make/model picker, reusing the home page's form partials. Explains the "may apply" limit in one sentence. |
| Model-year page | A "Recalls" line with the count and a link, so existing traffic flows to the new pages. |
| VIN result page `/vin/{vin}` | A "Recalls that may apply" section with the count, the top items and a link. This is the service-counter moment, and the Wenmar Pro line there can mention recall checks at check-in. |
| Sitemap | A `recalls-{year}` sitemap per model year, as `models-{year}` does. |

### CLI and npm

- CLI: `wenmar-open recalls vehicle <id>`, `recalls vin <vin>`, `recalls campaign <source> <campaign>` and `recalls search ...`.
- npm: `client.recalls(id)`, `client.vinRecalls(vin)`, `client.recall(source, campaign)` and `client.searchRecalls(query)`, with offline equivalents.

### Phase 1 tests (beyond the checklist)

- A pinned-layout test for each source parser, using small committed sample files in `crates/open-data/tests/fixtures/`. Cut them from the real downloads and keep them under 50 rows each.
- A matcher test covering an alias, a rename and an unmatched row.
- A test that a VIN which decodes to a vehicle with recalls shows them on `/vin/{vin}`, in the API and over MCP, and that the three agree.
- A test that the text "may apply" and both official lookup links appear on every recall page and in every recall answer.

---

## Phase 2: service bulletins (NHTSA manufacturer communications)

### Source (Verify)

NHTSA ODI manufacturer communications: the flat file formerly named `TSBS`, renamed to manufacturer communications in 2024 (`MfrComms`, **Verify** the current name and URL), with its layout file. It holds an index record for each communication: NHTSA id, manufacturer's bulletin number, date, component, communication type (service bulletin, warranty extension, over-the-air update and others, **Verify** the codes), a summary, and the make, model and year it covers.

- **Licence:** NHTSA's index and summaries are public domain.
- **The bulletin documents themselves (PDFs) are the automakers' copyright.** Never download, store, re-host or quote them. Link to NHTSA's copy (static.nhtsa.gov) for each bulletin. **Verify** the URL pattern.

### Shape

Tables `bulletin` and `bulletin_vehicle`, mirroring the recall tables and using the same matcher.

API:

| Route | Answer |
|---|---|
| `GET /v1/vehicles/{id}/bulletins?type=&component=&q=&limit=&cursor=` | Bulletins for that model year |
| `GET /v1/vin/{vin}/bulletins` | Bulletins for the decoded vehicle |
| `GET /v1/bulletins/{nhtsa_id}` | One bulletin |

- **MCP:** `wenmar_bulletins`, with actions `vehicle`, `vin`, `get` and `search`.
- **Website:** `/makes/{make}/{model}/{year}/bulletins` (+ `.md`), filterable by type and component, with warranty extensions called out at the top because they close jobs. Add `/bulletins/{nhtsa_id}` (+ `.md`), a line on the model-year page, and a section on the VIN page.
- **CLI and npm:** as in phase 1.

**Cross-link:** a bulletin page lists recalls for the same vehicle and component, and the reverse. Use the source's component text, normalised into a small curated component list, `data/components.yaml`. Phase 3 reuses that list.

---

## Phase 3: trouble codes (generic OBD-II)

### Source and licence (the main risk in this plan)

- The code numbers (P0420) and the systems they belong to are facts.
- **The official descriptions in SAE J2012 are copyrighted and sold.** Many websites copy them. Do not copy from SAE, other code sites, or GPL code lists such as python-OBD.
- **Write every description in our own words**, as curated data in the repo: `data/codes/*.yaml`, reviewed like code. Add a CONTRIBUTING note saying where text may not come from.
- Cover generic codes only: P0xxx, P2xxx, P34xx, and the generic B, C and U ranges. Leave out manufacturer-specific codes (P1xxx and others), because their meaning depends on the make and we have no licensed source.
- **Start with the most searched codes, about 200** (catalyst, misfire, O2/AF sensor, EVAP, MAF, lean/rich, thermostat, cam/crank correlation). Grow from search-console data once pages are live.

### Record (draft)

```yaml
- code: P0420
  system: powertrain/emissions
  title: Catalyst efficiency below threshold, bank 1   # our wording
  meaning: >-
    ...plain-English explanation for a service advisor...
  common_causes: [...]          # our wording, general, not make-specific
  checks: [...]                 # what a tech typically verifies first
  components: [catalytic-converter, oxygen-sensor]   # data/components.yaml ids
  related: [P0430]
```

### Shape

- **Data:** a `code` table built from the YAML, with no network fetch.
- **API:** `GET /v1/codes/{code}` and `GET /v1/codes?q=&system=`. `GET /v1/vehicles/{id}/codes/{code}` gives the code plus the recalls and bulletins for that vehicle whose components match: "P0420 on a 2014 Camry: 1 bulletin, 0 recalls". That combination is the reason this beats generic code sites.
- **MCP:** `wenmar_codes`, with actions `get`, `search` and `vehicle`.
- **Website:** `/codes` (index by system with a search box), `/codes/{code}` (+ `.md`), and `/makes/{make}/{model}/{year}/codes/{code}`. Index the vehicle-specific pages only when they hold at least one related bulletin or recall, so the site does not fill with thin pages. Use JSON-LD `DefinedTerm`. The Wenmar Pro line here can be about writing the code onto a repair order.

---

## Phase 4: fuel and specs (EPA and NRCan)

### Sources (Verify)

| Source | What | Licence |
|---|---|---|
| EPA fueleconomy.gov `vehicles.csv` (https://www.fueleconomy.gov/feg/download.shtml) | One row per year, make, model and powertrain from 1984: displacement, cylinders, transmission, drive, fuel type, city/highway/combined MPG, EV range, and EPA's own vehicle id | Public domain (EPA data licence, **Verify**) |
| NRCan fuel consumption ratings (open.canada.ca) | Canadian L/100 km and CO₂, including battery-electric and plug-in hybrid vehicles | Open Government Licence – Canada: attribution required |

### Matching (the main effort in this phase)

- Match on year, make and model using the 0.3 matcher. Then match to a catalog engine (`catalog_engine`) using displacement and cylinders, and to a submodel where the names allow.
- Keep a match `confidence` (`model`, `engine`, `submodel`), and never show a figure more precisely than the match supports.
- Report coverage per make in `open-data coverage`.

### Shape

- **Data:** `fuel_rating(id, source, source_id, year, make_id, model_id, engine_id, submodel_id, confidence, displacement_l, cylinders, transmission, drive, fuel, city, highway, combined, unit, ev_range_km, co2_g_km, make_text, model_text, variant_text)`.
- **API:**
  - Add a `fuel` block to `GET /v1/vehicles/{id}`. This is additive, so it is allowed.
  - Add `GET /v1/vehicles/{id}/fuel` for every rating that matched.
  - In the VIN decode, add `fuel` only when the decode settles the engine. Otherwise leave it out and point to the vehicle's ratings.
- **MCP:** a `fuel` action on `wenmar_vehicles`.
- **Website:** a "Fuel economy" section on the model-year page, in both L/100 km and MPG (Canadian-first: L/100 km first), plus a Markdown twin.
- **Comparison pages are out of scope.** This data is mainly a reason for the existing pages to rank, and it fills the catalog's gaps (transmission, drive, fuel type) where vPIC is thin. Feed only matches marked `engine` or better back into catalog detail.

---

## Phase 5: engine oil, curated by hand

**Correction, 2026-10-12: the oil grade is a free download.** EPA's yearly Fuel Economy Guide files (`fueleconomy.gov/feg/epadata/<yy>data.zip`, one spreadsheet per model year) carry an `Oil Viscosity` column that the automaker fills in for each model, engine and transmission. It is filled for every row from model year 2014 on (about 800 to 1,300 rows a year), for most of 2012, for a fifth of 2011, and is absent before that. This is separate from the `vehicles.csv` file in phase 4, which does not have it.

- The grade is written loosely: 229 spellings across the years sampled (`0W20`, `SAE 0W-20`, `OW-20`, `5W/30, VW50400`). A pattern reads all but about 2% into a standard grade; the rest go in a small curated map. Keep the original text beside the parsed grade.
- It is the oil the vehicle was certified with, as the maker reported it to EPA. Label it "per EPA fuel economy certification", not "per the owner's manual".
- It gives the grade only. Capacity and the maker's specification still come from the manual by hand.
- So phase 5 splits: **5a**, the grade for model years 2012 on, built from EPA with phase 4's matching and no hand work; **5b**, capacity, specification and older vehicles, curated by hand as described below, starting with the best sellers.

The rest of this section is about 5b and the other fluids.

- **No free bulk source exists for the rest.** No government publishes fluid capacities, other fluid types or service intervals by vehicle. The compiled databases are sold: MOTOR (fluids, maintenance schedules), DataOne, Autodata, CarMD. A licence from one of them would not allow republishing openly, so licensed data belongs in Wenmar Pro, not here.
- **The free lookup sites are other people's compilations.** Oil-capacity sites and the oil makers' product finders are compiled data under terms that forbid copying, and their accuracy is unknown.
- **The facts themselves can be published.** "0W-20, 4.4 L with filter" is a fact. What cannot be copied is someone else's compilation or the manual's wording. So the licence-clean route is to read each figure from the automaker's own manual, write it in our own words, and cite the manual. This is a reading of copyright in facts, not legal advice; check it before publishing at scale.
- **The cost is the collecting.** The catalog has about 3,600 light models; each has several model years and engines, and oil specification follows the engine. A wrong oil grade on a shop site does real harm, so every record is checked by a person.

### Shape

- **Data:** `data/fluids/*.yaml`, reviewed like code, built into a `fluid` table in the shop file with no network fetch. Each record is keyed by catalog vehicle and engine and carries the grade, the specification (for example API SP, dexos1 Gen 3), the capacity with filter in litres, the manual it came from and the date checked.
- **Scope:** engine oil only, for the 100 to 200 best-selling vehicles of the last 15 years. Show nothing for a vehicle without a record. Never estimate, and never carry a figure across engines or model years without a source for each.
- **API:** an `oil` block on `GET /v1/vehicles/{id}`, and on the VIN decode only when the decode settles the engine, as phase 4 does for fuel.
- **MCP:** an `oil` action on `wenmar_vehicles`. **CLI and npm:** as in phase 4.
- **Website:** an "Engine oil" line on the model-year page, with litres first and quarts beside, and the source named.
- **Growth:** from what Wenmar Pro shops record, as battery fitment would grow (see "Not planned").

### What to add after oil, in order of value to a shop

Each is the same kind of work as oil: one fact per engine or generation, read from the manual or the under-hood label and cited.

| Fact | Worth it? | Why |
|---|---|---|
| A/C refrigerant type and charge | Yes | R-134a against R-1234yf changes the machine and the price of the job. Printed on the under-hood label. |
| Transmission fluid specification (ATF, CVT, DCT) | Yes | The wrong fluid damages the transmission, and the names are maker-specific. Type only; capacities vary by service method. |
| Coolant type | Yes | Mixing types causes damage. Type only. |
| Tire size, pressure and lug nut torque | Yes, with oil | Asked for daily. Moved into this phase; see "Tires and wheels" below. |
| Wiper blade lengths, battery group size | Maybe | Useful at the counter; battery fitment is discussed under "Not planned". |
| Brake fluid | No | Nearly always DOT 3 or DOT 4, and printed on the reservoir cap. |
| Washer fluid | No | Not vehicle-specific. |

### Specs that are free and already in the planned sources

These cost nothing to gather beyond phase 4's matching, so show them before curating anything by hand:

- **EPA `vehicles.csv`:** recommended fuel grade (regular, midgrade or premium: 9,591 of the 21,774 rows from 2010 on are premium), transmission type and speeds, drive, turbocharger or supercharger, start-stop, hybrid or plug-in type, electric range and 240 V charge time (about 2,000 rows), and size class.
- **vPIC**, already in the data file: the decode's safety and driver-assistance fields, where they are not shown yet.
- **NHTSA crash-test ratings:** `api.nhtsa.gov/SafetyRatings` answers with no key. A candidate for a later phase. **Verify** whether there is a bulk file.
- **Transport Canada's Canadian Vehicle Specifications** (dimensions and curb weight) is believed to be open data. **Verify** that it exists, its licence and its coverage; the open.canada.ca API refused scripted requests on 2026-10-12.

### Tires and wheels

Tire data comes in two parts, and only one is free.

- **Free now, from vPIC, already in the data file:** rim diameter front and rear (elements 119 and 120, present in about 1,450 vPIC schemas) and the tire-pressure monitor type (element 168, about 4,600 schemas). Coverage is partial, and they are known only from a VIN decode. Show them in the decode and in `specs` where present. This needs no new source and can ship with phase 1.
- **Not free: tire size, load and speed rating, placard pressure, lug nut torque, bolt pattern.** They are printed on the door placard and in the manual, and no government publishes them in bulk. The fitment databases are sold. So this is hand curation like oil, with one difference: a model year has several answers, one per trim or wheel option. Store a list of fitments and say which trims each covers; never pick one.

Curate tires in the same pass as oil, since the same manual is open: `data/tires/*.yaml`, built into a `tire_fitment` table. For each vehicle record every original-equipment size with its trims, the front and rear cold pressures, the lug nut torque and the bolt pattern, with the source and the date checked.

## Maintenance schedules: link, do not copy

The automakers do publish their schedules, and this site can point to them. Copying them in bulk is what does not work:

- **There is nothing uniform to fetch.** Each maker publishes differently: PDF manuals, owner portals that ask for a VIN or a sign-in, or nothing online for older vehicles. Each would need its own collector, and the makers' site terms generally forbid automated collection. **Verify** the terms of any site before fetching from it.
- **A schedule is a table, not a fact.** One interval is a fact. The maker's whole schedule, with its selection of items and its wording, is closer to the maker's own work, so reproducing it wholesale carries more risk than quoting an oil grade.
- **It is not one answer per vehicle.** Intervals differ by engine, by drivetrain, by "normal" against "severe" use, and between the US and Canada. Many vehicles since about 2010 have no fixed oil interval at all: the car's oil-life monitor decides.

What is practical:

1. **Link to the maker's own schedule.** Add an optional `maintenance_url` to `data/catalog/makes.yaml`, as phase 1 adds `recall_lookup_url`, and show "Maintenance schedule from Honda" on every model-year page. About 40 makes, checked by hand, no copying.
2. **Curate a few headline intervals by hand** once phase 5 exists: oil, and timing belt where the engine has one, each citing the manual and stating the market and the use it applies to. Say "follows the oil-life monitor" where that is the maker's answer.

Step 1 can ship with phase 1. Step 2 is a later addition to phase 5.

### Linking to the manuals themselves

Linking to an automaker's PDF is allowed and useful. Collecting the links is the difficulty, measured on 2026-10-12:

- **The automakers' sites refuse scripts.** Ford's manual and maintenance pages and Mopar's answered `403` to a scripted request; Honda's and Toyota's owner pages answered. So the links cannot be gathered automatically, and an automatic check that a link still works will be unreliable for some makes.
- **PDF addresses are not predictable and do move.** A manual's address carries a version and a print date. Each one is found by hand, per model year.

So link in three levels, cheapest first:

1. **Per make:** the owner-manual page and the maintenance-schedule page, in `makes.yaml` (`manual_url`, `maintenance_url`). About 40 makes. Ships with phase 1.
2. **Per model year, where the maker's address has a pattern:** a template in `makes.yaml`, for example Honda's `owners.honda.com/vehicles/information/{year}/{model}/manuals`, filled from the catalog and checked by hand for a sample. **Verify** each maker's pattern before using it.
3. **Per model year, by hand:** a direct PDF link recorded in the same YAML record as the oil and tire figures in phase 5b, since the manual is already open. It carries the date checked. A monthly job requests every stored link and reports the failures; it does not fail the build, because some makes refuse it.

Never download, store or re-host a manual.

### Maintenance changes that NHTSA does host

Some bulletins are about maintenance, and NHTSA hosts the document, so the site can link straight to it:

- About 190 bulletin summaries mention a maintenance schedule, 150 scheduled maintenance, 150 a maintenance interval, 190 a service interval, 115 a timing belt and 100 the oil-life monitor. 345 of them cover vehicles from model year 2015 on. Examples: a timing belt interval change for a diesel engine, a document listing transmission fluid types and service intervals, a fault that resets the oil-life monitor.
- Tag these at build time with a `maintenance` topic from those phrases, and show them first on the model-year page under "Maintenance updates from the manufacturer". For a shop this is the part of a schedule that changed after the manual was printed.
- The model-year page then has, in one place: the link to the maker's schedule, the oil grade (5a), the curated intervals where they exist (5b), and the maintenance bulletins.

## More that the same files hold (measured 2026-10-12)

- **Trouble codes inside bulletin summaries.** About 24,500 of the 260,220 bulletins name at least one code in their summary (about 12,300 of the 89,600 since 2020), and about 6,900 distinct codes appear, P0300, P0456 and P0420 among the commonest. Extract them at build time into `bulletin_code(bulletin_id, code)`. This gives phase 3 a direct link from a code on a vehicle to the bulletins that name it, which is stronger than matching by component, and it reaches manufacturer-specific codes we cannot describe ourselves. Add `code=` to the bulletin search routes in phase 2.
- **Bulletin summaries are searchable by symptom:** "noise" appears in about 17,600, "leak" in 9,800, "software" in 22,500, "warranty" in 7,900. The `q=` search on bulletins is the main way a technician will use them.
- **Recall fields the draft left out,** for rows since 2010: the manufacturer's own campaign number (81%), which is what a dealer looks a recall up by; the date owners were notified (98%); the build-date range (36%), which can be compared with the date on the door sticker; and the affected part number (67%). Add `mfr_campaign`, `owner_notified`, `built_from`, `built_to` and `part_number` to the recall tables and answers.
- **Complaints and investigations** sit beside the recall files (`static.nhtsa.gov/odi/ffdd/cmpl/` and `/inv/`, both present). Complaint counts by component give "what owners report most on this model year", and an open investigation is an early warning before a recall. Not measured yet. Candidates for a phase after 2, as counts and investigation summaries only, never the owners' free text.

## Phase 6 (future): labour times from Wenmar's own estimates

Not scheduled. Planned now so that the earlier phases leave room for it.

### What it is, and what it must not be

- An estimate of the hours a job takes on a vehicle, made by Wenmar: written by people who do the work, and later measured from what Wenmar Pro shops record.
- **Never taken from a labour guide.** MOTOR, Mitchell, ALLDATA and the automakers' warranty times are compiled and sold. Nobody who writes an estimate may have a guide open, and the CONTRIBUTING note for phase 3 is extended to say so. The manufacturers' labour allowances that appear in bulletins (about 800 summaries mention a labour time) are not harvested either.
- Published as Wenmar's data under a licence that requires attribution. *Decided.* Proposal: CC BY 4.0 for the labour data, with the attribution line stated in `NOTICE.md`, on every labour page and in every labour answer (`"attribution": "Labour estimates by Wenmar Pro, CC BY 4.0"`). The code stays MIT. The labour tables therefore ship in their own dataset entry with its own `licence` value in the shop file's `datasets` key, so a reader can tell which licence covers which table. **Verify** the licence choice with whoever advises Wenmar on licensing.

### Two sources, one table

| Basis | Where it comes from | When |
|---|---|---|
| `expert` | Curated YAML, `data/labor/*.yaml`, reviewed like code. One estimate per job and vehicle class, with overrides for particular models, generations and engines. | First. Works with no shop data. |
| `observed` | Wenmar Pro repair orders: the hours billed or clocked for a job on a vehicle, reduced to a median and a range. Only from shops that opt in. Only published when enough shops and jobs stand behind a figure (proposal: at least 5 shops and 20 jobs). No shop, customer, VIN or date leaves Wenmar Pro; this repository receives the finished table only. | When Wenmar Pro records jobs against the job list below. |

An `observed` figure replaces the `expert` one for the same job and vehicle once it passes the threshold. The answer always says which it is.

### The job list comes first

Labour times are only comparable if every shop means the same thing by a job. So the first piece is a job list, `data/jobs.yaml`:

```yaml
- id: brake-pads-front
  name: Replace front brake pads
  system: brakes
  components: [brake-pad]          # ids from data/components.yaml (phase 2)
  includes: [remove wheels, replace pads, clean and lubricate slides, road test]
  excludes: [rotor replacement, caliper replacement, fluid flush]
  related: [brake-pads-and-rotors-front]
```

- Start with the 50 to 100 jobs that make up most repair orders: brakes, oil service, battery, belts, spark plugs, struts, wheel bearings, alternator, starter, water pump, timing belt.
- **Wenmar Pro should adopt the same job ids on its repair orders as early as it can**, before this phase is built. That is what makes `observed` possible later, and it is the one dependency outside this repository.
- `data/components.yaml`, created in phase 2, is shared by recalls, bulletins, trouble codes and jobs. Design its ids in phase 2 with jobs in mind: components a job acts on (`brake-pad`, `water-pump`), not only NHTSA's broad groups.

### Table (draft)

```sql
CREATE TABLE job (id TEXT PRIMARY KEY, name TEXT, system TEXT, includes TEXT, excludes TEXT);
CREATE TABLE labor_estimate (
  job_id      TEXT NOT NULL,
  scope       TEXT NOT NULL,     -- 'class' | 'model' | 'generation' | 'engine'
  class       TEXT,              -- e.g. 'compact-car', 'half-ton-pickup'
  make_id     INTEGER, model_id INTEGER, year_from INTEGER, year_to INTEGER, engine_id INTEGER,
  hours_low   INTEGER, hours_typical INTEGER, hours_high INTEGER,   -- hundredths, as shop_math::Hours
  basis       TEXT NOT NULL,     -- 'expert' | 'observed'
  sample_jobs INTEGER, sample_shops INTEGER,                        -- observed only
  notes       TEXT, updated TEXT NOT NULL
);
```

A lookup takes the narrowest scope that has a figure: engine, then generation, then model, then class. The answer names the scope it used, so "typical for a half-ton pickup" is never shown as if it were measured on that truck. The vehicle classes can come from EPA's size class in phase 4.

### Surfaces

- **API:** `GET /v1/jobs`, `GET /v1/jobs/{job}`, `GET /v1/vehicles/{id}/labor`, `GET /v1/vehicles/{id}/labor/{job}`, and the same under `/v1/vin/{vin}`. A `labor` block in `overview`.
- **MCP:** `wenmar_labor`, with actions `jobs`, `vehicle` and `vin`.
- **Website:** `/jobs`, `/jobs/{job}`, and `/makes/{make}/{model}/{year}/labor/{job}`: "how long does it take to replace front brake pads on a 2018 F-150". Index a vehicle page only when its figure is narrower than class scope. Each page links the recalls, bulletins and trouble codes for the same components.
- **A price, not only hours:** `shop-math` already has `Hours` and the labour rate arithmetic, and `data/rates/canada.toml` has the taxes. The page can take the visitor's shop rate and show hours × rate with tax, using the existing calculators. No rate is assumed.

```json
"labor": [{
  "job": "brake-pads-front", "name": "Replace front brake pads",
  "hours": { "low": 0.8, "typical": 1.0, "high": 1.4 },
  "basis": "expert", "scope": "class", "class": "half-ton-pickup",
  "sample": null, "updated": "2027-02-01"
}]
```

Every page and answer carries: "An estimate, not a quote. Rust, seized fasteners and added equipment change the time."

### What the earlier phases must leave room for

1. The shop file's `datasets` key (0.1) already lets `labor` arrive without a schema bump.
2. `overview` returns `"labor": null` from phase 1, so a client written now does not break later.
3. `components.yaml` ids are chosen in phase 2 with jobs in mind (above).
4. Routes under `/makes/{make}/{model}/{year}/` keep `labor` free.

## Easy wins from files beside the ones already planned (measured 2026-10-12)

All are US government files, public domain, with the same make, model and year text the shared matcher already handles. In order of value for the effort:

| Source | File | What it adds | Size | Where it fits |
|---|---|---|---|---|
| EPA Fuel Economy Guide, yearly | `fueleconomy.gov/feg/epadata/<yy>data.zip` | Oil grade (phase 5a). Also number of gears, cylinder deactivation and its trade name, variable valve timing, valves per cylinder, passenger and luggage volume, smog rating, and for hybrids the battery type and voltage and motor power | Under 1 MB a year, about 160 columns | Phase 4, with `vehicles.csv` |
| NHTSA crash ratings | `static.nhtsa.gov/nhtsa/downloads/Safercar/Safercar_data.csv` | Overall, frontal, side and rollover stars; which safety and driver-assistance features are standard or optional; curb weight for 7,037 rows | 10 MB, 17,313 rows, model years 1990 to 2026; 5,484 have an overall rating | A small phase after 1. A `safety` block and a section on the model-year page |
| NHTSA investigations | `static.nhtsa.gov/odi/ffdd/inv/FLAT_INV.zip` | Defect investigations with subject, summary, dates opened and closed, and the recall each led to (`CAMPNO` joins to the recall table). 5,349 investigations, 222 open | 4 MB zipped | With phase 1: "under investigation" on a vehicle, and "investigated as" on a recall |
| NHTSA complaints | `static.nhtsa.gov/odi/ffdd/cmpl/FLAT_CMPL.zip` | 2,255,013 owner complaints, about 100,000 a year, each with a component, mileage at failure, and crash and fire flags. A 2016 Honda Civic has 445 under steering, 120 electrical, 95 engine | 374 MB zipped, 1.6 GB unzipped | After phase 2. Store counts only, by vehicle and component, with the median mileage: a few megabytes. Never the owners' text, names, cities or VINs |
| EPA emissions | `fueleconomy.gov/feg/epadata/emissions.csv.zip` | Emission standard and smog score per EPA vehicle id | 53,022 rows | Phase 4; joins to `vehicles.csv` by `id` |

Complaint counts answer "what goes wrong on this one", which neither recalls nor bulletins do, and they give the trouble code pages a ranking of likely components. They are the largest file here and should be reduced to counts in the build, not shipped.

## Further candidates (2026-10-12)

Ideas beyond the six phases, grouped by how they would be made. None is scheduled. "Checked" means the source answered on that day; nothing here was measured further.

### From data the plan already gathers: views, not new sources

| Idea | What a shop does with it |
|---|---|
| **Parts-ordering card for a VIN.** One block with what a parts counter asks for: engine and its VIN 8th character (already in `catalog_engine.vin8`), displacement, drive, transmission, body, model year and plant, emission standard and sales area (EPA `emissions.csv`), build-date split where a recall gives one. A copy button and a `GET /v1/vin/{vin}/parts-card`. | Reads it to the supplier or pastes it into an order, and gets the right part the first time. |
| **Recall completion rate.** NHTSA's quarterly reports (`static.nhtsa.gov/odi/ffdd/rcl/FLAT_RCL_Qrtly_Rpts.zip`, 1.5 MB, checked) give how many vehicles in each campaign have been remedied. | "Only 61% of these have been fixed" tells an advisor how likely the car in the bay is still open. Add to phase 1. |
| **Known problems by mileage.** From complaint counts: the components owners report most on a model year, and the mileage at which they fail. | A used-vehicle inspection list, and what to look at when a vehicle comes in at 120,000 km. |
| **Sibling vehicles.** Models that share a recall campaign or a bulletin are usually the same platform (Silverado and Sierra, Escape and Corsair). Derive "shares campaigns with" from the recall and bulletin tables. | A hint about where else a part or a fix applies. Label it as derived, not as fitment. |
| **Manufacturer VIN guides.** vPIC's `GetParts?type=565` lists the VIN-deciphering documents each maker files with NHTSA (496 in the first five months of 2026, checked), each with a link. | The maker's own explanation of a VIN position when the decode is not enough. |

### Curated by hand, small and stable

| Idea | Notes |
|---|---|
| **Per-make link directory.** In `makes.yaml`: recall lookup, owner manuals, maintenance schedule, the maker's service-information site and its short-term subscription, the parts catalogue, and the customer line. About 40 makes. | The cheapest useful thing here. Independent shops often do not know a maker sells a day of factory service information. Extends phase 1's link fields. |
| **Fluid and specification glossary.** Pages in our own words for oil specifications (API, ILSAC, dexos, the European maker approvals), transmission fluid families and which supersede which, coolant chemistries, brake fluid grades, refrigerants. | Makes the oil grade from phase 5a readable, and stops a wrong substitution. Each fluid field links to its page. |
| **Canadian rules for repair shops, by province.** Written-estimate and authorisation rules, the limit on exceeding an estimate, the warranty a repair must carry, what must be on an invoice, return of old parts, winter-tire and inspection requirements, and the environmental fees on oil, filters, batteries and tires. Kept like `data/rates/canada.toml`: every rule with its source and the date read. | High value to an owner and close to what the site already does with taxes and tire fees. Not legal advice; say so. |
| **Battery group size table.** The dimensions and terminal layout of each group size are facts. Which group fits which vehicle is not available (see "Not planned"). | A lookup for "will a group 35 fit where a 24F was". |

### Generated: tools and reference tables

These need no data source, only arithmetic or public physical constants, and fit the existing `/tools` pages and `shop-math`:

- Tire size comparison: diameter, speedometer error, clearance change between two sizes.
- Tire date code and DOT plant code reader. **Verify** that NHTSA publishes the plant code list in bulk.
- Refrigerant pressure-temperature chart for R-134a and R-1234yf.
- Battery state of charge from open-circuit voltage, with temperature correction.
- Torque conversion, and a general fastener torque table by size and grade for when no specification is known, labelled as general.
- Wire gauge for a given current and length.
- A reader for fuel trims and for OBD-II readiness monitors: what each monitor needs before it will run. Written in our own words, as phase 3 is.
- Shop business: effective labour rate, technician efficiency and productivity, break-even hours a day, parts markup against margin. Some exist already; check `shop-math` before adding.

### From Wenmar Pro, as labour times will be

Each needs shops to opt in and a minimum sample before anything is published, exactly as phase 6 describes:

- Labour rates by province and by shop type.
- The parts that go with a job: "front pads on this truck usually also take hardware and one rotor in three".
- Battery group size and tire size actually fitted, by vehicle. This is the licence-clean route to the fitment data nobody publishes.
- Which jobs follow which: what a vehicle usually needs at a given mileage, from real orders.

### Looked at and set aside

- **Parts catalogues, part numbers, supersessions and interchange.** The aftermarket's vehicle and part standards (ACES and PIES) and interchange data are licensed. Not available to publish.
- **Paint codes, bulb sizes, key and module programming procedures, wiring diagrams.** No free source; proprietary or compiled.
- **Wikidata for generations and platforms.** Free to use, but uneven. The derived "sibling vehicles" view above gives most of the value from data we can check.

## Still to plan, across all phases

1. **Corrections.** Curated data will have mistakes. Every page showing a curated figure needs a "report an error" link that opens a prefilled issue, and each YAML record needs a named reviewer. Plan this before phase 3, the first curated phase.
2. **A review rule for curated data.** Who may approve a fluid, tire or labour record, and what evidence the pull request must show. Write it into CONTRIBUTING before phase 3.
3. **A licence table.** With public-domain, Open Government Licence – Canada and CC BY data in one file, `/data` and `NOTICE.md` need one table: dataset, source, licence, required attribution. Each API answer carries the attribution its data needs.
4. **Request limits.** *Decided 2026-10-12:* keep the existing limits for the new routes. No new limit and no exemption for Wenmar Pro's traffic. Revisit once `overview` and `recalls/recent` have real traffic.
5. **French.** Text is stored from phase 1 (Q6). Serving French pages is still undecided.
6. **Order of the small phases.** Investigations and completion rates ride with phase 1; crash ratings follow it; complaints follow phase 2. Confirm this when phase 1 is scheduled.

## API additions for an app showing one vehicle

Wenmar Pro's vehicle page wants everything about one vehicle in one request. The per-dataset routes in each phase stay; these sit over them.

A catalog vehicle id is already the year, make and model: `2018_ford_f-150`. `GET /v1/vehicles/years?make=ford&model=f-150` already lists the years, and `GET /v1/vehicles/2018_ford_f-150` already answers for one. So "an F-150 by year" needs no new address, only more in the answer.

| Route | Answer | Phase |
|---|---|---|
| `GET /v1/vehicles/{id}/overview` | Everything held for that model year in one answer: `specs`, `fuel`, `fluids`, `tires`, the counts and newest items of `recalls` and `bulletins`, and `links`. Lists that depend on the engine or trim come back as lists. | 1, growing each phase |
| `GET /v1/vin/{vin}/overview` | The same, narrowed by the decode: when the VIN settles the engine, `fluids` and `fuel` hold one entry instead of a list. This is the route for a shop that has the VIN. | 1 |
| `GET /v1/vehicles/{id}/specs`, `/fluids`, `/tires` | One block each, for a caller that wants only that. | 4, 5 |
| `GET /v1/models/{make}/{model}` | Every model year of one model, each with its id and its recall and bulletin counts. One request draws a "by year" table. | 1 |
| `GET /v1/recalls/recent?since=&country=&make=` | Campaigns added since a date, newest first, with the vehicles each lists. Lets an app check its customers' vehicles against new recalls without fetching every vehicle again. | 1 |
| `POST /v1/vin/batch` | Add an optional `include=recalls` that returns the recall count per VIN. | 1 |
| `GET /v1/meta` | Add `datasets`, with the "as of" date of each. | 1 |

Rules for every answer that carries the new data:

- A block the file has no data for is `null`, and a list with no entries is `[]`. A caller can tell "nothing known" from "none exist": `recalls.count: 0` with an `as_of` date means none were found, and `fluids: null` means nobody has recorded it.
- Every curated fact carries `source` and `checked`. Every matched fact carries `match` (`exact`, `base` or `engine`).
- Recall blocks always carry `may_apply: true` and the `lookup` links.
- Units are metric first, with the US figure beside it, in separate fields. No strings that need parsing.

Shape of `GET /v1/vin/{vin}/overview`. The values are invented to show the shape; none is a real figure.

```json
{
  "vehicle": { "id": "2018_ford_f-150", "year": 2018, "make": "Ford", "model": "F-150" },
  "decode": { "vin": "1FTEW1EP5JFA00000", "engine": "2.7L V6 turbo", "engine_settled": true },
  "specs": {
    "body": "Pickup", "drive": "4-Wheel Drive",
    "transmission": { "type": "Automatic", "speeds": 10 },
    "fuel_grade": "Regular Gasoline",
    "forced_induction": "turbo", "start_stop": true,
    "tpms": "direct", "rim_diameter_in": { "front": 18, "rear": 18 },
    "source": ["vpic", "epa"], "match": "engine"
  },
  "fuel": [{
    "source": "epa", "source_id": "39142", "match": "engine",
    "l_100km": { "city": 12.4, "highway": 9.8, "combined": 11.2 },
    "mpg": { "city": 19, "highway": 24, "combined": 21 }
  }],
  "fluids": {
    "engine_oil": {
      "grade": "5W-30", "specification": "Ford WSS-M2C946-B1",
      "capacity_l": 5.7, "capacity_qt": 6.0, "with_filter": true,
      "source": "2018 F-150 Owner's Manual", "checked": "2026-11-02"
    },
    "transmission": null, "coolant": null, "refrigerant": null
  },
  "tires": [{
    "size": "275/65R18", "trims": ["XLT", "Lariat"],
    "pressure_kpa": { "front": 240, "rear": 240 }, "pressure_psi": { "front": 35, "rear": 35 },
    "lug_torque_nm": 204, "bolt_pattern": "6x135",
    "source": "2018 F-150 Owner's Manual", "checked": "2026-11-02"
  }],
  "recalls": {
    "may_apply": true, "count": { "us": 7, "ca": 5 }, "as_of": { "us": "2026-10-11", "ca": "2026-10-11" },
    "do_not_drive": false, "park_outside": false,
    "newest": [{
      "source": "nhtsa", "campaign": "24V000000", "date": "2024-03-01",
      "component": "Electrical system", "summary": "...", "match": "exact",
      "url": "https://open.wenmarpro.com/recalls/nhtsa/24V000000"
    }],
    "lookup": { "nhtsa": "https://www.nhtsa.gov/recalls", "tc": "...", "maker": "..." }
  },
  "labor": null,
  "bulletins": {
    "count": 418, "by_type": { "service_bulletin": 401, "warranty_extension": 2, "over_the_air": 0 },
    "as_of": "2026-10-11", "newest": [{ "nhtsa_id": 11000000, "date": "2026-09-30", "type": "service_bulletin", "component": "Engine", "summary": "..." }]
  },
  "links": { "maintenance": "https://...", "page": "https://open.wenmarpro.com/makes/ford/f-150/2018" },
  "warnings": []
}
```

`GET /v1/vehicles/{id}/overview` has the same fields without `decode`; there `fluids.engine_oil` is a list with one entry per engine, each naming its `engine`.

## Not planned, and why

- **Battery fitment** (which BCI group size fits which vehicle):
  - No free, redistributable source covers fitment by vehicle. The BCI data book is sold, and the battery makers' lookup tools are their own compiled data under site terms that forbid copying.
  - The group sizes themselves (dimensions, terminal layout) are facts and could be published.
  - A licence-clean route to fitment would be shop-contributed data. A Wenmar Pro shop records the group it installed on a VIN, and the counts by year/make/model/engine are published openly once enough shops agree. That is a product decision, not a data download. It would also be a strong marketing story ("built from real installs").
- **Repair procedures and other publishers' labour guides:** these are proprietary (OEM, MOTOR, Mitchell, ALLDATA). Wenmar's own labour estimates are phase 6. Fluids are phase 5, and maintenance schedules have their own section above.
- **Complaints:** they come from the same NHTSA ODI downloads and could be added cheaply after phase 2, as counts by component only, never the owners' free text. Left out of this plan to keep its scope down.
- **Recall by VIN:** neither government publishes it (see phase 1).

## Open questions for the planning session

- **Q1.** *Answered by the measurements:* one separate shop file holding all the new datasets, with a `datasets` meta key; the existing file is not bumped (0.1).
- **Q2.** Should the readers and matcher go in a new crate `wenmar-shopdata` (0.3), or in `wenmar-vehicles`?
- **Q3.** How fresh must recalls be? Both sources refresh daily. With the shop file separate (0.1), a weekly rebuild that hot-swaps it on the hosted service is cheap; check how `docs/deploy.md` ships data. Still to decide: weekly or daily.
- **Q4.** How should a US campaign and a Canadian campaign for the same defect be linked? Manufacturer plus component plus close dates is a heuristic. Show them separately in phase 1 and link them only if a reliable key exists. Transport Canada does not record the NHTSA campaign number. Both record the manufacturer's own campaign number (NHTSA `MFGCAMPNO`; Transport Canada `MANUFACTURER_RECALL_NO_TXT`, present on 13,204 of 17,987 campaigns), which is the only candidate key. **Verify** how often the two agree before relying on it.
- **Q5.** *Answered by the measurements:* recalls add about 55 MB and bulletins about 230 MB before compaction, against 167 MB today. They ship as an optional second package, `wenmar-open-data-shop` (0.1). *Decided:* compact `bulletin_vehicle` and measure; if bulletins still add more than about 100 MB, they are served by the hosted site and API only and left out of the offline shop file.
- **Q8.** *Decided:* each of the 260,220 bulletins gets its own indexed `/bulletins/{nhtsa_id}` page. Each has a summary of only about 220 characters and a link to a PDF we may not host, so the page must carry more than the summary to be worth indexing: the vehicles it covers, the type, the components, and the recalls and other bulletins for the same vehicle and component. Split the sitemap by year received, and watch search-console coverage after launch.
- **Q6.** Should French text be shown? Store it from phase 1; serving French pages is a separate decision.
- **Q7.** How should the Wenmar Pro link read on each new page type? It should be one link, worded for the moment, like the VIN page's.

## Done means

For each phase:

- `bin/check` passes: fmt, clippy `-D warnings`, and the whole workspace's tests.
- `bin/js` passes.
- `openapi.json`, `schema.ts` and the README examples are in sync.
- `open-data coverage` meets its threshold, and the data-release workflow builds and releases the file with the new dataset.
- On a deployed copy, the new data can be reached:
  - in a browser, with JavaScript off;
  - with `curl`;
  - from Claude, through the `/mcp` connector;
  - from `wenmar-open` online and offline;
  - from the npm client, online and offline.
- The phase's attribution, "as of" date and disclaimer appear wherever its data does.
