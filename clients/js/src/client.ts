import { WenmarOpenError, parseRetryAfter } from "./errors.js";
import type {
  BatchItem,
  EngineOption,
  EnginesQuery,
  Entry,
  ErrorBody,
  Make,
  MakesQuery,
  Meta,
  Model,
  ModelsQuery,
  SearchQuery,
  Submodel,
  SubmodelsQuery,
  VinDecode,
  YearsQuery,
} from "./types.js";

/** The hosted API. */
export const DEFAULT_BASE_URL = "https://open.wenmarpro.com";
/** How long a request may take before it fails with `timeout`. */
export const DEFAULT_TIMEOUT_MS = 10_000;
/** The most VINs the API decodes in one batch. */
export const MAX_BATCH = 50;

/** What the client asks of a `fetch`: the part of the standard it uses. */
export interface FetchInit {
  method: string;
  headers: Record<string, string>;
  body?: string;
  signal: AbortSignal;
}

/** What the client reads of a response. */
export interface FetchResponse {
  readonly status: number;
  readonly headers: { get(name: string): string | null };
  text(): Promise<string>;
}

/** A `fetch`. The platform's own fits; so does a Workers service binding's. */
export type FetchLike = (url: string, init: FetchInit) => Promise<FetchResponse>;

export interface ClientOptions {
  /**
   * Where the API is. Default `https://open.wenmarpro.com`. A path is kept,
   * so `https://example.com/open` reaches `https://example.com/open/v1/...`.
   */
  baseUrl?: string;
  /**
   * Milliseconds a request may take, the body included. Default 10,000.
   * `0` means no limit.
   */
  timeoutMs?: number;
  /** Used instead of the platform's `fetch`. */
  fetch?: FetchLike;
}

/** What every method accepts last. */
export interface RequestOptions {
  /** Aborting it fails the request with the code `aborted`. */
  signal?: AbortSignal;
  /** Overrides the client's `timeoutMs` for this request. */
  timeoutMs?: number;
}

export interface DecodeOptions extends RequestOptions {
  /** Use this model year instead of working it out from the VIN. */
  year?: number;
}

type QueryValue = string | number | undefined;

function queryString(query: object | undefined): string {
  const params = new URLSearchParams();
  for (const [name, value] of Object.entries((query ?? {}) as Record<string, QueryValue>)) {
    if (value === undefined || value === null || value === "") continue;
    params.append(name, String(value));
  }
  const text = params.toString();
  return text === "" ? "" : `?${text}`;
}

/**
 * One path segment. A URL treats `.` and `..` as directions, not names, even
 * when they are percent-encoded, so they are refused here: sent, they would
 * reach a different address than the one meant.
 */
