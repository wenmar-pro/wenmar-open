# wenmar-open-data

The [Wenmar Open](https://open.wenmarpro.com) data file as an npm package: NHTSA's vPIC VIN patterns and a year, make, model, trim and engine catalog, in one plain SQLite database. It is what [`wenmar-open/offline`](https://www.npmjs.com/package/wenmar-open) reads to decode VINs with no network.

```bash
npm install wenmar-open wenmar-open-data
```

The package is one file of about 167 MB (49 MB to download), a few lines that say where it is, and a script that writes it out for Cloudflare D1. It has no dependencies and runs nothing on install.

```js
import { path, dataVersion, schemaVersion } from "wenmar-open-data";

console.log(path); // .../node_modules/wenmar-open-data/wenmar-open.sqlite3
console.log(dataVersion); // 2026.09
console.log(schemaVersion); // 3
```

`wenmar-open/offline/node` finds the file by itself. Any SQLite reader can open it too; open it read-only.

## Versions

A new file is built from each monthly vPIC release. The version has three numbers:

| Part | Is | Example |
|---|---|---|
| major | the schema version: the layout of the tables | `3` |
| minor | the year and month of the data | `202609` |
| patch | a rebuild within the month | `0` |

So `3.202609.0` is the data of September 2026 and `3.202609.1` is a rebuild of it. `"wenmar-open-data": "^3.202609.0"`, which is what `npm install` writes, takes each new month and never a file with another layout. A version of `wenmar-open` reads one schema version; its README says which.

The same file is attached to each [data release](https://github.com/wenmar-pro/wenmar-open/releases) on GitHub, gzipped, for use without npm.

## Cloudflare D1

A Worker cannot carry the file, so its tables are imported into a D1 database. D1 does not take a SQLite file; it takes SQL. `wenmar-open-d1` writes the SQL:

```bash
npx wenmar-open-d1 --out ./d1-import
```

For the 2026.09 file that is 4 files of at most 50 MB, 165 MB in all, holding about 1,900 statements, none over 90,000 bytes (D1 allows 100,000). It takes a few seconds, needs Node 22.16 or later, and does not contact Cloudflare.

Import the files in order into a new, empty database:

```bash
npx wrangler d1 create wenmar-open-2026-09
for part in ./d1-import/part-*.sql; do
  npx wrangler d1 execute wenmar-open-2026-09 --remote --yes --file="$part" || break
done
npx wrangler d1 execute wenmar-open-2026-09 --remote --command "SELECT key, value FROM meta"
```

The last command should print `schema_version`, `data_version`, `vpic_release` and `built_at`.

What to know before you start:

- **It needs the Workers Paid plan.** The import writes about 2.5 million rows and then builds 17 indexes over them, and D1 counts both as rows written: about 5.6 million. The free plan allows 100,000 a day. The paid plan includes 50 million a month.
- **Each file is all or nothing.** If one fails, the database is as it was before that file and the same file can be run again. A database is unavailable while a file is being imported.
- **Import each month into a new database**, then point the Worker's binding at it and deploy, then delete the old one. The switch is one deploy, there is no time with half the data, and going back is the same step in reverse.
- The first file drops the tables it is about to create, so it can also be run over an older import.

Then bind the database and read it:

```toml
# wrangler.toml
[[d1_databases]]
binding = "DB"
database_name = "wenmar-open-2026-09"
database_id = "<the id wrangler d1 create printed>"
```

```js
import { WenmarOpenOffline, d1Store } from "wenmar-open/offline";
import wasm from "wenmar-open/offline.wasm";

let open;

export default {
  async fetch(request, env) {
    open ??= new WenmarOpenOffline({ store: d1Store(env.DB), wasm });
    return Response.json(await open.decodeVin(new URL(request.url).pathname.slice(1)));
  },
};
```

To write the SQL for another file, or only to see what would be written:

```bash
npx wenmar-open-d1 --file ./wenmar-open-2026.09.sqlite3 --out ./d1-import
npx wenmar-open-d1 --dry-run
```

## License and sources

The scripts and Wenmar Open's own lists (popular makes, aliases, name corrections, added trims and engines) are MIT licensed. The vehicle data comes from NHTSA's vPIC, a work of the United States government in the public domain. See `NOTICE.md`. Wenmar Open is not affiliated with or endorsed by NHTSA, and the data comes with no warranty.
