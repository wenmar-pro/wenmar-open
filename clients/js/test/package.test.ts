// What is published: the files in the package, and what it depends on.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { homedir } from "node:os";
import { test } from "node:test";

/** clients/js/, from .test-build/. */
const ROOT = new URL("../", import.meta.url);

const manifest = JSON.parse(readFileSync(new URL("package.json", ROOT), "utf8")) as Record<string, unknown>;

/** The files of the hosted client. Everything else in dist/ is the offline mode. */
const HOSTED = ["batch.js", "client.js", "errors.js", "index.js", "schema.js", "types.js"];
const OFFLINE = ["client.js", "engine.js", "index.js", "node.js", "store.js"];

/** The most the WebAssembly may weigh. It was 449,772 bytes when this was written. */
const WASM_BUDGET = 600_000;

function wasmBytes(): Buffer {
  return readFileSync(new URL("dist/offline/wenmar_open.wasm", ROOT));
}

/** Asks the decoder one question, with nothing but the module's own exports. */
function ask(request: object): unknown {
  const instance = new WebAssembly.Instance(new WebAssembly.Module(wasmBytes()));
  const exports = instance.exports as {
    memory: { buffer: ArrayBuffer };
    wo_alloc(length: number): number;
    wo_call(pointer: number, length: number): number;
    wo_result(): number;
    wo_result_len(): number;
  };
  const input = new TextEncoder().encode(JSON.stringify(request));
  const pointer = exports.wo_alloc(input.length);
  new Uint8Array(exports.memory.buffer, pointer, input.length).set(input);
  exports.wo_call(pointer, input.length);
  const output = new Uint8Array(exports.memory.buffer, exports.wo_result(), exports.wo_result_len());
  return JSON.parse(new TextDecoder().decode(output));
}

test("the package has no runtime dependencies and runs no script on install", () => {
  for (const field of ["dependencies", "optionalDependencies", "bundledDependencies"]) {
    assert.equal(manifest[field], undefined, `package.json has ${field}`);
  }
  const scripts = manifest["scripts"] as Record<string, string>;
  for (const hook of ["preinstall", "install", "postinstall"]) {
    assert.equal(scripts[hook], undefined, `package.json runs a ${hook} script`);
  }
});

test("the one peer is the data package, optional, at the schema version the decoder reads", () => {
  // The data package's major version is the data file's schema version, so
  // this range is every data file this version of the client can read.
  const answer = ask({ op: "version" }) as { ok: { version: string; schema_version: string } };
  assert.deepEqual(manifest["peerDependencies"], { "wenmar-open-data": `^${answer.ok.schema_version}.0.0` });
  assert.deepEqual(manifest["peerDependenciesMeta"], { "wenmar-open-data": { optional: true } });
  // And the decoder is the one built from this version of the repository.
  assert.equal(answer.ok.version, manifest["version"]);
});

test("the package is ESM with type declarations and names its repository", () => {
  assert.equal(manifest["name"], "wenmar-open");
  assert.equal(manifest["type"], "module");
  assert.equal(manifest["license"], "MIT");
  assert.equal(manifest["sideEffects"], false);
  assert.deepEqual(manifest["exports"], {
    ".": { types: "./dist/index.d.ts", default: "./dist/index.js" },
    "./offline": { types: "./dist/offline/index.d.ts", default: "./dist/offline/index.js" },
    "./offline/node": { types: "./dist/offline/node.d.ts", default: "./dist/offline/node.js" },
    "./offline.wasm": "./dist/offline/wenmar_open.wasm",
    "./package.json": "./package.json",
  });
  // npm's trusted publishing refuses a package whose repository is not the
  // one the workflow runs in.
  assert.deepEqual(manifest["repository"], {
    type: "git",
    url: "git+https://github.com/wenmar-pro/wenmar-open.git",
    directory: "clients/js",
  });
});

test("npm pack would publish the built files, the README and the licence, and nothing else", () => {
  const output = execFileSync("npm", ["pack", "--dry-run", "--json", "--ignore-scripts"], {
    cwd: ROOT,
    encoding: "utf8",
  });
  const [packed] = JSON.parse(output) as [{ files: { path: string }[] }];
  const files = packed.files.map((file) => file.path).sort();
  const built = (directory: string, names: string[]) =>
    names.flatMap((name) => [`${directory}${name.replace(/\.js$/, ".d.ts")}`, `${directory}${name}`]);
  assert.deepEqual(
    files,
    [
      "LICENSE",
      "README.md",
      ...built("dist/", HOSTED),
      ...built("dist/offline/", OFFLINE),
      "dist/offline/wasm-inline.js",
      "dist/offline/wenmar_open.wasm",
      "package.json",
    ].sort(),
  );
});

