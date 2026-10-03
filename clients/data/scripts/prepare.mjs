// Puts together the directory that is published as wenmar-open-data:
//
//   node scripts/prepare.mjs <data file .sqlite3> [output directory]
//
// The output directory, clients/data/build by default, is replaced. It
// gets this package's files, the data file under the name
// wenmar-open.sqlite3, an index.js that says where the file is and what it
// is, and a package.json whose version is worked out from the file's own
// meta table (see lib/version.mjs). Nothing is published from here.
import { copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, realpathSync, rmSync, statSync, writeFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { npmVersion } from "../lib/version.mjs";

const [source, target] = process.argv.slice(2);
if (source === undefined) {
  console.error("usage: node scripts/prepare.mjs <data file .sqlite3> [output directory]");
  process.exit(2);
}
const home = new URL("../", import.meta.url);
const out = target ?? fileURLToPath(new URL("build", home));

// The real path of a place that may not exist yet: its nearest existing
// ancestor is resolved through links, and the rest is added back.
function realish(path) {
  const full = resolve(path);
  return existsSync(full) ? realpathSync(full) : join(realish(dirname(full)), full.slice(dirname(full).length + 1));
}
const contains = (outer, inner) => {
  const way = relative(outer, inner);
  return way === "" || (!way.startsWith("..") && !isAbsolute(way));
};
// The output directory is emptied. If it holds the data file or this
// package, that would delete the only copy of a file that takes a full
// build to make, so nothing is touched.
if (contains(realish(out), dirname(realish(source))) || contains(realish(out), realish(fileURLToPath(home)))) {
  console.error(`${out} would delete the data file or the package; give an output directory of its own.`);
  process.exit(1);
}

const database = new DatabaseSync(source, { readOnly: true });
let meta;
try {
  meta = Object.fromEntries(
    database
      .prepare("SELECT key, value FROM meta")
      .all()
      .map((row) => [row.key, row.value]),
  );
} catch (error) {
  console.error(`${source} is not a Wenmar Open data file: ${error.message}`);
  process.exit(1);
} finally {
  database.close();
}
for (const key of ["schema_version", "data_version", "vpic_release", "built_at"]) {
  if (typeof meta[key] !== "string" || meta[key] === "") {
    console.error(`${source} is not a Wenmar Open data file: its meta table has no ${key}.`);
    process.exit(1);
  }
}
const version = npmVersion(meta.schema_version, meta.data_version);

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
for (const name of ["README.md", "LICENSE", "NOTICE.md", "index.d.ts"]) {
  copyFileSync(new URL(name, home), `${out}/${name}`);
}
for (const name of ["bin", "lib"]) {
  cpSync(new URL(name, home), `${out}/${name}`, { recursive: true });
}
copyFileSync(source, `${out}/wenmar-open.sqlite3`);

const manifest = JSON.parse(readFileSync(new URL("package.json", home), "utf8"));
manifest.version = version;
delete manifest.scripts;
writeFileSync(`${out}/package.json`, `${JSON.stringify(manifest, null, 2)}\n`);
writeFileSync(
  `${out}/index.js`,
  `// Written by scripts/prepare.mjs for data ${meta.data_version}.
import { dirname, isAbsolute, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const path = fileURLToPath(new URL("./wenmar-open.sqlite3", import.meta.url));
export const dataVersion = ${JSON.stringify(meta.data_version)};
export const schemaVersion = ${JSON.stringify(meta.schema_version)};
export const vpicRelease = ${JSON.stringify(meta.vpic_release)};
export const builtAt = ${JSON.stringify(meta.built_at)};
`,
);

const bytes = statSync(`${out}/wenmar-open.sqlite3`).size;
console.log(`wenmar-open-data@${version}: data ${meta.data_version}, schema ${meta.schema_version}, ${bytes} bytes, in ${out}`);
