import type { BatchItem, ErrorBody } from "./types.js";

/** The most VINs decoded in one batch. */
export const MAX_BATCH = 50;

/** Whether an item of a batch is the error for its VIN and not a decode. */
export function isBatchError(item: BatchItem): item is ErrorBody {
  const error = (item as { error?: unknown }).error;
  return typeof error === "object" && error !== null && !Array.isArray(error);
}
