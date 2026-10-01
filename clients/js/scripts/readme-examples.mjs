// Copies every ```ts block of README.md into test/types/readme/, one file
// each, so `npm run test:types` compiles the examples a reader will paste.
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";

const readme = readFileSync(new URL("../README.md", import.meta.url), "utf8");
const directory = new URL("../test/types/readme/", import.meta.url);
rmSync(directory, { recursive: true, force: true });
mkdirSync(directory, { recursive: true });

const blocks = [...readme.matchAll(/^```ts\n([\s\S]*?)^```$/gm)].map((match) => match[1]);
if (blocks.length === 0) {
  console.error("README.md has no ```ts examples.");
  process.exit(1);
}
blocks.forEach((block, index) => {
  writeFileSync(new URL(`example-${index + 1}.ts`, directory), block);
});
console.log(`${blocks.length} README examples written to test/types/readme/`);
