import { WenmarOpenError } from "../errors.js";

/** A value bound to a statement: text, a number or null. */
export type Param = string | number | null;

/** One `SELECT`, with what binds to `?1`, `?2` and so on, in order. */
export interface Statement {
  readonly sql: string;
  readonly params: readonly Param[];
}

/**
 * A database holding a Wenmar Open data file. It is asked to run a list of
 * `SELECT`s and gives back the rows of each, in the same order. A row is an
 * array with one value per selected column, in the order selected: never an
 * object, because two columns may share a name. It may answer at once or
 * with a promise.
 */
export interface Store {
  query(statements: readonly Statement[]): unknown[][][] | Promise<unknown[][][]>;
  /** Called by `close()` on the client. A store the caller opened leaves this out. */
  close?(): void;
}

/** A prepared statement of `node:sqlite` or of `better-sqlite3`. */
export interface SyncStatement {
  all(...params: (Param | Record<string, Param>)[]): unknown[];
  /** `node:sqlite`, Node 22.16 and later. */
  setReturnArrays?(enabled: boolean): void;
  /** `better-sqlite3`. */
  raw?(enabled?: boolean): unknown;
}

/**
 * A database of `node:sqlite` (`DatabaseSync`) or of `better-sqlite3`. What
 * `prepare` returns is used as a `SyncStatement`; it is typed loosely here
 * so that both libraries' own declarations fit.
 */
export interface SyncDatabase {
  prepare(sql: string): object;
}

/** How many prepared statements a store keeps. */
const MOST_PREPARED = 256;

/**
 * Gives every bare `?` the number SQLite gives it (one more than the
 * largest number so far), so a statement that mixes `?` and `?3` has only
 * numbered parameters. Text in quotes and comments is left alone.
 */
export function numberParameters(sql: string): string {
  let out = "";
  let largest = 0;
  let index = 0;
  while (index < sql.length) {
    const char = sql.charAt(index);
    const pair = sql.slice(index, index + 2);
    let end = -1;
    if (char === "'" || char === '"' || char === "`") {
      end = sql.indexOf(char, index + 1);
      while (end !== -1 && sql.charAt(end + 1) === char) end = sql.indexOf(char, end + 2);
      end = end === -1 ? sql.length : end + 1;
    } else if (pair === "--") {
      end = sql.indexOf("\n", index);
      end = end === -1 ? sql.length : end;
    } else if (pair === "/*") {
      end = sql.indexOf("*/", index + 2);
      end = end === -1 ? sql.length : end + 2;
    }
    if (end !== -1) {
      out += sql.slice(index, end);
      index = end;
      continue;
    }
    if (char === "?") {
      const digits = /^\d*/.exec(sql.slice(index + 1))?.[0] ?? "";
      if (digits === "") {
        largest += 1;
        out += `?${largest}`;
      } else {
        largest = Math.max(largest, Number(digits));
        out += `?${digits}`;
      }
      index += 1 + digits.length;
      continue;
    }
    out += char;
    index += 1;
  }
  return out;
}

/**
 * A store over a database the caller opened with `node:sqlite` or
 * `better-sqlite3`. The caller closes it.
 *
 * The two bind differently. `node:sqlite` binds values to `?1`, `?2` by
 * position. `better-sqlite3` reads `?1` as a parameter named `1`, which a
 * positional value does not fill, so there the values are given as one
 * object keyed by number.
 */
export function syncStore(database: SyncDatabase): Store {
  type Run = (params: readonly Param[]) => unknown[][];
  const prepared = new Map<string, Run>();
  const prepare = (sql: string): Run => {
    const kept = prepared.get(sql);
    if (kept !== undefined) return kept;
    let run: Run;
    const probe = database.prepare(sql) as SyncStatement;
    if (typeof probe.setReturnArrays === "function") {
      probe.setReturnArrays(true);
      run = (params) => probe.all(...params) as unknown[][];
    } else {
      const statement = typeof probe.raw === "function" ? (database.prepare(numberParameters(sql)) as SyncStatement) : probe;
      if (typeof statement.raw !== "function") {
        throw new WenmarOpenError({
          code: "store_error",
          message:
            "This database cannot return rows as arrays. Use node:sqlite from Node 22.16 or later, or better-sqlite3.",
        });
      }
      statement.raw(true);
      run = (params) =>
        (params.length === 0
          ? statement.all()
          : statement.all(Object.fromEntries(params.map((value, index) => [index + 1, value])))) as unknown[][];
    }
    if (prepared.size >= MOST_PREPARED) prepared.clear();
    prepared.set(sql, run);
    return run;
  };
  return {
    query: (statements) => statements.map(({ sql, params }) => prepare(sql)(params)),
  };
}

/** What `d1Store` uses of a D1 prepared statement. */
export interface D1Statement {
  bind(...values: Param[]): D1Statement;
  raw(): Promise<unknown[][]>;
}

/** A Cloudflare D1 binding, or a session made with `withSession()`. */
export interface D1Like {
  prepare(sql: string): D1Statement;
}

/**
 * A store over a Cloudflare D1 database that holds an imported data file.
 * The statements of one step are sent together and each counts as one D1
 * query.
 */
export function d1Store(database: D1Like): Store {
  return {
    query: (statements) =>
      Promise.all(statements.map(({ sql, params }) => database.prepare(sql).bind(...params).raw())),
  };
}

function invalid(message: string, details: Record<string, unknown>): WenmarOpenError {
  return new WenmarOpenError({ code: "data_invalid", message, details });
}

/**
 * The rows of one statement, as the decoder reads them: each value a
 * number, text or null. A database set to return every integer as a BigInt
 * is accepted. Bytes are not: the data file holds none.
 */
export function cleanRows(rows: unknown, sql: string): (string | number | null)[][] {
  if (!Array.isArray(rows)) {
    throw new WenmarOpenError({
      code: "store_error",
      message: "The store did not return a list of rows for a statement.",
      details: { sql },
    });
  }
  return rows.map((row: unknown) => {
    if (!Array.isArray(row)) {
      throw new WenmarOpenError({
        code: "store_error",
        message: "The store must return each row as an array of values, in the order selected.",
        details: { sql },
      });
    }
    return row.map((value: unknown, column: number) => {
      if (value === null || value === undefined) return null;
      if (typeof value === "string") return value;
      if (typeof value === "number") {
        if (Number.isFinite(value)) return value;
        throw invalid("The database returned a number that is not finite.", { sql, column });
      }
      if (typeof value === "bigint") {
        if (value >= BigInt(Number.MIN_SAFE_INTEGER) && value <= BigInt(Number.MAX_SAFE_INTEGER)) {
          return Number(value);
        }
        throw invalid("The database returned a whole number too large to read exactly.", { sql, column });
      }
      if (typeof value === "boolean") return value ? 1 : 0;
      throw invalid("The database returned a value that is not a number, text or null.", {
        sql,
        column,
        kind: ArrayBuffer.isView(value) || value instanceof ArrayBuffer ? "bytes" : typeof value,
      });
    });
  });
}
