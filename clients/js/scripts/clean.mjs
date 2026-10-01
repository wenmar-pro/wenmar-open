// Removes the directories named on the command line, so a build never
// carries a file left over from an earlier one.
import { rmSync } from "node:fs";

for (const directory of process.argv.slice(2)) {
  rmSync(new URL(`../${directory}`, import.meta.url), { recursive: true, force: true });
}
