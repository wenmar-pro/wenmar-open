// Puts together the directory that is published as wenmar-open-data-shop:
//
//   node scripts/prepare.mjs <shop file .sqlite3> <data file .sqlite3> [output directory]
//
// The output directory, clients/data-shop/build by default, is replaced.
// It gets this package's files, the shop file under the name
// wenmar-open-shop.sqlite3, an index.js that says where the file is and
// what it holds, and a package.json whose version is worked out from the
// shop file's own meta table (see lib/version.mjs).
//
// A shop file holds the catalog ids of one data file. So the data file it
// was built against is given too: this refuses a pair that do not go
// together, and the package names that exact version of wenmar-open-data as
// a peer, so that npm says so when the two installed do not match.
// Nothing is published from here.
import { copyFileSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { fileURLToPath } from "node:url";

import { refuseOutput } from "../../data/lib/output.mjs";
import { npmVersion } from "../../data/lib/version.mjs";
import { npmShopVersion } from "../lib/version.mjs";

/**
 * The largest shop file published, unpacked: 200 MiB. With recalls alone
 * the file is 77 MB. npm documents no limit; refusals have been reported
 * from about 230 MB packed, and a file this size is the point at which to
 * decide what stays out of the offline package. WENMAR_OPEN_SHOP_MAX_BYTES
 * overrides it, for the test.
 */
const LARGEST_BYTES = Number(process.env.WENMAR_OPEN_SHOP_MAX_BYTES ?? 200 * 1024 * 1024);

const [source, core, target] = process.argv.slice(2);
if (source === undefined || core === undefined) {
  console.error("usage: node scripts/prepare.mjs <shop file .sqlite3> <data file .sqlite3> [output directory]");
  process.exit(2);
}
const home = new URL("../", import.meta.url);
const out = target ?? fileURLToPath(new URL("build", home));

// The output directory is emptied; see clients/data/lib/output.mjs for when
// that is refused.
for (const file of [source, core]) {
  const refusal = refuseOutput(file, out, fileURLToPath(home));
  if (refusal !== undefined) {
    console.error(refusal);
    process.exit(1);
  }
}

/** The meta table of a file, or a refusal that names the file. */
function metaOf(file, what, keys) {
  let meta;
  let database;
  try {
    database = new DatabaseSync(file, { readOnly: true });
    meta = Object.fromEntries(
      database
        .prepare("SELECT key, value FROM meta")
        .all()
        .map((row) => [row.key, row.value]),
    );
  } catch (error) {
    console.error(`${file} is not a Wenmar Open ${what}: ${error.message}`);
    process.exit(1);
  } finally {
    database?.close();
  }
  for (const key of keys) {
    if (typeof meta[key] !== "string" || meta[key] === "") {
      console.error(`${file} is not a Wenmar Open ${what}: its meta table has no ${key}.`);
      process.exit(1);
    }
  }
  return meta;
}

const meta = metaOf(source, "shop file", ["schema_version", "data_version", "catalog_data_version", "built_at", "datasets"]);
const coreMeta = metaOf(core, "data file", ["schema_version", "data_version", "vpic_release"]);
if (meta.catalog_data_version !== coreMeta.data_version) {
  console.error(
    `${source} was matched against data version ${meta.catalog_data_version}, and ${core} is data version ${coreMeta.data_version}. Give the data file the shop file was built against.`,
  );
  process.exit(1);
}
let datasets;
try {
  datasets = JSON.parse(meta.datasets);
} catch (error) {
  console.error(`${source} is not a Wenmar Open shop file: its datasets value is not JSON: ${error.message}`);
  process.exit(1);
}
if (datasets === null || typeof datasets !== "object" || Array.isArray(datasets) || Object.keys(datasets).length === 0) {
  console.error(`${source} holds no dataset. A shop file with nothing in it is not published.`);
  process.exit(1);
}
const bytes = statSync(source).size;
if (bytes > LARGEST_BYTES) {
  console.error(
    `${source} is ${bytes} bytes; the largest shop file published is ${LARGEST_BYTES}. Decide what stays out of the offline package before raising this (see docs/plans/shop-data.md, Q5).`,
  );
  process.exit(1);
}
const version = npmShopVersion(meta.schema_version, meta.data_version);
const peer = npmVersion(coreMeta.schema_version, coreMeta.data_version);

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
for (const name of ["README.md", "LICENSE", "NOTICE.md", "index.d.ts"]) {
  copyFileSync(new URL(name, home), `${out}/${name}`);
}
copyFileSync(source, `${out}/wenmar-open-shop.sqlite3`);

const manifest = JSON.parse(readFileSync(new URL("package.json", home), "utf8"));
manifest.version = version;
// The one data file this shop file can be read with.
manifest.peerDependencies = { "wenmar-open-data": peer };
delete manifest.scripts;
writeFileSync(`${out}/package.json`, `${JSON.stringify(manifest, null, 2)}\n`);
writeFileSync(
  `${out}/index.js`,
  `// Written by scripts/prepare.mjs for shop data ${meta.data_version}.
import { fileURLToPath } from "node:url";

export const path = fileURLToPath(new URL("./wenmar-open-shop.sqlite3", import.meta.url));
export const dataVersion = ${JSON.stringify(meta.data_version)};
export const catalogDataVersion = ${JSON.stringify(meta.catalog_data_version)};
export const schemaVersion = ${JSON.stringify(meta.schema_version)};
export const builtAt = ${JSON.stringify(meta.built_at)};
export const datasets = ${JSON.stringify(datasets)};
`,
);

console.log(
  `wenmar-open-data-shop@${version}: shop data ${meta.data_version} for wenmar-open-data@${peer}, schema ${meta.schema_version}, ${Object.keys(datasets).join(", ")}, ${bytes} bytes, in ${out}`,
);
