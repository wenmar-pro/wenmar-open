# wenmar-open-data-shop

The [Wenmar Open](https://open.wenmarpro.com) shop file as an npm package: recall campaigns from NHTSA (United States) and Transport Canada, matched to the Wenmar Open vehicle catalog, in one plain SQLite database. It is what [`wenmar-open/offline`](https://www.npmjs.com/package/wenmar-open) reads, beside `wenmar-open-data`, to list the recalls that may apply to a vehicle with no network.

```bash
npm install wenmar-open wenmar-open-data wenmar-open-data-shop
```

The package is one file of about 77 MB (17 MB to download) and a few lines that say where it is and what it holds. It has no dependencies and runs nothing on install.

```js
import { path, dataVersion, catalogDataVersion, datasets } from "wenmar-open-data-shop";

console.log(path); // .../node_modules/wenmar-open-data-shop/wenmar-open-shop.sqlite3
console.log(dataVersion); // 2026.10.12
console.log(catalogDataVersion); // 2026.09
console.log(datasets.recalls_us.as_of); // 2026-10-11
```

`wenmar-open/offline/node` finds the file by itself:

```js
import { openOffline } from "wenmar-open/offline/node";

const open = await openOffline();
const answer = await open.vinRecalls("KM8K2CAB4PU001140");
console.log(answer.recalls.length, "recall campaigns may apply");
console.log("Check the VIN:", answer.lookup.nhtsa, answer.lookup.tc);
```

## A recall listed may apply

A campaign lists a year, a make and a model. Neither government publishes recalls by VIN as data, so nothing in this file says a recall applies to one vehicle. Every answer built from it has `may_apply: true` and the official lookups, where a VIN can be checked with the manufacturer's records.

## It goes with one data file

The shop file holds the catalog's ids, so it is read only with the data file it was built against. `catalogDataVersion` says which, and the package names that exact version of `wenmar-open-data` as a peer: npm warns when the two installed do not go together, and `wenmar-open` then refuses the pair with `data_invalid`. Each month, when `wenmar-open-data` has a new version, a shop file is published for it. Update the two together:

```bash
npm install wenmar-open-data@latest wenmar-open-data-shop@latest
```

## Versions

A new file is built each week, and again after each monthly data file. The version has three numbers:

| Part | Is | Example |
|---|---|---|
| major | the schema version: the layout of the tables | `1` |
| minor | the day the file was built | `20261012` |
| patch | a rebuild within the day | `0` |

So `1.20261012.0` is the file of 12 October 2026. A version of `wenmar-open` reads one schema version; its README says which.

The same file is attached to each [shop data release](https://github.com/wenmar-pro/wenmar-open/releases) on GitHub, tagged `shop-YYYY.MM.DD`, gzipped, for use without npm. `wenmar-open data pull --shop` downloads it for the command-line tool. A release has a second, larger file with `full` in its name, which also holds NHTSA's manufacturer communications (service bulletins). It is what open.wenmarpro.com serves; this package never carries it, and bulletins are read from the hosted API.

## Cloudflare D1

This package has no script of its own for D1. The one in `wenmar-open-data` writes any Wenmar Open file as SQL when it is given the file:

```bash
npx wenmar-open-d1 --file node_modules/wenmar-open-data-shop/wenmar-open-shop.sqlite3 --out ./d1-shop
```

Import it into a **second** D1 database, not the one that holds the data file: both files have a table called `meta`. Give `wenmar-open/offline` the second binding as its shop store:

```js
new WenmarOpenOffline({ store: d1Store(env.DB), shop: d1Store(env.SHOP_DB), wasm });
```

The import writes about 310,000 rows and builds 4 indexes over them, so it needs the Workers Paid plan, as the data file does. D1 refuses a `LIKE` pattern over 50 bytes: `searchRecalls` with `q` or `component` longer than 48 bytes fails there with `store_error`.

## Where the data comes from

- United States: NHTSA's recall files, a work of the United States government, in the public domain.
- Canada: Transport Canada's Vehicle Recalls Database. Contains information licensed under the Open Government Licence - Canada.

`datasets` gives the day each was current and the credit line each licence asks for. Show the credit where you show the data. See NOTICE.md.

## License

MIT for the package. The data is as described above.
