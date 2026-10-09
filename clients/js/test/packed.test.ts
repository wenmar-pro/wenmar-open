// What a consumer installs: the packed tarballs, not the working tree.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const KONA = "KM8K2CAB4PU001140";

/** Packs a package directory and returns the tarball path. */
function pack(directory: string): string {
  const output = execFileSync("npm", ["pack", "--pack-destination", tmpdir(), "--json", "--ignore-scripts"], {
    cwd: new URL(directory, import.meta.url).pathname,
    encoding: "utf8",
  });
  const [packed] = JSON.parse(output) as [{ filename: string }];
  return join(tmpdir(), packed.filename);
}

test("a clean install of the packed tarballs decodes a VIN offline", async () => {
  const home = mkdtempSync(join(tmpdir(), "wenmar-open-packed-"));
  const client = pack("../");
  const data = pack("../../data/build");
  execFileSync("npm", ["init", "-y"], { cwd: home });
  execFileSync("npm", ["install", "--no-audit", "--no-fund", client, data], { cwd: home, stdio: "inherit" });

  // The data package must have landed inside the peer range, or the install
  // resolved to a placeholder with no SQLite file in it.
  const installed = JSON.parse(
    readFileSync(join(home, "node_modules", "wenmar-open-data", "package.json"), "utf8"),
  ) as { version: string };
  assert.match(installed.version, /^\d+\.\d+\.\d+$/, `wenmar-open-data is ${installed.version}`);

  // Import through the package name, so Node resolves the exports map
  // exactly as it would for a consumer. The entry point is
  // openOffline() from wenmar-open/offline/node, which finds the data file
  // by itself; passing { path } explicitly is also accepted.
  const script = `
    import { openOffline } from "wenmar-open/offline/node";
    import { path, dataVersion } from "wenmar-open-data";
    const db = await openOffline({ path });
    const decode = await db.decodeVin(${JSON.stringify(KONA)});
    console.log(JSON.stringify({ dataVersion, make: decode.make, model: decode.model }));
  `;
  const output = execFileSync("node", ["--input-type=module", "-e", script], {
    cwd: home,
    encoding: "utf8",
  });
  const result = JSON.parse(output.trim().split("\n").pop()!) as {
    dataVersion: string;
    make?: string;
    model?: string;
  };
  assert.equal(result.dataVersion, "2026.09");
  assert.equal(result.make, "Hyundai");
  assert.equal(result.model, "Kona");
});

test("openOffline finds the data file with no path given", async () => {
  const home = mkdtempSync(join(tmpdir(), "wenmar-open-default-"));
  execFileSync("npm", ["init", "-y"], { cwd: home });
  execFileSync("npm", ["install", "--no-audit", "--no-fund", pack("../"), pack("../../data/build")], {
    cwd: home,
    stdio: "ignore",
  });
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
    cwd: new URL("../", import.meta.url).pathname,
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
