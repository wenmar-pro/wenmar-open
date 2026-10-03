// What the offline tests share: the fixture written by
// crates/wenmar-open-wasm/tests/fixture.rs, a store that needs no SQLite,
// and a stand-in for a Cloudflare D1 binding.
import { readFileSync } from "node:fs";

import type { WenmarOpen } from "wenmar-open";
import type { D1Like, D1Statement, Param, Statement, Store, WenmarOpenOffline } from "wenmar-open/offline";

/** clients/js/test/fixtures/, from .test-build/support/. */
const FIXTURES = new URL("../../test/fixtures/", import.meta.url);

export interface Failure {
  code: string;
  message: string;
  details: Record<string, unknown>;
}

export interface Case {
  name: string;
  op: string;
  args: Record<string, unknown>;
  /** How many times the question goes back to the database. */
  steps: number;
  answer: { ok: unknown } | { error: Failure };
}

interface Recorded {
  sql: string;
  params: Param[];
  rows: (string | number | null)[][];
}

export const fixture = JSON.parse(readFileSync(new URL("offline-cases.json", FIXTURES), "utf8")) as {
  current_year: number;
  cases: Case[];
  recorded: Recorded[];
};

/** The fixture data file as SQL, for a real SQLite. */
export const fixtureSql = readFileSync(new URL("offline.sql", FIXTURES), "utf8");

export function caseNamed(name: string): Case {
  const found = fixture.cases.find((candidate) => candidate.name === name);
  if (found === undefined) throw new Error(`no case named ${name}`);
  return found;
}

const keyOf = (statement: Statement): string => `${statement.sql}\u0000${JSON.stringify(statement.params)}`;

const recorded = new Map<string, Recorded["rows"]>(
  fixture.recorded.map((entry) => [keyOf(entry), entry.rows]),
);

/** The rows recorded for a statement. A statement nobody recorded is a test failure. */
export function rowsOf(statement: Statement): unknown[][] {
  const rows = recorded.get(keyOf(statement));
  if (rows === undefined) throw new Error(`no rows were recorded for: ${statement.sql} ${JSON.stringify(statement.params)}`);
  return structuredClone(rows);
}

/** A store that answers at once from the recorded rows, and counts. */
export class RecordedStore implements Store {
  steps = 0;
  readonly statements: Statement[] = [];
  /** Changes every row before it is returned. */
  alter: (rows: unknown[][], statement: Statement) => unknown[][] = (rows) => rows;

  query(statements: readonly Statement[]): unknown[][][] {
    this.steps += 1;
    return statements.map((statement) => {
      this.statements.push(statement);
      return this.alter(rowsOf(statement), statement);
    });
  }
}

/**
 * A Cloudflare D1 binding, as far as `d1Store` uses one: every statement is
 * prepared, bound and read with `raw()`, which answers later. It keeps D1's
 * limit of 100 bound parameters and counts queries, as D1 does for each
 * Worker invocation.
 */
export class FakeD1 implements D1Like {
  queries = 0;
  /** The query with this number, counting from 1, fails. */
  failAt: number | undefined;

  prepare(sql: string): D1Statement {
    const bound = (params: Param[]): D1Statement => ({
      bind: (...values: Param[]) => bound(values),
      raw: async () => {
        await new Promise((resolve) => setTimeout(resolve, 1));
        this.queries += 1;
        if (this.queries === this.failAt) throw new Error("D1_ERROR: Network connection lost.");
        if (params.length > 100) throw new Error("D1_ERROR: too many SQL variables");
        if (params.some((value) => typeof value === "bigint")) throw new Error("D1_TYPE_ERROR: BigInt is not supported");
        return rowsOf({ sql, params });
      },
    });
    return bound([]);
  }
}

/** What both clients have in common: the methods a case can call. */
export type Client = Pick<
  WenmarOpen,
  "decodeVin" | "decodeVins" | "years" | "makes" | "models" | "submodels" | "trims" | "engines" | "search" | "vehicle" | "meta"
>;

// The offline client is a `Client`: if a method's parameters or answer
// drift from the hosted client's, this line stops compiling.
export const sameSurface = (offline: WenmarOpenOffline): Client => offline;

/** Asks a client the question of a case. */
export function ask(client: Client, { op, args }: Case): Promise<unknown> {
  const query = args as never;
  switch (op) {
    case "decode":
      return client.decodeVin(String(args["vin"]), args["year"] === undefined ? {} : { year: Number(args["year"]) });
    case "years":
      return client.years(query);
    case "makes":
      return client.makes(query);
    case "models":
      return client.models(query);
    case "submodels":
      return client.submodels(query);
    case "engines":
      return client.engines(query);
    case "search":
      return client.search(query);
    case "vehicle":
      return client.vehicle(String(args["id"]));
    case "meta":
      return client.meta();
    default:
      throw new Error(`no method for ${op}`);
  }
}

/** A call's answer, or the code, message and details it failed with. */
export async function outcome(call: Promise<unknown>): Promise<{ ok: unknown } | { error: Failure }> {
  try {
    return { ok: await call };
  } catch (error) {
    const { code, message, details } = error as Failure;
    return { error: { code, message, details } };
  }
}

/**
 * A module with the decoder's five exports whose `wo_call` stops at once,
 * as a panic does, leaving `the decoder panicked` where the answer goes.
 */
export const STOPPING_WASM = Uint8Array.from(
  atob(
    "AGFzbQEAAAABEANgAX8Bf2ACf38Bf2AAAX8DBQQAAQICBQMBAAEHOwUGbWVtb3J5AgAId29fYWxsb2MAAAd3b19jYWxsAAEJd29fcmVzdWx0AAINd29fcmVzdWx0X2xlbgADChUEBQBBgAgLAwAACwQAQSALBABBFAsLGgEAQSALFHRoZSBkZWNvZGVyIHBhbmlja2Vk",
  ),
  (character) => character.charCodeAt(0),
);
