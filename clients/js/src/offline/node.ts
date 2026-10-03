import { WenmarOpenError } from "../errors.js";
import { WenmarOpenOffline } from "./client.js";
import type { WasmSource } from "./engine.js";
import { syncStore } from "./store.js";
import type { Store, SyncDatabase } from "./store.js";

export interface NodeOptions {
  /**
   * The data file. Default: the one in the `wenmar-open-data` package,
   * which must then be installed.
   */
  path?: string;
  wasm?: WasmSource;
  currentYear?: number;
}

interface Sqlite {
  DatabaseSync: new (path: string, options: { readOnly: boolean }) => SyncDatabase & { close(): void };
}

/** Loaded by name at run time, so nothing is resolved when this file is read. */
async function load<T>(specifier: string): Promise<T> {
  return (await import(specifier)) as T;
}

/**
 * Opens a data file with Node's own SQLite (Node 22.16 or later) and
 * returns a client that reads it. The file is opened read-only and checked:
 * a file of another schema version fails here, not at the first decode.
 *
 * On Node 20, open the file with `better-sqlite3` and give it to
 * `syncStore` instead.
 */
export async function openOffline(options: NodeOptions = {}): Promise<WenmarOpenOffline> {
  let path = options.path;
  if (path === undefined) {
    try {
      path = (await load<{ path: string }>("wenmar-open-data")).path;
    } catch (cause) {
      throw new WenmarOpenError({
        code: "no_data",
        message: "There is no data file. Install the wenmar-open-data package, or pass { path }.",
        cause,
      });
    }
  }
  let sqlite: Sqlite;
  try {
    sqlite = await load<Sqlite>("node:sqlite");
  } catch (cause) {
    throw new WenmarOpenError({
      code: "store_error",
      message:
        "node:sqlite is not available. It needs Node 22.16 or later; on older versions open the file with better-sqlite3 and pass it to syncStore.",
      cause,
    });
  }
  let database: SyncDatabase & { close(): void };
  try {
    database = new sqlite.DatabaseSync(path, { readOnly: true });
  } catch (cause) {
    throw new WenmarOpenError({
      code: "no_data",
      message: `The data file at ${path} could not be opened.`,
      details: { path },
      cause,
    });
  }
  const store: Store = { ...syncStore(database), close: () => database.close() };
  const client = new WenmarOpenOffline({
    store,
    ...(options.wasm === undefined ? {} : { wasm: options.wasm }),
    ...(options.currentYear === undefined ? {} : { currentYear: options.currentYear }),
  });
  try {
    await client.meta();
  } catch (error) {
    database.close();
    throw error;
  }
  return client;
}
