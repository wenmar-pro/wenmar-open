import { MAX_BATCH } from "../batch.js";
import type { DecodeOptions, RequestOptions } from "../client.js";
import { WenmarOpenError } from "../errors.js";
import type {
  BatchItem,
  EngineOption,
  EnginesQuery,
  Entry,
  Make,
  MakesQuery,
  Meta,
  Model,
  ModelsQuery,
  ModelYears,
  Overview,
  RecallCampaign,
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
  VehicleRecalls,
  VehicleRecallsQuery,
  VehicleSafety,
  VehicleSpecs,
  VehicleTires,
  VinDecode,
  VinFluids,
  VinFuel,
  VinFuelQuery,
  VinRecalls,
  VinRecallsQuery,
  VinSpecs,
  VinTires,
  VinYearQuery,
  YearsQuery,
} from "../types.js";
import { Engine, compile } from "./engine.js";
import type { Answered, CompiledWasm, EngineAnswer, WasmSource } from "./engine.js";
import { cleanRows } from "./store.js";
import type { Statement, Store } from "./store.js";

/**
 * The most times one question, or the opening of the data, may go back to
 * the store. Opening takes 2; over the fixture of the tests a decode takes
 * at most 7 and a search at most 4. Reaching this means something is wrong.
 */
export const MAX_STEPS = 32;

export interface OfflineOptions {
  /** Where the data is: `syncStore(...)`, `d1Store(...)`, or your own. */
  store: Store;
  /**
   * Where the shop file is: a second database, which holds the recalls.
   * It must be the shop file that was built against the data file in
   * `store`. Leave it out and the recall methods fail with
   * `dataset_unavailable`; everything else works.
   */
  shop?: Store;
  /**
   * The decoder. Leave it out everywhere but Cloudflare Workers, where it
   * is the module `import wasm from "wenmar-open/offline.wasm"` gives.
   */
  wasm?: WasmSource;
  /**
   * The current calendar year, which bounds the model years a VIN can
   * have. Default: the year of the device's clock, in UTC.
   */
  currentYear?: number;
}

interface Question {
  op: string;
  args: object;
}

type Failure = { code: string; message: string; details: Record<string, unknown> };
type Outcome = { ok: unknown } | { error: Failure };

const keyOf = (statement: Statement): string =>
  `${statement.db ?? ""}\u0000${statement.sql}\u0000${JSON.stringify(statement.params)}`;

/**
 * The operations that are not about recalls alone and still read the shop
 * file when there is one. Without a shop store they answer all the same.
 */
const READS_SHOP = new Set(["meta", "overview_vehicle", "overview_vin", "model_years"]);

/** Whether a question reads the shop file, when there is one. */
const needsShop = (question: Question): boolean =>
  READS_SHOP.has(question.op) || question.op.startsWith("recall") || question.op === "safety" || FACTS.test(question.op);

/** The operations for fuel, specifications, fluids and tire fitments, such as `fuel_vin`. */
const FACTS = /^(fuel|specs|fluids|tires)_(vehicle|vin)$/;

function thrown(failure: Failure): WenmarOpenError {
  return new WenmarOpenError({ code: failure.code, message: failure.message, details: failure.details });
}

/**
 * The rows of every statement, from the store. A store that throws, and one
 * that does not return one list for each statement, are `store_error`: the
 * same whether the data is being opened or a question answered.
 */
async function read(store: Store, statements: readonly Statement[]): Promise<unknown[][][]> {
  let rows: unknown;
  try {
    rows = await store.query(statements);
  } catch (cause) {
    if (cause instanceof WenmarOpenError) throw cause;
    throw new WenmarOpenError({
      code: "store_error",
      message: "The database failed while it was being read.",
      cause,
    });
  }
  if (!Array.isArray(rows) || rows.length !== statements.length) {
    throw new WenmarOpenError({
      code: "store_error",
      message: "The store must return one list of rows for each statement.",
    });
  }
  return rows as unknown[][][];
}

