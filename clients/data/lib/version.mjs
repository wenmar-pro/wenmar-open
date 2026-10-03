// The npm version of a data file.
//
// A data version is YYYY.MM, or YYYY.MM.N for the Nth rebuild of a month.
// npm needs three numbers with no leading zero, and what a version range
// must protect an application from is a file its copy of wenmar-open cannot
// read. So the schema version is the major number:
//
//   schema 3, data 2026.09    ->  3.202609.0
//   schema 3, data 2026.09.1  ->  3.202609.1
//
// `"wenmar-open-data": "^3.202609.0"`, which is what `npm install` writes,
// then takes every later month and never a file of another layout.

/** The npm version for a schema version and a data version. Throws on anything else. */
export function npmVersion(schemaVersion, dataVersion) {
  if (!/^[1-9][0-9]{0,3}$/.test(String(schemaVersion))) {
    throw new Error(`"${schemaVersion}" is not a schema version such as 3`);
  }
  const match = /^([0-9]{4})\.(0[1-9]|1[0-2])(?:\.([1-9][0-9]{0,3}))?$/.exec(String(dataVersion));
  if (match === null) {
    throw new Error(`"${dataVersion}" is not a data version such as 2026.09 or 2026.09.1`);
  }
  const [, year, month, rebuild] = match;
  return `${schemaVersion}.${year}${month}.${rebuild ?? "0"}`;
}

/** The data version an npm version stands for: `3.202609.1` is `2026.09.1`. */
export function dataVersion(version) {
  const match = /^[1-9][0-9]*\.([0-9]{4})(0[1-9]|1[0-2])\.(0|[1-9][0-9]*)$/.exec(String(version));
  if (match === null) throw new Error(`"${version}" is not a version of wenmar-open-data`);
  const [, year, month, rebuild] = match;
  return rebuild === "0" ? `${year}.${month}` : `${year}.${month}.${rebuild}`;
}
