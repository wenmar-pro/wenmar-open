/** The absolute path of the shop file, a plain SQLite database. */
export declare const path: string;
/** The shop file's own version: the day it was built, such as `2026.10.12`. */
export declare const dataVersion: string;
/**
 * The version of the data file (`wenmar-open-data`) its catalog ids belong
 * to, such as `2026.09`. The shop file is read only with that data file.
 */
export declare const catalogDataVersion: string;
/** The layout of the file's tables. `wenmar-open` reads one schema version. */
export declare const schemaVersion: string;
/** When the file was built, in UTC. */
export declare const builtAt: string;
/** What the file holds, by dataset id such as `recalls_us`. */
export declare const datasets: Record<
  string,
  { as_of: string; rows: number; licence: string; attribution: string }
>;