/**
 * VIN decoding and the vehicle catalog with no network: the methods of
 * `WenmarOpen`, answered from a data file by the decoder inside the package.
 * Every method gives what the hosted API gives for the same question and
 * fails with the same `WenmarOpenError` codes.
 */
export class WenmarOpenOffline {
  readonly #store: Store;
  readonly #shop: Store | undefined;
  /** The copy of the decoder the shop file was opened on, and that opening. */
  #shopOpened: { engine: Engine; done: Promise<void> } | undefined;
  readonly #wasm: WasmSource | undefined;
  readonly #currentYear: number | undefined;
  #module: Promise<CompiledWasm> | undefined;
  #engine: Promise<Engine> | undefined;

  constructor(options: OfflineOptions) {
    this.#store = options.store;
    this.#shop = options.shop;
    this.#wasm = options.wasm;
    this.#currentYear = options.currentYear;
  }

  /** Decode one VIN. Spaces, dashes and lowercase are accepted. */
  async decodeVin(vin: string, options: DecodeOptions = {}): Promise<VinDecode> {
    return this.#one({ op: "decode", args: { vin: String(vin), year: options.year } }, options);
  }

  /**
   * Decode up to 50 VINs. The answer has one item per VIN, in order: a
   * decode, or the error a single call for that VIN would have thrown.
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
    const questions = vins.map((vin) => ({ op: "decode", args: { vin: String(vin) } }));
    const outcomes = await this.#ask(questions, options);
    return outcomes.map((outcome) => ("ok" in outcome ? outcome.ok : { error: outcome.error }) as BatchItem);
  }

  /** Model years, newest first. */
  async years(query: YearsQuery = {}, options: RequestOptions = {}): Promise<number[]> {
    return this.#one({ op: "years", args: query }, options);
  }

  /** Makes, popular ones first and the rest by name. */
  async makes(query: MakesQuery = {}, options: RequestOptions = {}): Promise<Make[]> {
    return this.#one({ op: "makes", args: query }, options);
  }

  /** Models of a make, by name. An unknown make has no models. */
  async models(query: ModelsQuery, options: RequestOptions = {}): Promise<Model[]> {
    return this.#one({ op: "models", args: query }, options);
  }

  /** Submodels of a model year: trims, or series where there are no trims. */
  async submodels(query: SubmodelsQuery, options: RequestOptions = {}): Promise<Submodel[]> {
    return this.#one({ op: "submodels", args: query }, options);
  }

  /** The same as `submodels`, under the name most people use. */
  async trims(query: SubmodelsQuery, options: RequestOptions = {}): Promise<Submodel[]> {
    return this.#one({ op: "submodels", args: query }, options);
  }

  /** Engines of a model year, in short form. */
  async engines(query: EnginesQuery, options: RequestOptions = {}): Promise<EngineOption[]> {
    return this.#one({ op: "engines", args: query }, options);
  }

  /**
   * Catalog entries for free text such as `2019 civic si`, best first. The
   * hosted API also finds words in any order with an index of its own;
   * here, text the catalog cannot read as year, make, model and submodel
   * finds nothing.
   */
  async search(query: SearchQuery, options: RequestOptions = {}): Promise<Entry[]> {
    return this.#one({ op: "search", args: query }, options);
  }

  /** One catalog entry by its id, such as `2019_honda_civic_si`. */
  async vehicle(id: string, options: RequestOptions = {}): Promise<Entry> {
    return this.#one({ op: "vehicle", args: { id: String(id) } }, options);
  }

  /**
   * NHTSA's crash test ratings for a model year, by its id such as
   * `2018_ford_f-150`. NHTSA rates by body style and drive, so `ratings`
   * often has several entries: each names the vehicle it is for. Needs a
   * shop file that holds the ratings: without one it fails with
   * `dataset_unavailable`.
   */
  async safety(id: string, options: RequestOptions = {}): Promise<VehicleSafety> {
    return this.#one({ op: "safety", args: { id: String(id) } }, options);
  }

