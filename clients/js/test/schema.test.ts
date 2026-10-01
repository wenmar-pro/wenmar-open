import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";

interface Generator {
  generate(): Promise<string>;
  SCHEMA: URL;
}

// scripts/generate.mjs, from .test-build/.
const generator = (await import(new URL("../scripts/generate.mjs", import.meta.url).href)) as Generator;

test("the committed types match the OpenAPI description", async () => {
  const committed = await readFile(generator.SCHEMA, "utf8");
  const wanted = await generator.generate();
  assert.ok(
    committed === wanted,
    "clients/js/src/schema.ts is out of date with crates/open-server/openapi.json. Run: npm run generate (in clients/js), and commit the file.",
  );
});
