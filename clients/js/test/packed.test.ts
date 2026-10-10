// What a consumer installs: the packed tarballs, not the working tree.
// The two tests that install both packages are skipped where node:sqlite
// cannot return rows as arrays (before Node 22.16) and where the data
// package has not been prepared: clients/data/build is gitignored, so a
// fresh checkout has none, and the skip reason names the command that
// builds it. The pack test needs neither and always runs.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";

const KONA = "KM8K2CAB4PU001140";

const sqlite: typeof import("node:sqlite") | undefined = await import("node:sqlite").then(
  (module) => (typeof module.StatementSync.prototype.setReturnArrays === "function" ? module : undefined),
  () => undefined,
);
const skipSqlite = sqlite === undefined ? "node:sqlite with setReturnArrays needs Node 22.16 or later" : false;

// The data package is prepared, never checked in: only a tree that has
// run prepare.mjs holds anything to pack as wenmar-open-data.
const dataPrepared = existsSync(new URL("../../data/build/package.json", import.meta.url));
const skipData = dataPrepared
  ? false
  : "no prepared data package (clients/data/build): run node clients/data/scripts/prepare.mjs data/build/wenmar-open-2026.09.sqlite3";

const skip = skipData || skipSqlite;

/** Packs a package directory into `destination`, shared with no other run. */
function pack(directory: URL, destination: string): string {
  const output = execFileSync("npm", ["pack", "--pack-destination", destination, "--json", "--ignore-scripts"], {
    cwd: directory,
    encoding: "utf8",
  });
  const [packed] = JSON.parse(output) as [{ filename: string }];
  return join(destination, packed.filename);
}

test("a clean install of the packed tarballs decodes a VIN offline", { skip }, async () => {
  const home = mkdtempSync(join(tmpdir(), "wenmar-open-packed-"));
  after(() => rmSync(home, { recursive: true, force: true }));
  const client = pack(new URL("../", import.meta.url), home);
  const data = pack(new URL("../../data/build/", import.meta.url), home);
  execFileSync("npm", ["init", "-y"], { cwd: home });
  execFileSync("npm", ["install", "--no-audit", "--no-fund", client, data], { cwd: home, stdio: "ignore" });

  // Import through the package name, so Node resolves the exports map
  // exactly as it would for a consumer. The entry point is
  // openOffline() from wenmar-open/offline/node, which finds the data file
  // by itself; passing { path } explicitly is also accepted.
  const script = `
    import { openOffline } from "wenmar-open/offline/node";
    import { path, dataVersion, schemaVersion } from "wenmar-open-data";
    const db = await openOffline({ path });
    const decode = await db.decodeVin(${JSON.stringify(KONA)});
    console.log(JSON.stringify({ dataVersion, schemaVersion, make: decode.make, model: decode.model }));
  `;
  const output = execFileSync("node", ["--input-type=module", "-e", script], {
    cwd: home,
    encoding: "utf8",
  });
  const line = output.trim().split("\n").pop();
  assert.ok(line, "the offline decode produced no output");
  const result = JSON.parse(line) as {
    dataVersion: string;
    schemaVersion: string;
    make?: string;
    model?: string;
  };

  // The data package must have landed inside the client's peer range, or
  // the install resolved to a placeholder with no SQLite file in it. A
  // data package's npm version is <schema>.<year><month>.<rebuild>, so a
  // peer of ^<n>.0.0 admits it exactly when n is the schemaVersion it
  // reports — the same rule package.test.ts holds the range to.
  const installed = JSON.parse(
    readFileSync(join(home, "node_modules", "wenmar-open", "package.json"), "utf8"),
  ) as { peerDependencies?: { "wenmar-open-data"?: string } };
  assert.equal(
    installed.peerDependencies?.["wenmar-open-data"],
    `^${result.schemaVersion}.0.0`,
    `wenmar-open-data is schema ${result.schemaVersion}, which the installed wenmar-open does not read`,
  );

  // Deliberately pinned to the prepared file's own data version: this
  // pins the content of the packed file, so it is hand-edited at each
  // monthly data release, next to the VIN above.
  assert.equal(result.dataVersion, "2026.09");
  assert.equal(result.make, "Hyundai");
  assert.equal(result.model, "Kona");
});

test("openOffline finds the data file with no path given", { skip }, async () => {
  const home = mkdtempSync(join(tmpdir(), "wenmar-open-default-"));
  after(() => rmSync(home, { recursive: true, force: true }));
  execFileSync("npm", ["init", "-y"], { cwd: home });
  execFileSync(
    "npm",
    [
      "install",
      "--no-audit",
      "--no-fund",
      pack(new URL("../", import.meta.url), home),
      pack(new URL("../../data/build/", import.meta.url), home),
    ],
    { cwd: home, stdio: "ignore" },
  );
  const output = execFileSync(
    "node",
    [
      "--input-type=module",
      "-e",
      `import { openOffline } from "wenmar-open/offline/node";
       const db = await openOffline();
       const d = await db.decodeVin(${JSON.stringify(KONA)});
       console.log(d.model);`,
    ],
    { cwd: home, encoding: "utf8" },
  );
  // This is the README's own example: install both packages, import, decode.
  assert.equal(output.trim(), "Kona");
});

test("the packed client carries the offline mode and its WebAssembly", () => {
  const output = execFileSync("npm", ["pack", "--dry-run", "--json", "--ignore-scripts"], {
    cwd: new URL("../", import.meta.url),
    encoding: "utf8",
  });
  const [packed] = JSON.parse(output) as [{ files: { path: string }[] }];
  const files = packed.files.map((file) => file.path);
  for (const needed of [
    "dist/offline/index.js",
    "dist/offline/node.js",
    "dist/offline/wenmar_open.wasm",
  ]) {
    assert.ok(files.includes(needed), `the tarball has no ${needed}`);
  }
});