  /**
   * Fuel consumption ratings of a model year, from EPA and Natural
   * Resources Canada. Each says in `confidence` how far it was matched:
   * `model`, `engine` or `submodel`. Needs a shop file that holds them:
   * without one it fails with `dataset_unavailable`.
   */
  async fuel(id: string, query: VehicleFuelQuery = {}, options: RequestOptions = {}): Promise<VehicleFuel> {
    return this.#one({ op: "fuel_vehicle", args: { ...query, id: String(id) } }, options);
  }

  /** Fuel ratings for the vehicle a VIN decodes to: one engine's when the decode settles it. */
  async vinFuel(vin: string, query: VinFuelQuery = {}, options: RequestOptions = {}): Promise<VinFuel> {
    return this.#one({ op: "fuel_vin", args: { ...query, vin: String(vin) } }, options);
  }

  /** Specifications of a model year from EPA's files, one entry per EPA rating. */
  async specs(id: string, options: RequestOptions = {}): Promise<VehicleSpecs> {
    return this.#one({ op: "specs_vehicle", args: { id: String(id) } }, options);
  }

  /** Specifications for the vehicle a VIN decodes to: one engine's when the decode settles it. */
  async vinSpecs(vin: string, query: VinYearQuery = {}, options: RequestOptions = {}): Promise<VinSpecs> {
    return this.#one({ op: "specs_vin", args: { ...query, vin: String(vin) } }, options);
  }

  /**
   * Fluids of a model year: the engine oil viscosity grade the maker
   * certified each engine with, per EPA, and the records a person read from
   * the maker's own documents, each with its source and the day it was checked.
   */
  async fluids(id: string, options: RequestOptions = {}): Promise<VehicleFluids> {
    return this.#one({ op: "fluids_vehicle", args: { id: String(id) } }, options);
  }

  /** Fluids for the vehicle a VIN decodes to: one engine's when the decode settles it. */
  async vinFluids(vin: string, query: VinYearQuery = {}, options: RequestOptions = {}): Promise<VinFluids> {
    return this.#one({ op: "fluids_vin", args: { ...query, vin: String(vin) } }, options);
  }

  /**
   * Original-equipment tire fitments of a model year. Every size on record
   * is listed with the trims it covers; none is picked.
   */
  async tires(id: string, options: RequestOptions = {}): Promise<VehicleTires> {
    return this.#one({ op: "tires_vehicle", args: { id: String(id) } }, options);
  }

  /** Tire fitments of the model year a VIN decodes to. Never narrowed: a VIN does not name the wheels. */
  async vinTires(vin: string, query: VinYearQuery = {}, options: RequestOptions = {}): Promise<VinTires> {
    return this.#one({ op: "tires_vin", args: { ...query, vin: String(vin) } }, options);
  }

  /**
   * Recalls that may apply to a model year, by its id such as
   * `2018_ford_f-150`. The list is for the year, make and model: it is not
   * a lookup by VIN, and `lookup` says where one vehicle can be checked.
   * Needs a shop file: without one it fails with `dataset_unavailable`.
   */
  async recalls(
    id: string,
    query: VehicleRecallsQuery = {},
    options: RequestOptions = {},
  ): Promise<VehicleRecalls> {
    return this.#one({ op: "recalls_vehicle", args: { ...query, id: String(id) } }, options);
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
    return this.#one({ op: "recalls_vin", args: { ...query, vin: String(vin) } }, options);
  }

  /** One recall campaign: `source` is `nhtsa` or `tc`. */
  async recall(source: string, campaign: string, options: RequestOptions = {}): Promise<RecallCampaign> {
    return this.#one({ op: "recall", args: { source: String(source), campaign: String(campaign) } }, options);
  }

  /** Recall campaigns, newest first, narrowed by what is given. */
  async searchRecalls(query: RecallSearchQuery = {}, options: RequestOptions = {}): Promise<RecallList> {
    return this.#one({ op: "recalls_search", args: query }, options);
  }

