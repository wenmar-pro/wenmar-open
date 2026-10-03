// The tests compile with @types/node 20 and without the DOM library, so
// two things Node has at run time are declared here, as far as the tests
// use them. If a later @types/node declares them, delete what clashes.

declare namespace WebAssembly {
  class Module {
    constructor(bytes: ArrayBuffer | ArrayBufferView);
    static exports(module: Module): { name: string; kind: string }[];
    static imports(module: Module): { module: string; name: string; kind: string }[];
  }
  class Instance {
    constructor(module: Module, imports?: object);
    readonly exports: Record<string, unknown>;
  }
  class RuntimeError extends Error {}
  let compile: (bytes: ArrayBuffer | ArrayBufferView) => Promise<Module>;
  let instantiate: (module: Module, imports?: object) => Promise<Instance>;
}

declare module "node:sqlite" {
  export class StatementSync {
    all(...params: (string | number | null)[]): unknown[];
    setReturnArrays(enabled: boolean): void;
    setReadBigInts(enabled: boolean): void;
  }
  export class DatabaseSync {
    constructor(path: string, options?: { readOnly?: boolean });
    exec(sql: string): void;
    prepare(sql: string): StatementSync;
    close(): void;
  }
}
