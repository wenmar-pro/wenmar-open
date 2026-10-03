import { WenmarOpenError } from "../errors.js";
import type { Statement } from "./store.js";

/**
 * A compiled `WebAssembly.Module`. It is written as `object` so that these
 * declarations name nothing a project without the DOM library lacks.
 */
export type CompiledWasm = object;

/**
 * The decoder: a compiled module, or its bytes. Cloudflare Workers do not
 * compile bytes, so there it is the module `import wasm from
 * "wenmar-open/offline.wasm"` gives.
 */
export type WasmSource = CompiledWasm | ArrayBuffer | ArrayBufferView;

/** A statement with the rows a store gave for it. */
export interface Answered extends Statement {
  readonly rows: readonly (readonly (string | number | null)[])[];
}

export interface EngineRequest {
  op: string;
  args?: object;
  current_year?: number;
  answers?: readonly Answered[];
}

export type EngineAnswer =
  | { need: Statement[] }
  | { ok: unknown }
  | { error: { code: string; message: string; details: Record<string, unknown> } };

interface Exports {
  memory: WebAssembly.Memory;
  wo_alloc(length: number): number;
  wo_call(pointer: number, length: number): number;
  wo_result(): number;
  wo_result_len(): number;
}

const EXPORTS = ["memory", "wo_alloc", "wo_call", "wo_result", "wo_result_len"] as const;

function bytesOf(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
  return bytes;
}

/**
 * Compiles the decoder. With no source it is the copy inside the package,
 * which is read as text and never fetched, so nothing here needs a file
 * system or a network.
 */
export async function compile(source?: WasmSource): Promise<CompiledWasm> {
  if (source instanceof WebAssembly.Module) return source;
  try {
    const bytes = source ?? bytesOf((await import("./wasm-inline.js")).WASM_BASE64);
    return await WebAssembly.compile(bytes as BufferSource);
  } catch (cause) {
    throw new WenmarOpenError({
      code: "internal_error",
      message:
        "The decoder could not be loaded. Where bytes may not be compiled, as in Cloudflare Workers, pass { wasm } from: import wasm from \"wenmar-open/offline.wasm\".",
      cause,
    });
  }
}

/** One running copy of the decoder. */
export class Engine {
  readonly #exports: Exports;
  #stopped = false;

  private constructor(exports: Exports) {
    this.#exports = exports;
  }

  static async start(module: CompiledWasm): Promise<Engine> {
    let instance: WebAssembly.Instance;
    try {
      instance = await WebAssembly.instantiate(module as WebAssembly.Module, {});
    } catch (cause) {
      throw new WenmarOpenError({
        code: "internal_error",
        message: "The decoder could not be started: the wasm given is not the one of this package.",
        cause,
      });
    }
    const exports = instance.exports as unknown as Exports;
    for (const name of EXPORTS) {
      if (exports[name] === undefined) {
        throw new WenmarOpenError({
          code: "internal_error",
          message: "The decoder could not be started: the wasm given is not the one of this package.",
          details: { missing: name },
        });
      }
    }
    return new Engine(exports);
  }

  /** Whether a call stopped this copy. A stopped copy is never used again. */
  get stopped(): boolean {
    return this.#stopped;
  }

  #read(): string {
    const { memory, wo_result, wo_result_len } = this.#exports;
    // The memory may have grown during the call, so it is looked at afresh.
    return new TextDecoder().decode(new Uint8Array(memory.buffer, wo_result(), wo_result_len()));
  }

  /**
   * One request, one answer. A request that cannot be written as JSON is
   * refused before the decoder sees it. A stop inside the decoder is thrown
   * as an error, and this copy is not used again.
   */
  call(request: EngineRequest): EngineAnswer {
    if (this.#stopped) {
      throw new WenmarOpenError({ code: "internal_error", message: "The decoder has stopped." });
    }
    let input: Uint8Array;
    try {
      input = new TextEncoder().encode(JSON.stringify(request));
    } catch (cause) {
      throw new WenmarOpenError({
        code: "validation_failed",
        message: "The question could not be written as JSON.",
        cause,
      });
    }
    const { memory, wo_alloc, wo_call } = this.#exports;
    try {
      const pointer = wo_alloc(input.length);
      new Uint8Array(memory.buffer, pointer, input.length).set(input);
      wo_call(pointer, input.length);
    } catch (cause) {
      // WebAssembly cannot unwind, so a failure inside it ends the call
      // here. What it was is left where the answer would have been.
      this.#stopped = true;
      let panic: string | undefined;
      try {
        panic = this.#read();
      } catch {
        panic = undefined;
      }
      // An answer is always a JSON object and a panic's message never is:
      // text that starts as an object is an answer left from before.
      if (panic === "" || panic?.startsWith("{")) panic = undefined;
      throw new WenmarOpenError({
        code: "internal_error",
        message: "The decoder stopped unexpectedly. Please report the VIN or the question that caused it.",
        details: panic === undefined ? {} : { panic },
        cause,
      });
    }
    try {
      return JSON.parse(this.#read()) as EngineAnswer;
    } catch (cause) {
      throw new WenmarOpenError({
        code: "internal_error",
        message: "The decoder gave an answer that could not be read.",
        cause,
      });
    }
  }
}
