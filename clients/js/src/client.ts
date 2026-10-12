import { MAX_BATCH } from "./batch.js";
import { WenmarOpenError, parseRetryAfter } from "./errors.js";
import type {
  BatchItem,
  BulletinDetail,
  BulletinFilterQuery,
  BulletinList,
  BulletinSearchQuery,
  Code,
  CodeList,
  CodeSearchQuery,
  EngineOption,
  EnginesQuery,
  Entry,
  Make,
  MakeDetail,
  MakesQuery,
  Meta,
  Model,
  ModelYears,
  ModelsQuery,
  Overview,
  PartsCard,
  RecallCampaign,
  RecallInvestigations,
  RecallList,
  RecallSearchQuery,
  RecentRecalls,
  RecentRecallsQuery,
  SearchQuery,
  Submodel,
  SubmodelsQuery,
  VehicleFluids,
  VehicleFuel,
  VehicleFuelQuery,
  VehicleInvestigations,
  VehicleInvestigationsQuery,
  VehicleBulletins,
  VehicleCode,
  VehicleRecalls,
  VehicleRecallsQuery,
  VehicleComplaints,
  VehicleSafety,
  VehicleSpecs,
  VehicleTires,
  VinDecode,
  VinFluids,
  VinFuel,
  VinFuelQuery,
  VinBulletins,
  VinBulletinsQuery,
  VinRecalls,
  VinRecallsQuery,
  VinSpecs,
  VinTires,
  VinYearQuery,
  YearsQuery,
} from "./types.js";

/** The hosted API. */
export const DEFAULT_BASE_URL = "https://open.wenmarpro.com";
/** How long a request may take before it fails with `timeout`. */
export const DEFAULT_TIMEOUT_MS = 10_000;

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