test("the licence in the package is the repository's", () => {
  const packaged = readFileSync(new URL("LICENSE", ROOT), "utf8");
  const repository = readFileSync(new URL("../../LICENSE", ROOT), "utf8");
  assert.equal(packaged, repository);
});

test("the hosted client uses nothing but the web platform and loads nothing of the offline mode", () => {
  assert.deepEqual(
    readdirSync(new URL("dist/", ROOT)).filter((name) => name.endsWith(".js")).sort(),
    HOSTED,
  );
  for (const name of HOSTED) {
    const text = readFileSync(new URL(`dist/${name}`, ROOT), "utf8");
    assert.doesNotMatch(text, /from "node:|require\(|process\.|Buffer\b/, `dist/${name} uses a Node API`);
    assert.doesNotMatch(text, /WebAssembly|import\(/, `dist/${name} loads code at run time`);
    const imports = [...text.matchAll(/from "([^"]+)"/g)].map((match) => match[1]);
    for (const specifier of imports) {
      // A file beside it: never offline/, never another package.
      assert.match(String(specifier), /^\.\/[a-z]+\.js$/, `dist/${name} imports ${String(specifier)}`);
    }
  }
});

test("the offline mode has no way to reach a network", () => {
  assert.deepEqual(
    readdirSync(new URL("dist/offline/", ROOT)).filter((name) => name.endsWith(".js")).sort(),
    [...OFFLINE, "wasm-inline.js"].sort(),
  );
  for (const name of OFFLINE) {
    const text = readFileSync(new URL(`dist/offline/${name}`, ROOT), "utf8");
    assert.doesNotMatch(
      text,
      /\bfetch\b|XMLHttpRequest|WebSocket|EventSource|sendBeacon|importScripts|node:(http|https|http2|net|tls|dgram|dns)|https?:\/\//,
      `dist/offline/${name} names a way to reach a network`,
    );
    const imports = [...text.matchAll(/(?:from|import\() ?"([^"]+)"/g)].map((match) => String(match[1]));
    for (const specifier of imports) {
      assert.ok(
        /^\.\/[a-z-]+\.js$/.test(specifier) || ["../batch.js", "../errors.js"].includes(specifier),
        `dist/offline/${name} imports ${specifier}`,
      );
    }
    // Only the Node entry point may name a Node module, and only node:sqlite.
    const node = [...text.matchAll(/"(node:[a-z/_]+)"/g)].map((match) => match[1]);
    assert.deepEqual(node, name === "node.js" ? ["node:sqlite"] : [], `dist/offline/${name}`);
  }
});

test("the WebAssembly imports nothing, exports five things and is within its size", () => {
  const bytes = wasmBytes();
  assert.ok(bytes.length <= WASM_BUDGET, `the WebAssembly is ${bytes.length} bytes; the budget is ${WASM_BUDGET}`);
  const module = new WebAssembly.Module(bytes);
  // No import means no clock, no random numbers, no file and no network:
  // the module can call nothing outside itself.
  assert.deepEqual(WebAssembly.Module.imports(module), []);
  assert.deepEqual(
    WebAssembly.Module.exports(module).map((entry) => `${entry.kind} ${entry.name}`).sort(),
    ["function wo_alloc", "function wo_call", "function wo_result", "function wo_result_len", "memory memory"],
  );
});

test("the copy of the WebAssembly kept as text is the same bytes as the file", async () => {
  const inline = (await import(new URL("dist/offline/wasm-inline.js", ROOT).href)) as { WASM_BASE64: string };
  assert.ok(Buffer.from(inline.WASM_BASE64, "base64").equals(wasmBytes()));
});

test("the WebAssembly names no path of the machine that built it", () => {
  const text = wasmBytes().toString("latin1");
  assert.ok(!text.includes(homedir()), `the WebAssembly contains ${homedir()}`);
  assert.doesNotMatch(text, /\/(Users|home)\/[A-Za-z0-9_.-]+\//);
});
