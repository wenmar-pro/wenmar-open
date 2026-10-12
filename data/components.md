# Components

`components.yaml` lists the parts and systems that a recall, a service bulletin, a trouble code or a job is about. It is written by hand for this project. `open-data build-shop` reads it and records, for every recall and every bulletin, which components it names; pages and the API use that to show "other bulletins about the brakes of this vehicle".

## Fields

| Field | Meaning |
|---|---|
| `id` | Lowercase words joined by hyphens: `brake-pad`, `water-pump`. It never changes once other files use it: trouble codes (`data/codes/`) and jobs (`data/jobs.yaml`) refer to components by id. |
| `name` | What a page shows: `Brake pads`. Our own words. |
| `parent` | For a part, the id of the system it belongs to. A system has no `parent`. There are two levels only: a parent must be a system. |
| `nhtsa` | NHTSA component paths, as its recall and bulletin files write them: `SERVICE BRAKES, HYDRAULIC:FOUNDATION COMPONENTS:DISC:PADS`. An entry covers that path and every path below it. Case and spacing do not matter. A comma is part of a name, so put every entry in double quotes. May be left out. |
| `tc` | Transport Canada system names, as its recall file writes them: `Brakes`. May be left out. |

## Rules

- **Two kinds of entry.** A system is one of the broad groups the sources use (`brakes`, `engine`, `electrical`). A part is something a job acts on (`brake-pad`, `alternator`, `water-pump`). Add a part when a job or a trouble code needs to point at it, even if no source names it: `timing-belt` and `oxygen-sensor` have no `nhtsa` paths because NHTSA has no such component.
- **A text is about every entry above it.** A bulletin whose component is `SERVICE BRAKES, HYDRAULIC:FOUNDATION COMPONENTS:DISC:PADS` is about `brakes` and about `brake-pad`. List a path under the most exact part it describes and let the system's shorter path cover the rest; do not repeat a part's paths under its system.
- **A path may be listed under more than one entry** when it belongs to both, as the exhaust system belongs to `engine` (through `ENGINE AND ENGINE COOLING`) and to `exhaust-emissions`.
- **Leave `UNKNOWN OR OTHER`, `OTHER` and `EQUIPMENT` alone** unless the path below them says what the part is. A recall or bulletin with no component here is still shown; it only has no component links.
- Names and ids are ours. Do not copy a parts catalogue, a labour guide or another site's component list.

## Checking a change

```bash
cargo test -p open-data --lib components     # the file loads and the examples in the tests still hold
bin/shop-data                                # rebuild, then:
```

```sql
-- the commonest component texts that no entry covers
SELECT component, COUNT(*) FROM bulletin
WHERE id NOT IN (SELECT bulletin_id FROM bulletin_component)
GROUP BY 1 ORDER BY 2 DESC LIMIT 20;
```

The build stops if an id is not lowercase words joined by hyphens, if an id is listed twice, if a name is empty, or if a `parent` is missing from the file or has a parent itself.
