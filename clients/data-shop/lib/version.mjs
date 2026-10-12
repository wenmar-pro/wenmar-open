// The npm version of a shop file.
//
// A shop version is the day the file was built, YYYY.MM.DD, or YYYY.MM.DD.N
// for the Nth rebuild of that day. npm needs three numbers with no leading
// zero, and what a version range must protect an application from is a file
// its copy of wenmar-open cannot read. So the shop file's schema version is
// the major number:
//
//   schema 1, shop data 2026.10.12    ->  1.20261012.0
//   schema 1, shop data 2026.10.12.1  ->  1.20261012.1
//
// `"wenmar-open-data-shop": "^1.20261012.0"`, which is what `npm install`
// writes, then takes every later week and never a file of another layout.

/** The npm version for a shop schema version and a shop data version. Throws on anything else. */
export function npmShopVersion(schemaVersion, dataVersion) {
  if (!/^[1-9][0-9]{0,3}$/.test(String(schemaVersion))) {
    throw new Error(`"${schemaVersion}" is not a schema version such as 1`);
  }
  const match = /^([0-9]{4})\.(0[1-9]|1[0-2])\.(0[1-9]|[12][0-9]|3[01])(?:\.([1-9][0-9]{0,3}))?$/.exec(String(dataVersion));
  if (match === null) {
    throw new Error(`"${dataVersion}" is not a shop data version such as 2026.10.12 or 2026.10.12.1`);
  }
  const [, year, month, day, rebuild] = match;
  return `${schemaVersion}.${year}${month}${day}.${rebuild ?? "0"}`;
}

/** The shop data version an npm version stands for: `1.20261012.1` is `2026.10.12.1`. */
export function shopDataVersion(version) {
  const match = /^[1-9][0-9]*\.([0-9]{4})(0[1-9]|1[0-2])(0[1-9]|[12][0-9]|3[01])\.(0|[1-9][0-9]*)$/.exec(String(version));
  if (match === null) throw new Error(`"${version}" is not a version of wenmar-open-data-shop`);
  const [, year, month, day, rebuild] = match;
  return rebuild === "0" ? `${year}.${month}.${day}` : `${year}.${month}.${day}.${rebuild}`;
}