  /**
   * Everything held for a model year in one answer, by its id such as
   * `2018_ford_f-150`. A block the files do not hold is `null`. `recalls`
   * counts the campaigns that may apply to the year, make and model: it is
   * not a lookup by VIN. With no shop file `recalls` is `null`, and the
   * method still answers.
   */
  async overview(id: string, options: RequestOptions = {}): Promise<Overview> {
    return this.#one({ op: "overview_vehicle", args: { id: String(id) } }, options);
  }

  /**
   * The overview of the model year a VIN decodes to, with `decode`. The VIN
   * only names the model year: no public data says a recall applies to one
   * VIN.
   */
  async vinOverview(vin: string, options: DecodeOptions = {}): Promise<Overview> {
    return this.#one({ op: "overview_vin", args: { vin: String(vin), year: options.year } }, options);
  }

  /**
   * Every model year of one model, newest first, each with its vehicle id
   * and the number of recall campaigns that may apply to it: `null` with no
   * shop file.
   */
  async modelYears(make: string, model: string, options: RequestOptions = {}): Promise<ModelYears> {
    return this.#one({ op: "model_years", args: { make: String(make), model: String(model) } }, options);
  }

  /**
   * Recall campaigns reported on or after `since` (a day such as
   * `2026-09-01`), newest first, each with the vehicles it may apply to.
   * Needs a shop file: without one it fails with `dataset_unavailable`.
   */
  async recentRecalls(query: RecentRecallsQuery, options: RequestOptions = {}): Promise<RecentRecalls> {
    return this.#one({ op: "recalls_recent", args: query }, options);
  }

  /**
   * The data version, the vPIC release it came from, and the build time.
   * `server_version` is the version of this package. `datasets` lists what
   * the shop file holds, and is empty when no shop store was given.
   */
  async meta(options: RequestOptions = {}): Promise<Meta> {
    return this.#one({ op: "meta", args: {} }, options);
  }

  /** Closes the stores, if they were opened for this client. */
  close(): void {
    this.#store.close?.();
    this.#shop?.close?.();
  }

