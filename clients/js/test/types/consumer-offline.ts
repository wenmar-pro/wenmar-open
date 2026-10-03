// Not run: compiled. An application that uses the offline mode, and one
// function that works with either client, under the strictest compiler
// settings, with the DOM library and without it.
import type { VinDecode, WenmarOpen } from "wenmar-open";
import { WenmarOpenError, WenmarOpenOffline, d1Store, isBatchError, syncStore } from "wenmar-open/offline";
import type {
  D1Like,
  ErrorCode,
  OfflineErrorCode,
  OfflineOptions,
  Statement,
  Store,
  SyncDatabase,
  WasmSource,
} from "wenmar-open/offline";
import { openOffline } from "wenmar-open/offline/node";
import type { NodeOptions } from "wenmar-open/offline/node";

// Code written against the hosted client takes the offline one unchanged.
type Either = Pick<
  WenmarOpen,
  "decodeVin" | "decodeVins" | "years" | "makes" | "models" | "submodels" | "trims" | "engines" | "search" | "vehicle" | "meta"
>;

export async function describe(client: Either, vin: string, signal: AbortSignal): Promise<string> {
  const decode: VinDecode = await client.decodeVin(vin, { year: 2023, signal, timeoutMs: 500 });
  const items = await client.decodeVins([vin], { signal });
  const failed = items.filter(isBatchError).length;
  const models = await client.models({ make: decode.make ?? "honda", year: 2019 });
  const entry = await client.vehicle("2019_honda_civic_si");
  return `${decode.vin} ${models.length} ${entry.summary} ${failed}`;
}

declare const hosted: WenmarOpen;
declare const d1: D1Like;
declare const database: SyncDatabase;
declare const wasm: WasmSource;

const options: OfflineOptions = { store: d1Store(d1), wasm, currentYear: 2026 };
const offline = new WenmarOpenOffline(options);
export const both = (signal: AbortSignal) => [describe(hosted, "KM8K2CAB4PU001140", signal), describe(offline, "KM8K2CAB4PU001140", signal)];
export const overSqlite = new WenmarOpenOffline({ store: syncStore(database) });

// A store of one's own: the statements in, the rows of each out.
export const own: Store = {
  query: async (statements: readonly Statement[]) => statements.map(({ sql, params }) => [[sql, params.length]]),
};

export async function onNode(): Promise<void> {
  const nodeOptions: NodeOptions = { path: "/data/wenmar-open.sqlite3" };
  const client: WenmarOpenOffline = await openOffline(nodeOptions);
  client.close();
}

export function everyError(error: unknown): string {
  if (!(error instanceof WenmarOpenError)) return "other";
  const code: ErrorCode = error.code;
  const offlineCodes: OfflineErrorCode[] = ["no_data", "data_invalid", "store_error"];
  if (code === "data_invalid" || offlineCodes.includes(code as OfflineErrorCode)) return "the data";
  if (code === "invalid_vin") return String(error.details["suggestions"]);
  return error.status === undefined ? code : `${code} ${error.status}`;
}
