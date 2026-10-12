// The datasets only the hosted service carries: too large for the shop file
// that people download. A shop file that holds one of them is the full file,
// which the server reads, and is never published to npm or pulled by the
// command-line tool.
//
// This is the list of ids in `HOSTED_ONLY_TABLES` of
// crates/wenmar-shopdata/src/schema.rs. test/data-shop.test.mjs reads that
// file and fails when the two differ.
export const HOSTED_ONLY = ["bulletins"];

/** The hosted-only dataset ids among the keys of a `datasets` object. */
export function hostedOnlyIn(datasets) {
  return Object.keys(datasets).filter((id) => HOSTED_ONLY.includes(id));
}
