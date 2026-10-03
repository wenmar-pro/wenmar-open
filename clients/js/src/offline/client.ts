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
  SearchQuery,
  Submodel,
  SubmodelsQuery,
  VinDecode,
  YearsQuery,
} from "../types.js";
import { Engine, compile } from "./engine.js";
import type { Answered, CompiledWasm, EngineAnswer, WasmSource } from "./engine.js";
import { cleanRows } from "./store.js";
import type { Statement, Store } from "./store.js";

/**
 * The most times one question may go back to the store. A decode needs 8 to
 * 10 and a search about 6; reaching this means something is wrong.
 */
export const MAX_STEPS = 32;

export interface OfflineOptions {
  /** Where the data is: `syncStore(...)`, `d1Store(...)`, or your own. */
  store: Store;
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

const keyOf = (statement: Statement): string => `${statement.sql}\u0000${JSON.stringify(statement.params)}`;

function thrown(failure: Failure): WenmarOpenError {
  return new WenmarOpenError({ code: failure.code, message: failure.message, details: failure.details });
}

/**
 * VIN decoding and the vehicle catalog with no network: the methods of
 * `WenmarOpen`, answered from a data file by the decoder inside the package.
 * Every method gives what the hosted API gives for the same question and
 * fails with the same `WenmarOpenError` codes.
 */
export class WenmarOpenOffline {
  readonly #store: Store;
  readonly #wasm: WasmSource | undefined;
  readonly #currentYear: number | undefined;
  #module: Promise<CompiledWasm> | undefined;
  #engine: Promise<Engine> | undefined;

  constructor(options: OfflineOptions) {
    this.#store = options.store;
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
   * The data version, the vPIC release it came from, and the build time.
   * `server_version` is the version of this package.
   */
  async meta(options: RequestOptions = {}): Promise<Meta> {
    return this.#one({ op: "meta", args: {} }, options);
  }

  /** Closes the store, if the store was opened for this client. */
  close(): void {
    this.#store.close?.();
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
      let rows: unknown[][][];
      try {
        rows = await this.#store.query(answer.need);
      } catch (cause) {
        if (cause instanceof WenmarOpenError) throw cause;
        throw new WenmarOpenError({
          code: "data_invalid",
          message: "The database could not be read as a Wenmar Open data file.",
          cause,
        });
      }
      answer.need.forEach((statement, index) => {
        answers.push({ ...statement, rows: cleanRows(rows[index], statement.sql) });
      });
    }
    throw new WenmarOpenError({ code: "internal_error", message: "Opening the data did not finish." });
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
      let rows: unknown[][][];
      try {
        rows = await this.#store.query(statements);
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