export interface BatchOptions extends RequestOptions {
  /**
   * `"recalls"` adds to each decode the number of recall campaigns that may
   * apply to the model year it reaches, as `recalls`.
   */
  include?: "recalls";
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
  async decodeVins(vins: readonly string[], options: BatchOptions = {}): Promise<BatchItem[]> {
    if (vins.length > MAX_BATCH) {
      throw new WenmarOpenError({
        code: "validation_failed",
        message: `a batch holds at most ${MAX_BATCH} VINs`,
        details: { field: "vins", max: MAX_BATCH, received: vins.length },
      });
    }
    const path = `/v1/vin/batch${queryString({ include: options.include })}`;
    return this.#request("POST", path, { vins }, options);
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

  /**
   * NHTSA's crash test ratings for a model year, by its id such as
   * `2018_ford_f-150`. NHTSA rates by body style and drive, so `ratings`
   * often has several entries: each names the vehicle it is for.
   */
  async safety(id: string, options: RequestOptions = {}): Promise<VehicleSafety> {
    return this.#request("GET", `/v1/vehicles/${segment("id", id)}/safety`, undefined, options);
  }

  /**
   * What owners reported to NHTSA about a model year, as counts by
   * component. These are owners' reports: not verified, and not a rate.
   */
  async complaints(id: string, options: RequestOptions = {}): Promise<VehicleComplaints> {
    return this.#request("GET", `/v1/vehicles/${segment("id", id)}/complaints`, undefined, options);
  }

  /**
   * One generic trouble code, such as `P0420`, described in this project's
   * own words. A manufacturer-specific code has no description: the call
   * fails with `not_found`.
   */
  async code(code: string, options: RequestOptions = {}): Promise<Code> {
    return this.#request("GET", `/v1/codes/${segment("code", code)}`, undefined, options);
  }

  /** The trouble codes that are described, by code, narrowed by text or system. */
  async codes(query: CodeSearchQuery = {}, options: RequestOptions = {}): Promise<CodeList> {
    return this.#request("GET", `/v1/codes${queryString(query)}`, undefined, options);
  }

  /**
   * A trouble code on one model year: its description, the bulletins of
   * that model year that name it, and the bulletins and recalls about the
   * same components. Any code answers, a manufacturer-specific one too.
   */
  async vehicleCode(id: string, code: string, options: RequestOptions = {}): Promise<VehicleCode> {
    const path = `/v1/vehicles/${segment("id", id)}/codes/${segment("code", code)}`;
    return this.#request("GET", path, undefined, options);
  }

  /** One make by its id, name or alias, such as `honda`, with the maker's own links. */
  async make(make: string, options: RequestOptions = {}): Promise<MakeDetail> {
    return this.#request("GET", `/v1/makes/${segment("make", make)}`, undefined, options);
  }

  /**
   * Recalls that may apply to a model year, by its id such as
   * `2018_ford_f-150`. The list is for the year, make and model: it is not
   * a lookup by VIN, and `lookup` says where one vehicle can be checked.
   */
  async recalls(
    id: string,
    query: VehicleRecallsQuery = {},
    options: RequestOptions = {},
  ): Promise<VehicleRecalls> {
    const path = `/v1/vehicles/${segment("id", id)}/recalls${queryString(query)}`;
    return this.#request("GET", path, undefined, options);
  }

  /**
   * Recalls that may apply to the model year a VIN decodes to. The VIN only
   * names the model year: no public data says a recall applies to one VIN.
   */
  async vinRecalls(
    vin: string,
    query: VinRecallsQuery = {},
    options: RequestOptions = {},
  ): Promise<VinRecalls> {
    const path = `/v1/vin/${segment("vin", vin)}/recalls${queryString(query)}`;
    return this.#request("GET", path, undefined, options);
  }

  /** One recall campaign: `source` is `nhtsa` or `tc`. */
  async recall(source: string, campaign: string, options: RequestOptions = {}): Promise<RecallCampaign> {
    const path = `/v1/recalls/${segment("source", source)}/${segment("campaign", campaign)}`;
    return this.#request("GET", path, undefined, options);
  }

  /** Recall campaigns, newest first, narrowed by what is given. */
  async searchRecalls(query: RecallSearchQuery = {}, options: RequestOptions = {}): Promise<RecallList> {
    return this.#request("GET", `/v1/recalls${queryString(query)}`, undefined, options);
  }

  /**
   * Manufacturer communications that list a model year: service bulletins,
   * service campaigns, warranty extensions and over-the-air updates, one
   * page, newest first. `counts` says how many there are of each kind.
   */
  async bulletins(
    id: string,
    query: BulletinFilterQuery = {},
    options: RequestOptions = {},
  ): Promise<VehicleBulletins> {
    const path = `/v1/vehicles/${segment("id", id)}/bulletins${queryString(query)}`;
    return this.#request("GET", path, undefined, options);
  }

  /** Manufacturer communications for the model year a VIN decodes to. */
  async vinBulletins(
    vin: string,
    query: VinBulletinsQuery = {},
    options: RequestOptions = {},
  ): Promise<VinBulletins> {
    const path = `/v1/vin/${segment("vin", vin)}/bulletins${queryString(query)}`;
    return this.#request("GET", path, undefined, options);
  }

  /** One manufacturer communication, by NHTSA's id number. */
  async bulletin(nhtsaId: number | string, options: RequestOptions = {}): Promise<BulletinDetail> {
    const path = `/v1/bulletins/${segment("nhtsa_id", String(nhtsaId))}`;
    return this.#request("GET", path, undefined, options);
  }

  /** Manufacturer communications, newest first, narrowed by what is given. */
  async searchBulletins(query: BulletinSearchQuery = {}, options: RequestOptions = {}): Promise<BulletinList> {
    return this.#request("GET", `/v1/bulletins${queryString(query)}`, undefined, options);
  }

  /**
   * Fuel consumption ratings of a model year, from EPA and Natural Resources
   * Canada. Each rating says how far it was matched in `confidence`, and
   * names a catalog engine only when the match settled one.
   */
  async fuel(id: string, query: VehicleFuelQuery = {}, options: RequestOptions = {}): Promise<VehicleFuel> {
    const path = `/v1/vehicles/${segment("id", id)}/fuel${queryString(query)}`;
    return this.#request("GET", path, undefined, options);
  }

  /**
   * Fuel consumption ratings for the vehicle a VIN decodes to: narrowed to
   * its engine when `engine_settled` is `true`, the model year's otherwise.
   */
  async vinFuel(vin: string, query: VinFuelQuery = {}, options: RequestOptions = {}): Promise<VinFuel> {
    const path = `/v1/vin/${segment("vin", vin)}/fuel${queryString(query)}`;
    return this.#request("GET", path, undefined, options);
  }

  /** Specifications of a model year from EPA's files, one entry per rating. */
  async specs(id: string, options: RequestOptions = {}): Promise<VehicleSpecs> {
    return this.#request("GET", `/v1/vehicles/${segment("id", id)}/specs`, undefined, options);
  }

  /** Specifications for the vehicle a VIN decodes to. */
  async vinSpecs(vin: string, query: VinYearQuery = {}, options: RequestOptions = {}): Promise<VinSpecs> {
    const path = `/v1/vin/${segment("vin", vin)}/specs${queryString(query)}`;
    return this.#request("GET", path, undefined, options);
  }

  /**
   * Fluids of a model year: the engine oil viscosity grade the maker
   * certified each engine with, per EPA, and the records a person read from
   * the maker's own documents, each with its source and the day it was checked.
   */
  async fluids(id: string, options: RequestOptions = {}): Promise<VehicleFluids> {
    return this.#request("GET", `/v1/vehicles/${segment("id", id)}/fluids`, undefined, options);
  }

  /** Fluids for the vehicle a VIN decodes to: one engine's when the decode settles it. */
  async vinFluids(vin: string, query: VinYearQuery = {}, options: RequestOptions = {}): Promise<VinFluids> {
    const path = `/v1/vin/${segment("vin", vin)}/fluids${queryString(query)}`;
    return this.#request("GET", path, undefined, options);
  }

  /**
   * Original-equipment tire fitments of a model year, read by a person from
   * the door placard or the owner's manual. Every size on record is listed
   * with the trims it covers; none is picked.
   */
  async tires(id: string, options: RequestOptions = {}): Promise<VehicleTires> {
    return this.#request("GET", `/v1/vehicles/${segment("id", id)}/tires`, undefined, options);
  }

  /** Tire fitments of the model year a VIN decodes to. Never narrowed: a VIN does not name the wheels. */
  async vinTires(vin: string, query: VinYearQuery = {}, options: RequestOptions = {}): Promise<VinTires> {
    const path = `/v1/vin/${segment("vin", vin)}/tires${queryString(query)}`;
    return this.#request("GET", path, undefined, options);
  }

  /**
   * The parts-ordering card for a VIN: what a parts counter asks for, each
   * line with where it came from. `text` is the card ready to paste.
   */
  async partsCard(vin: string, query: VinYearQuery = {}, options: RequestOptions = {}): Promise<PartsCard> {
    const path = `/v1/vin/${segment("vin", vin)}/parts-card${queryString(query)}`;
    return this.#request("GET", path, undefined, options);
  }

  /**
   * Everything held for a model year in one answer, by its id such as
   * `2018_ford_f-150`. A block the server has no data for is `null`.
   * `recalls` counts the campaigns that may apply to the year, make and
   * model: it is not a lookup by VIN.
   */
  async overview(id: string, options: RequestOptions = {}): Promise<Overview> {
    return this.#request("GET", `/v1/vehicles/${segment("id", id)}/overview`, undefined, options);
  }

  /**
   * The overview of the model year a VIN decodes to, with `decode`. The VIN
   * only names the model year: no public data says a recall applies to one
   * VIN.
   */
  async vinOverview(vin: string, options: DecodeOptions = {}): Promise<Overview> {
    const path = `/v1/vin/${segment("vin", vin)}/overview${queryString({ year: options.year })}`;
    return this.#request("GET", path, undefined, options);
  }

  /**
   * Every model year of one model, newest first, each with its vehicle id
   * and the number of recall campaigns that may apply to it.
   */
  async modelYears(make: string, model: string, options: RequestOptions = {}): Promise<ModelYears> {
    const path = `/v1/models/${segment("make", make)}/${segment("model", model)}`;
    return this.#request("GET", path, undefined, options);
  }

  /**
   * Recall campaigns reported on or after `since` (a day such as
   * `2026-09-01`), newest first, each with the vehicles it may apply to.
   */
  async recentRecalls(query: RecentRecallsQuery, options: RequestOptions = {}): Promise<RecentRecalls> {
    return this.#request("GET", `/v1/recalls/recent${queryString(query)}`, undefined, options);
  }

  /**
   * The open NHTSA investigations that list a model year, by its id such as
   * `2019_ram_1500`; `{ status: "all" }` adds the closed ones. An
   * investigation is not a finding that a defect exists: show `note`.
   */
  async investigations(
    id: string,
    query: VehicleInvestigationsQuery = {},
    options: RequestOptions = {},
  ): Promise<VehicleInvestigations> {
    const path = `/v1/vehicles/${segment("id", id)}/investigations${queryString(query)}`;
    return this.#request("GET", path, undefined, options);
  }

  /** The NHTSA investigations that led to a recall campaign: `source` is `nhtsa` or `tc`. */
  async recallInvestigations(
    source: string,
    campaign: string,
    options: RequestOptions = {},
  ): Promise<RecallInvestigations> {
    const path = `/v1/recalls/${segment("source", source)}/${segment("campaign", campaign)}/investigations`;
    return this.#request("GET", path, undefined, options);
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
