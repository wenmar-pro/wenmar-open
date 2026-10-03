// A data file as SQL text that Cloudflare D1 imports.
//
// D1 does not take a SQLite file. It takes SQL, with no statement over
// 100,000 bytes and no transaction statements. This writes the tables, then
// their rows as INSERT statements of many rows each, then the indexes, so
// an index is built once and not kept up row by row.

/** D1 refuses a statement over this many bytes. */
export const D1_STATEMENT_BYTES = 100_000;
/** Rows are gathered into a statement up to this, which leaves D1 room to spare. */
export const STATEMENT_BYTES = 90_000;
/** D1 refuses a LIKE or GLOB pattern over 50 bytes. */
export const PATTERN_BYTES = 50;

const encoder = new TextEncoder();
const size = (text) => encoder.encode(text).length;

/** The statement itself, or an error naming the table if D1 would refuse it. */
function within(statement, table) {
  const length = size(statement);
  if (length > D1_STATEMENT_BYTES) {
    throw new Error(`a statement for table "${table}" is ${length} bytes; D1 allows at most ${D1_STATEMENT_BYTES}`);
  }
  return statement;
}

/** One value as a SQL literal. The data file holds numbers, text and nulls. */
export function literal(value) {
  if (value === null || value === undefined) return "NULL";
  if (typeof value === "number" || typeof value === "bigint") {
    if (typeof value === "number" && !Number.isFinite(value)) throw new Error("a number that is not finite");
    return String(value);
  }
  if (typeof value === "string") {
    if (value.includes("\u0000")) throw new Error("text with a NUL character");
    return `'${value.replaceAll("'", "''")}'`;
  }
  throw new Error(`a value that is neither a number, text nor null: ${typeof value}`);
}

/**
 * Yields the statements, each ending in `;` and a newline. `database` is a
 * `DatabaseSync` of node:sqlite, opened on a data file.
 */
export function* statements(database) {
  const objects = database
    .prepare("SELECT type, name, tbl_name, sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY rowid")
    .all();
  const tables = objects.filter((object) => object.type === "table" && !object.name.startsWith("sqlite_"));
  const indexes = objects.filter((object) => object.type === "index");

  // The decoder matches VIN patterns with GLOB, and the pattern is the
  // stored key. One over D1's limit would fail every decode that reads it.
  if (tables.some((table) => table.name === "pattern")) {
    const [{ longest }] = database.prepare("SELECT MAX(LENGTH(CAST(keys AS BLOB))) + 1 AS longest FROM pattern").all();
    if (longest !== null && longest > PATTERN_BYTES) {
      throw new Error(`a VIN pattern is ${longest - 1} bytes; D1 matches patterns of at most ${PATTERN_BYTES - 1}`);
    }
  }

  for (const table of [...tables].reverse()) yield within(`DROP TABLE IF EXISTS "${table.name}";\n`, table.name);
  for (const table of tables) yield within(`${table.sql};\n`, table.name);
  for (const table of tables) {
    const select = database.prepare(`SELECT * FROM "${table.name}"`);
    select.setReturnArrays(true);
    const head = `INSERT INTO "${table.name}" VALUES\n`;
    let rows = [];
    let bytes = size(head);
    for (const row of select.iterate()) {
      const text = `(${row.map(literal).join(",")})`;
      const added = size(text) + 2;
      if (rows.length > 0 && bytes + added > STATEMENT_BYTES) {
        yield within(`${head}${rows.join(",\n")};\n`, table.name);
        rows = [];
        bytes = size(head);
      }
      rows.push(text);
      bytes += added;
    }
    if (rows.length > 0) yield within(`${head}${rows.join(",\n")};\n`, table.name);
  }
  for (const index of indexes) yield within(`${index.sql};\n`, index.tbl_name);
}

/**
 * Gathers statements into parts of at most `partBytes` each (a statement is
 * never split) and hands each part to `write(name, text)`. Returns what was
 * written.
 */
export function writeParts(database, partBytes, write) {
  const summary = { parts: 0, statements: 0, bytes: 0, longestStatement: 0 };
  let part = [];
  let bytes = 0;
  const flush = () => {
    if (part.length === 0) return;
    summary.parts += 1;
    write(`part-${String(summary.parts).padStart(3, "0")}.sql`, part.join(""));
    part = [];
    bytes = 0;
  };
  for (const statement of statements(database)) {
    const length = size(statement);
    if (bytes > 0 && bytes + length > partBytes) flush();
    part.push(statement);
    bytes += length;
    summary.statements += 1;
    summary.bytes += length;
    summary.longestStatement = Math.max(summary.longestStatement, length);
  }
  flush();
  return summary;
}