  async #one<T>(question: Question, options: RequestOptions): Promise<T> {
    const [outcome] = await this.#ask([question], options);
    if (outcome === undefined) {
      throw new WenmarOpenError({ code: "internal_error", message: "The decoder gave no answer." });
    }
    if ("error" in outcome) throw thrown(outcome.error);
    return outcome.ok as T;
  }

  /** The decoder, started and with the data opened. Started again after a stop. */
  async #ready(): Promise<Engine> {
    const current = this.#engine;
    if (current !== undefined) {
      const engine = await current;
      if (!engine.stopped) return engine;
      if (this.#engine === current) this.#engine = undefined;
    }
    this.#engine ??= this.#start();
    try {
      return await this.#engine;
    } catch (error) {
      this.#engine = undefined;
      throw error;
    }
  }

  async #start(): Promise<Engine> {
    this.#module ??= compile(this.#wasm);
    let module: CompiledWasm;
    try {
      module = await this.#module;
    } catch (error) {
      this.#module = undefined;
      throw error;
    }
    const engine = await Engine.start(module);
    const answers: Answered[] = [];
    for (let step = 0; step < MAX_STEPS; step += 1) {
      const answer = engine.call({ op: "open", answers });
      if ("ok" in answer) return engine;
      if ("error" in answer) throw thrown(answer.error);
      const rows = await read(this.#store, answer.need);
      answer.need.forEach((statement, index) => {
        answers.push({ ...statement, rows: cleanRows(rows[index], statement.sql) });
      });
    }
    throw new WenmarOpenError({ code: "internal_error", message: "Opening the data did not finish." });
  }

  /**
   * The rows of every statement, each from the store its file is in: a
   * statement marked `db: "shop"` is for the shop file.
   */
  async #read(statements: readonly Statement[]): Promise<unknown[][][]> {
    const ofShop = statements.map((statement) => statement.db === "shop");
    if (!ofShop.includes(true)) return read(this.#store, statements);
    const shop = this.#shop;
    if (shop === undefined) {
      throw new WenmarOpenError({
        code: "internal_error",
        message: "The decoder asked for the shop file, and no shop store was given.",
      });
    }
    const forData = statements.filter((_, index) => !ofShop[index]);
    const forShop = statements.filter((_, index) => ofShop[index]);
    const [fromData, fromShop] = await Promise.all([
      forData.length === 0 ? [] : read(this.#store, forData),
      read(shop, forShop),
    ]);
    let [data, other] = [0, 0];
    return statements.map((_, index) => (ofShop[index] ? fromShop[other++] : fromData[data++]) ?? []);
  }

  /**
   * Opens the shop file on `engine`, once for each copy of the decoder. It
   * is opened when a question first needs it, so a shop store that fails
   * does not stop a VIN from being decoded.
   */
  async #withShop(engine: Engine): Promise<void> {
    if (this.#shopOpened?.engine !== engine) {
      this.#shopOpened = { engine, done: this.#openShop(engine) };
    }
    const opening = this.#shopOpened;
    try {
      await opening.done;
    } catch (error) {
      if (this.#shopOpened === opening) this.#shopOpened = undefined;
      throw error;
    }
  }

  async #openShop(engine: Engine): Promise<void> {
    const answers: Answered[] = [];
    for (let step = 0; step < MAX_STEPS; step += 1) {
      const answer = engine.call({ op: "open_shop", answers });
      if ("ok" in answer) return;
      if ("error" in answer) throw thrown(answer.error);
      const rows = await this.#read(answer.need);
      answer.need.forEach((statement, index) => {
        answers.push({ ...statement, rows: cleanRows(rows[index], statement.sql) });
      });
    }
    throw new WenmarOpenError({ code: "internal_error", message: "Opening the shop file did not finish." });
  }

  /**
   * Answers every question, going back to the store as often as the
   * decoder asks. What several questions need in the same step is read
   * together, each statement once.
   */
  async #ask(questions: readonly Question[], options: RequestOptions): Promise<Outcome[]> {
    const signal = options.signal;
    const timeoutMs = options.timeoutMs ?? 0;
    const started = Date.now();
    const currentYear = this.#currentYear ?? new Date().getUTCFullYear();
    const answers: Answered[][] = questions.map(() => []);
    const outcomes: (Outcome | undefined)[] = questions.map(() => undefined);

    for (let step = 0; ; step += 1) {
      if (signal?.aborted) {
        throw new WenmarOpenError({ code: "aborted", message: "The request was aborted.", cause: signal.reason });
      }
      if (timeoutMs > 0 && Date.now() - started > timeoutMs) {
        throw new WenmarOpenError({
          code: "timeout",
          message: `No answer within ${timeoutMs} ms.`,
          details: { timeout_ms: timeoutMs },
        });
      }
      const engine = await this.#ready();
      if (this.#shop !== undefined && questions.some(needsShop)) await this.#withShop(engine);
      const wanted = new Map<string, Statement>();
      const waiting: { index: number; need: Statement[] }[] = [];
      questions.forEach((question, index) => {
        if (outcomes[index] !== undefined) return;
        const answer: EngineAnswer = engine.call({
          op: question.op,
          args: question.args,
          current_year: currentYear,
          answers: answers[index] ?? [],
        });
        if ("need" in answer) {
          waiting.push({ index, need: answer.need });
          for (const statement of answer.need) wanted.set(keyOf(statement), statement);
        } else {
          outcomes[index] = answer;
        }
      });
      if (waiting.length === 0) return outcomes as Outcome[];
      if (step >= MAX_STEPS) {
        throw new WenmarOpenError({ code: "internal_error", message: "The decoder kept asking for more data." });
      }

      const statements = [...wanted.values()];
      const rows = await this.#read(statements);
      const found = new Map<string, Answered>();
      statements.forEach((statement, index) => {
        found.set(keyOf(statement), { ...statement, rows: cleanRows(rows[index], statement.sql) });
      });
      for (const { index, need } of waiting) {
        for (const statement of need) {
          const answered = found.get(keyOf(statement));
          if (answered !== undefined) answers[index]?.push(answered);
        }
      }
    }
  }
}
