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
  /**
   * The shop file, which holds the recalls. Default: the one in the
   * `wenmar-open-data-shop` package, if that package is installed. `false`
   * opens none even then.
   */
  shopPath?: string | false;
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
  // The shop file: the one named, or the one of the wenmar-open-data-shop
  // package when it is installed. With neither, recalls are
  // `dataset_unavailable` and everything else works.
  let shopPath = options.shopPath;
  if (shopPath === undefined) {
    shopPath = await load<{ path: string }>("wenmar-open-data-shop").then(
      (module) => module.path,
      () => false as const,
    );
  }
  let shopDatabase: (SyncDatabase & { close(): void }) | undefined;
  if (shopPath !== false) {
    try {
      shopDatabase = new sqlite.DatabaseSync(shopPath, { readOnly: true });
    } catch (cause) {
      database.close();
      throw new WenmarOpenError({
        code: "no_data",
        message: `The shop file at ${shopPath} could not be opened.`,
        details: { path: shopPath },
        cause,
      });
    }
  }
  const shop = shopDatabase;
  const store: Store = { ...syncStore(database), close: () => database.close() };
  const client = new WenmarOpenOffline({
    store,
    ...(shop === undefined ? {} : { shop: { ...syncStore(shop), close: () => shop.close() } }),
    ...(options.wasm === undefined ? {} : { wasm: options.wasm }),
    ...(options.currentYear === undefined ? {} : { currentYear: options.currentYear }),
  });
  try {
    // `meta` opens the data file and, when there is one, the shop file,
    // which must go with it.
    await client.meta();
  } catch (error) {
    database.close();
    shop?.close();
    // SQLite opens a file that is not a database without complaint and
    // fails on the first read; for a file on disk that means the file is
    // not a data file, not that a connection dropped.
    if (error instanceof WenmarOpenError && error.code === "store_error" && error.cause !== undefined) {
      throw new WenmarOpenError({
        code: "data_invalid",
        message: `The file at ${path} could not be read as a Wenmar Open data file.`,
        details: { path },
        cause: error.cause,
      });
    }
    throw error;
  }
  return client;
}
