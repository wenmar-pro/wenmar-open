/** The codes the API answers with. */
export type ApiErrorCode =
  | "invalid_vin"
  | "validation_failed"
  | "not_found"
  | "method_not_allowed"
  | "payload_too_large"
  | "uri_too_long"
  | "rate_limited"
  | "unavailable"
  | "internal_error";

/**
 * The codes this client adds for failures that never reached the API or
 * did not come from it.
 *
 * - `network`: the request could not be sent or the answer could not be read.
 * - `timeout`: no complete answer within `timeoutMs`.
 * - `aborted`: the caller's `signal` was aborted.
 * - `bad_response`: something answered, but not with the API's JSON. A proxy's
 *   error page and a sign-in page from a captive portal both end up here.
 */
export type ClientErrorCode = "network" | "timeout" | "aborted" | "bad_response";

/**
 * The codes of `wenmar-open/offline`, for failures of the data and not of
 * the question. The command-line tool uses `no_data` and `data_invalid` for
 * the same things.
 *
 * - `no_data`: there is no data file where one was looked for.
 * - `data_invalid`: the database is not a data file this version reads: it
 *   has another schema version, or a row is not what the layout says.
 * - `store_error`: the database failed while it was being read.
 */
export type OfflineErrorCode = "no_data" | "data_invalid" | "store_error";

/**
 * Every code known when this version was published. The API may add codes,
 * so `code` is typed to accept any string while still completing these.
 */
export type ErrorCode = ApiErrorCode | ClientErrorCode | OfflineErrorCode | (string & {});

export interface WenmarOpenErrorInit {
  code: ErrorCode;
  message: string;
  details?: Record<string, unknown> | undefined;
  status?: number | undefined;
  retryAfter?: number | undefined;
  cause?: unknown;
}

/** The one error every method of the client throws. */
export class WenmarOpenError extends Error {
  /** A stable code to branch on. */
  readonly code: ErrorCode;
  /** The HTTP status, or `undefined` when there was no HTTP answer. */
  readonly status: number | undefined;
  /** More about the error, as the API sent it. Empty when there is nothing. */
  readonly details: Record<string, unknown>;
  /**
   * Seconds to wait before trying again, from the `Retry-After` header of a
   * `429` or `503`. `undefined` when the answer did not say.
   */
  readonly retryAfter: number | undefined;

  constructor(init: WenmarOpenErrorInit) {
    super(init.message, init.cause === undefined ? undefined : { cause: init.cause });
    this.name = "WenmarOpenError";
    this.code = init.code;
    this.status = init.status;
    this.details = init.details ?? {};
    this.retryAfter = init.retryAfter;
  }
}

/**
 * Reads a `Retry-After` header: a number of seconds, or an HTTP date.
 * Returns whole seconds, never negative, or `undefined` if it is neither.
 */
export function parseRetryAfter(header: string | null, now: number = Date.now()): number | undefined {
  if (header === null) return undefined;
  const text = header.trim();
  if (/^\d+$/.test(text)) return Number(text);
  const date = Date.parse(text);
  if (Number.isNaN(date)) return undefined;
  return Math.max(0, Math.ceil((date - now) / 1000));
}
