# Plan: recalls, service bulletins, trouble codes and fuel data

Status: **draft for planning.** Written without network access to the data sources, so every file layout, field name and licence term below must be checked against the real files before code depends on it. Steps that need that check are marked **Verify**.

Audience: a Claude Code session (or a contributor) with network access that will refine this plan and then implement it, one phase at a time.

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

Phase 0 is shared groundwork that phase 1 needs. Each phase ships on its own: data, API, website, MCP, CLI, npm, docs and tests together. A phase is not done while any surface is missing.

---

## Phase 0: groundwork every dataset needs

### 0.1 Datasets that a data file may or may not hold

Today every reader refuses a data file whose `schema_version` differs from its own (`wenmar_vin::sqlite::SCHEMA_VERSION` and `wenmar_vehicles::schema::SCHEMA_VERSION`, both `"3"`). Adding tables in four phases would mean four schema bumps and four forced rebuilds for every npm and CLI user.

Proposal:

- Bump the schema once, to `4`, in phase 1.
- In the same change, add a `meta` key `datasets` listing the optional datasets the file holds and each one's source release, for example `{"recalls_us":"2026-10-05","recalls_ca":"2026-10-04"}`.
- Readers accept schema 4 and check `datasets` before running a dataset's queries. A dataset missing from the file gives one clear error: in the API, a `404` with code `dataset_unavailable`; in MCP, a tool error; in the CLI, a message telling the user to run `wenmar-open data pull`. It never surfaces as a SQL failure.
- Later phases add tables and a `datasets` entry with no schema bump. Bump only when an existing table changes shape.
- Update everything a bump touches (listed in `docs/releasing.md`): both `SCHEMA_VERSION` consts, the test fixtures in `crates/open-server/tests/it/common.rs`, the `clients/js/package.json` peer range `wenmar-open-data` `^3` → `^4`, `clients/data`, and the CHANGELOG note that older files must be rebuilt.

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
4. If that fails, look the name up in a new curated file per source, such as `data/catalog/recall-models.yaml`, in the style of `names.yaml`.
5. If that also fails, keep the row unmatched with its original text, so it can still be found by search and on its campaign page.

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
| NHTSA recalls flat file, `FLAT_RCL*.zip`, linked from https://www.nhtsa.gov/nhtsa-datasets-and-apis | Every recall campaign; one row per campaign × make × model × year | US government work, public domain | Tab-separated, no header. The field layout is in `RCL.txt` beside the zip. Read it and pin the column order in code with a test. Keep only vehicle records (`RCLTYPECD` = `V`, **Verify**), not tires or child seats, at first. |
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
  summary_en    TEXT, consequence_en TEXT, remedy_en TEXT,
  summary_fr    TEXT, consequence_fr TEXT, remedy_fr TEXT,   -- TC only
  units         INTEGER,               -- potentially affected, if given
  park_it       INTEGER, park_outside INTEGER,              -- NHTSA flags, Verify
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

## Not planned, and why

- **Battery fitment** (which BCI group size fits which vehicle):
  - No free, redistributable source covers fitment by vehicle. The BCI data book is sold, and the battery makers' lookup tools are their own compiled data under site terms that forbid copying.
  - The group sizes themselves (dimensions, terminal layout) are facts and could be published.
  - A licence-clean route to fitment would be shop-contributed data. A Wenmar Pro shop records the group it installed on a VIN, and the counts by year/make/model/engine are published openly once enough shops agree. That is a product decision, not a data download. It would also be a strong marketing story ("built from real installs").
- **Maintenance schedules, fluid capacities, labour times, lug torque and bolt patterns:** these are proprietary (OEM, MOTOR, Mitchell, ALLDATA). The data page can say that Wenmar Pro brings licensed data for these.
- **Complaints:** they come from the same NHTSA ODI downloads and could be added cheaply after phase 2, as counts by component only, never the owners' free text. Left out of this plan to keep its scope down.
- **Recall by VIN:** neither government publishes it (see phase 1).

## Open questions for the planning session

- **Q1.** Is one schema bump plus a `datasets` meta key acceptable (0.1), or should every dataset be a separate optional data file?
- **Q2.** Should the readers and matcher go in a new crate `wenmar-shopdata` (0.3), or in `wenmar-vehicles`?
- **Q3.** How fresh must recalls be? Monthly with the data release, or a weekly recall-only rebuild? The hosted service could also hot-swap a newer data file; check how `docs/deploy.md` ships data.
- **Q4.** How should a US campaign and a Canadian campaign for the same defect be linked? Manufacturer plus component plus close dates is a heuristic. Show them separately in phase 1 and link them only if a reliable key exists. **Verify** whether Transport Canada records the NHTSA campaign number.
- **Q5.** How large does the data file get? Measure after each phase. The npm `wenmar-open-data` package and the WASM offline mode load the whole file. If recalls and bulletins add a lot, consider an optional second package, `wenmar-open-data-full`, with the shop datasets.
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
