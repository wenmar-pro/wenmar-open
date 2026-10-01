// Writes src/schema.ts from the OpenAPI description the server commits.
//
//   node scripts/generate.mjs           write the file
//   node scripts/generate.mjs --check   exit 1 if the file is out of date
//
// The same function is used by the test that fails when the committed file
// no longer matches the description.
import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import openapiTS, { astToString } from "openapi-typescript";

export const DESCRIPTION = new URL("../../../crates/open-server/openapi.json", import.meta.url);
export const SCHEMA = new URL("../src/schema.ts", import.meta.url);

const HEADER = `// Generated from crates/open-server/openapi.json by scripts/generate.mjs.
// Do not edit. Run: npm run generate

`;

/** The text src/schema.ts should have for the description as it is now. */
export async function generate() {
  const description = JSON.parse(await readFile(DESCRIPTION, "utf8"));
  // An object the description gives no properties, such as an error's
  // `details`, may hold anything: type it as unknown values, not as empty.
  const ast = await openapiTS(description, { emptyObjectsUnknown: true });
  return HEADER + astToString(ast);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const wanted = await generate();
  if (process.argv.includes("--check")) {
    const committed = await readFile(SCHEMA, "utf8").catch(() => "");
    if (committed !== wanted) {
      console.error("src/schema.ts is out of date. Run: npm run generate");
      process.exit(1);
    }
  } else {
    await writeFile(SCHEMA, wanted);
  }
}
