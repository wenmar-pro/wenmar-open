// What is published: the files in the package, and what it depends on.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { test } from "node:test";

/** clients/js/, from .test-build/. */
const ROOT = new URL("../", import.meta.url);

const manifest = JSON.parse(readFileSync(new URL("package.json", ROOT), "utf8")) as Record<string, unknown>;

test("the package has no runtime dependencies and runs no script on install", () => {
  for (const field of ["dependencies", "peerDependencies", "optionalDependencies", "bundledDependencies"]) {
    assert.equal(manifest[field], undefined, `package.json has ${field}`);
  }
  const scripts = manifest["scripts"] as Record<string, string>;
  for (const hook of ["preinstall", "install", "postinstall"]) {
    assert.equal(scripts[hook], undefined, `package.json runs a ${hook} script`);
  }
});

test("the package is ESM with type declarations and names its repository", () => {
  assert.equal(manifest["name"], "wenmar-open");
  assert.equal(manifest["type"], "module");
  assert.equal(manifest["license"], "MIT");
  assert.deepEqual(manifest["exports"], {
    ".": { types: "./dist/index.d.ts", default: "./dist/index.js" },
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
  assert.deepEqual(files, [
    "LICENSE",
    "README.md",
    "dist/client.d.ts",
    "dist/client.js",
    "dist/errors.d.ts",
    "dist/errors.js",
    "dist/index.d.ts",
    "dist/index.js",
    "dist/schema.d.ts",
    "dist/schema.js",
    "dist/types.d.ts",
    "dist/types.js",
    "package.json",
  ]);
});

test("the licence in the package is the repository's", () => {
  const packaged = readFileSync(new URL("LICENSE", ROOT), "utf8");
  const repository = readFileSync(new URL("../../LICENSE", ROOT), "utf8");
  assert.equal(packaged, repository);
});

test("the built files use nothing but the web platform", () => {
  for (const name of ["client.js", "errors.js", "index.js", "schema.js", "types.js"]) {
    const text = readFileSync(new URL(`dist/${name}`, ROOT), "utf8");
    assert.doesNotMatch(text, /from "node:|require\(|process\.|Buffer\b/, `dist/${name} uses a Node API`);
    const imports = [...text.matchAll(/from "([^"]+)"/g)].map((match) => match[1]);
    for (const specifier of imports) {
      assert.match(String(specifier), /^\.\/[a-z]+\.js$/, `dist/${name} imports ${String(specifier)}`);
    }
  }
});