function segment(field: string, value: string): string {
  const text = String(value).trim();
  if (text === "" || text === "." || text === "..") {
    throw new WenmarOpenError({
      code: "validation_failed",
      message: `${field} is required`,
      details: { field },
    });
  }
  return encodeURIComponent(text);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Whether an item of a batch is the error for its VIN and not a decode. */
export function isBatchError(item: BatchItem): item is ErrorBody {
  return isRecord((item as { error?: unknown }).error);
}

/** A client for the Wenmar Open API. It keeps no state between requests. */
export class WenmarOpen {
  readonly baseUrl: string;
  readonly timeoutMs: number;
  readonly #fetch: FetchLike;

  constructor(options: ClientOptions = {}) {
    this.baseUrl = (options.baseUrl ?? DEFAULT_BASE_URL).replace(/\/+$/, "");
    this.timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
    // The platform's fetch is looked up at each call, not kept.
    this.#fetch = options.fetch ?? ((url, init) => fetch(url, init));
  }

  /** Decode one VIN. Spaces, dashes and lowercase are accepted. */
  async decodeVin(vin: string, options: DecodeOptions = {}): Promise<VinDecode> {
    const path = `/v1/vin/${segment("vin", vin)}${queryString({ year: options.year })}`;
    return this.#request("GET", path, undefined, options);
  }

  /**
   * Decode up to 50 VINs. The answer has one item per VIN, in order: a
   * decode, or the error a single request for that VIN would have given.
   * Tell them apart with `isBatchError`.
   */
  async decodeVins(vins: readonly string[], options: RequestOptions = {}): Promise<BatchItem[]> {
    if (vins.length > MAX_BATCH) {
      throw new WenmarOpenError({
        code: "validation_failed",
        message: `a batch holds at most ${MAX_BATCH} VINs`,
        details: { field: "vins", max: MAX_BATCH, received: vins.length },
      });
    }
    return this.#request("POST", "/v1/vin/batch", { vins }, options);
  }

  /** Model years, newest first. */
  async years(query: YearsQuery = {}, options: RequestOptions = {}): Promise<number[]> {
    return this.#request("GET", `/v1/vehicles/years${queryString(query)}`, undefined, options);
  }

  /** Makes, popular ones first and the rest by name. */
  async makes(query: MakesQuery = {}, options: RequestOptions = {}): Promise<Make[]> {
    return this.#request("GET", `/v1/vehicles/makes${queryString(query)}`, undefined, options);
  }

  /** Models of a make, by name. An unknown make has no models. */
  async models(query: ModelsQuery, options: RequestOptions = {}): Promise<Model[]> {
    return this.#request("GET", `/v1/vehicles/models${queryString(query)}`, undefined, options);
  }

  /** Submodels of a model year: trims, or series where there are no trims. */
  async submodels(query: SubmodelsQuery, options: RequestOptions = {}): Promise<Submodel[]> {
    return this.#request("GET", `/v1/vehicles/submodels${queryString(query)}`, undefined, options);
  }

  /** The same as `submodels`, under the name most people use. */
  async trims(query: SubmodelsQuery, options: RequestOptions = {}): Promise<Submodel[]> {
    return this.#request("GET", `/v1/vehicles/trims${queryString(query)}`, undefined, options);
  }

  /** Engines of a model year, in short form. */
  async engines(query: EnginesQuery, options: RequestOptions = {}): Promise<EngineOption[]> {
    return this.#request("GET", `/v1/vehicles/engines${queryString(query)}`, undefined, options);
  }

  /** Catalog entries for free text such as `2019 civic si`, best first. */
  async search(query: SearchQuery, options: RequestOptions = {}): Promise<Entry[]> {
    return this.#request("GET", `/v1/vehicles/search${queryString(query)}`, undefined, options);
  }

  /** One catalog entry by its id, such as `2019_honda_civic_si`. */
  async vehicle(id: string, options: RequestOptions = {}): Promise<Entry> {
    return this.#request("GET", `/v1/vehicles/${segment("id", id)}`, undefined, options);
  }

  /** The data version, the vPIC release it came from, and the build time. */
  async meta(options: RequestOptions = {}): Promise<Meta> {
    return this.#request("GET", "/v1/meta", undefined, options);
  }

  async #request<T>(
    method: "GET" | "POST",
    path: string,
    body: unknown,
    options: RequestOptions,
  ): Promise<T> {
    const caller = options.signal;
    if (caller?.aborted) {
      throw new WenmarOpenError({ code: "aborted", message: "The request was aborted.", cause: caller.reason });
    }
    const timeoutMs = options.timeoutMs ?? this.timeoutMs;
    const controller = new AbortController();
    let timedOut = false;
    const timer =
      timeoutMs > 0
        ? setTimeout(() => {
            timedOut = true;
            controller.abort();
          }, timeoutMs)
        : undefined;
    const onAbort = () => controller.abort();
    caller?.addEventListener("abort", onAbort, { once: true });

    const headers: Record<string, string> = { Accept: "application/json" };
    const init: FetchInit = { method, headers, signal: controller.signal };
    if (body !== undefined) {
      headers["Content-Type"] = "application/json";
      init.body = JSON.stringify(body);
    }

    let status: number;
    let retryAfter: number | undefined;
    let text: string;
    try {
      // Called as a plain function, never as a method of this object: some
      // runtimes refuse a `fetch` whose `this` is not the global object.
      const send = this.#fetch;
      const response = await send(`${this.baseUrl}${path}`, init);
      status = response.status;
      retryAfter = parseRetryAfter(response.headers.get("retry-after"));
      // The time limit covers the body: it is read before the timer stops.
      text = await response.text();
    } catch (cause) {
      if (timedOut) {
        throw new WenmarOpenError({
          code: "timeout",
          message: `No answer within ${timeoutMs} ms.`,
          details: { timeout_ms: timeoutMs },
          cause,
        });
      }
      if (caller?.aborted) {
        throw new WenmarOpenError({ code: "aborted", message: "The request was aborted.", cause });
      }
      throw new WenmarOpenError({
        code: "network",
        message: `The API at ${this.baseUrl} could not be reached.`,
        cause,
      });
    } finally {
      if (timer !== undefined) clearTimeout(timer);
      caller?.removeEventListener("abort", onAbort);
    }

    let parsed: unknown;
    let isJson = true;
    try {
      parsed = JSON.parse(text);
    } catch {
      isJson = false;
    }

    if (status >= 200 && status < 300) {
      if (!isJson) {
        throw new WenmarOpenError({
          code: "bad_response",
          message: `The answer from ${this.baseUrl} is not JSON. Something other than the API may have answered.`,
          status,
        });
      }
      return parsed as T;
    }

    const error = isRecord(parsed) ? parsed["error"] : undefined;
    if (isRecord(error) && typeof error["code"] === "string" && typeof error["message"] === "string") {
      const details = isRecord(error["details"]) ? error["details"] : {};
      // The header is what the API promises. The body's copy is used when a
      // proxy in between dropped the header.
      const inBody = details["retry_after"];
      throw new WenmarOpenError({
        code: error["code"],
        message: error["message"],
        details,
        status,
        retryAfter: retryAfter ?? (typeof inBody === "number" ? inBody : undefined),
      });
    }
    throw new WenmarOpenError({
      code: "bad_response",
      message: `The server answered ${status} without the API's error body. A proxy in front of the API may have answered.`,
      status,
      retryAfter,
    });
  }
}
