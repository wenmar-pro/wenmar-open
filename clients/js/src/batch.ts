import type { BatchItem, ErrorBody, NarrowedBatchItem } from "./types.js";

/** The most VINs decoded in one batch. */
export const MAX_BATCH = 250;

/** Whether an item of a batch is the error for its VIN and not a decode. */
export function isBatchError(item: BatchItem | NarrowedBatchItem): item is ErrorBody {
  const error = (item as { error?: unknown }).error;
  return typeof error === "object" && error !== null && !Array.isArray(error);
}

/** The `fields` of a batch as the API reads them: one text, separated by commas. */
export function fieldList(fields: readonly string[] | undefined): string | undefined {
  return fields === undefined ? undefined : fields.join(",");
}
