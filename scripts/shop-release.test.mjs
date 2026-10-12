// The scripts of a shop data release: the manifest, the check that no
// dataset went missing, and the fetch of the release that pairs with a data
// file. No network: shop-pull reads a directory instead.
//
//   node --test scripts/shop-release.test.mjs
//
// The manifest needs node:sqlite, Node 22.16 or later. On an older Node the
// tests that build a shop file are skipped.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

const sqlite = await import("node:sqlite").catch(() => undefined);
const skip = sqlite === undefined ? "needs node:sqlite of Node 22.16 or later" : false;

const scripts = fileURLToPath(new URL("./", import.meta.url));
const directory = realpathSync(mkdtempSync(join(tmpdir(), "wenmar-open-shop-release-")));
after(() => rmSync(directory, { recursive: true, force: true }));

const US = { as_of: "2026-10-11", rows: 7, licence: "public-domain", attribution: "" };
const CA = { as_of: "2026-10-06", rows: 2, licence: "ogl-canada", attribution: "Contains information licensed under the Open Government Licence – Canada." };

function run(script, args, env = {}) {
  const done = spawnSync("node", [join(scripts, script), ...args], { encoding: "utf8", env: { ...process.env, ...env } });
  return { code: done.status, out: done.stdout, err: done.stderr };
}

/** A shop file with the given meta rows. */
function shopFile(name, meta) {
  const path = join(directory, name);
  const database = new sqlite.DatabaseSync(path);
  database.exec("CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);");
  const insert = database.prepare("INSERT INTO meta (key, value) VALUES (?, ?)");
  for (const [key, value] of Object.entries(meta)) insert.run(key, value);
  database.close();
  return path;
}

const META = {
  schema_version: "1",
  data_version: "2026.10.12",
  catalog_data_version: "2026.09",
  built_at: "2026-10-12 06:00:00",
  datasets: JSON.stringify({ recalls_us: US, recalls_ca: CA }),
};

function manifestFile(name, manifest) {
  const path = join(directory, name);
  writeFileSync(path, JSON.stringify(manifest));
  return path;
}

test("the manifest is read from the file's own meta table", { skip }, () => {
  const shop = shopFile("named-anything.sqlite3", META);
  const packed = join(directory, "named-anything.sqlite3.gz");
  writeFileSync(packed, gzipSync(readFileSync(shop)));
  const { code, out, err } = run("shop-manifest.mjs", [shop, packed]);
  assert.equal(code, 0, err);
  assert.deepEqual(JSON.parse(out), {
    data_version: "2026.10.12",
    catalog_data_version: "2026.09",
    schema_version: "1",
    built_at: "2026-10-12 06:00:00",
    bytes: readFileSync(shop).length,
    sha256: createHash("sha256").update(readFileSync(packed)).digest("hex"),
    datasets: { recalls_us: US, recalls_ca: CA },
  });
  // The keys come in this order, and the text ends with one newline.
  assert.deepEqual(Object.keys(JSON.parse(out)), [
    "data_version",
    "catalog_data_version",
    "schema_version",
    "built_at",
    "bytes",
    "sha256",
    "datasets",
  ]);
  assert.ok(out.endsWith("}\n"));
});

test("the manifest refuses what is not a shop file", { skip }, () => {
  const packed = join(directory, "any.gz");
  writeFileSync(packed, gzipSync("x"));
  const cases = [
    [shopFile("no-catalog.sqlite3", { ...META, catalog_data_version: "" }), "its meta table has no catalog_data_version"],
    [shopFile("monthly.sqlite3", { ...META, data_version: "2026.10" }), "which is not like 2026.10.12"],
    [shopFile("bad-datasets.sqlite3", { ...META, datasets: "[]" }), "datasets value that is not an object"],
    [shopFile("not-json.sqlite3", { ...META, datasets: "{" }), "datasets value that is not JSON"],
  ];
  for (const [shop, message] of cases) {
    const { code, out, err } = run("shop-manifest.mjs", [shop, packed]);
    assert.equal(code, 1, shop);
    assert.equal(out, "", shop);
    assert.ok(err.includes(message), err);
  }
  const text = join(directory, "text.sqlite3");
  writeFileSync(text, "this is not a database");
  assert.equal(run("shop-manifest.mjs", [text, packed]).code, 1);
  assert.equal(run("shop-manifest.mjs", [cases[0][0], join(directory, "absent.gz")]).code, 1);
  assert.equal(run("shop-manifest.mjs", []).code, 2);
});

test("a release that holds everything the one before held is accepted", () => {
  const before = manifestFile("before.json", { data_version: "2026.10.05", datasets: { recalls_us: US, recalls_ca: CA } });
  const grown = manifestFile("grown.json", {
    data_version: "2026.10.12",
    datasets: { recalls_us: { ...US, rows: 9 }, recalls_ca: CA, bulletins: { ...US, rows: 4 } },
  });
  const { code, out, err } = run("shop-datasets-check.mjs", [grown, before]);
  assert.equal(code, 0, err);
  assert.ok(out.includes("recalls_us: 9 rows, as of 2026-10-11 (was 7)"), out);
  assert.ok(out.includes("bulletins: 4 rows, as of 2026-10-11 (new)"), out);
  assert.ok(out.includes("2026.10.12 holds everything 2026.10.05 held."), out);
  // The first release has nothing to be compared with.
  const first = run("shop-datasets-check.mjs", [grown]);
  assert.equal(first.code, 0, first.err);
  assert.ok(first.out.includes("holds everything none held."), first.out);
});

test("a release that lost a dataset, emptied one or halved one is refused, by name", () => {
  const before = manifestFile("had.json", { data_version: "2026.10.05", datasets: { recalls_us: US, recalls_ca: CA } });
  const cases = [
    [{ recalls_us: US }, ["recalls_ca was in 2026.10.05 and is missing from 2026.10.12"]],
    [{}, ["recalls_ca was in", "recalls_us was in"]],
    [{ recalls_us: US, recalls_ca: { ...CA, rows: 0 } }, ["recalls_ca has no rows in 2026.10.12"]],
    [{ recalls_us: { ...US, rows: 3 }, recalls_ca: CA }, ["recalls_us has 3 rows in 2026.10.12 and had 7 in 2026.10.05: fewer than half"]],
  ];
  cases.forEach(([datasets, messages], index) => {
    const lost = manifestFile(`lost-${index}.json`, { data_version: "2026.10.12", datasets });
    const { code, err } = run("shop-datasets-check.mjs", [lost, before]);
    assert.equal(code, 1, err);
    for (const message of messages) assert.ok(err.includes(message), err);
    assert.ok(err.includes("not released"), err);
  });
  // A new dataset with no rows is refused even on the first release.
  const empty = manifestFile("empty.json", { data_version: "2026.10.12", datasets: { recalls_us: { ...US, rows: 0 } } });
  assert.equal(run("shop-datasets-check.mjs", [empty]).code, 1);
  // Exactly half is not fewer than half.
  const half = manifestFile("half.json", { data_version: "2026.10.12", datasets: { recalls_us: { ...US, rows: 4 }, recalls_ca: { ...CA, rows: 1 } } });
  assert.equal(run("shop-datasets-check.mjs", [half, before]).code, 0);
});

test("what is not a manifest is a usage error, not a refusal", () => {
  const good = manifestFile("good.json", { data_version: "2026.10.12", datasets: { recalls_us: US } });
  const list = manifestFile("list.json", []);
  const text = join(directory, "text.json");
  writeFileSync(text, "not json");
  assert.equal(run("shop-datasets-check.mjs", []).code, 2);
  assert.equal(run("shop-datasets-check.mjs", [list]).code, 2);
  assert.equal(run("shop-datasets-check.mjs", [good, text]).code, 2);
  assert.equal(run("shop-datasets-check.mjs", [join(directory, "absent.json")]).code, 2);
});

// ----- shop-pull -----

const DEPLOY = `service: wenmar-open
env:
  clear:
    OPEN_DATA: /app/data/wenmar-open.sqlite3
    OPEN_SHOP_DATA: /app/data/wenmar-open-shop.sqlite3
builder:
  args:
    # The data release baked into the image.
    DATA_VERSION: "2026.09"
    # The shop data release baked into the image.
    SHOP_VERSION: "2026.10.05"
`;

/** A checkout with a deploy file, and a directory standing in for GitHub. */
function world(name, releases) {
  const root = join(directory, name);
  const served = join(root, "served");
  mkdirSync(join(root, "config"), { recursive: true });
  mkdirSync(served, { recursive: true });
  writeFileSync(join(root, "config/deploy.yml"), DEPLOY);
  const list = [];
  for (const { version, catalog, draft = false, files = ["gz", "json", "sha256"], corrupt = false } of releases) {
    const base = `wenmar-open-shop-${version}`;
    const unpacked = Buffer.from(`shop file ${version} for ${catalog}`);
    const packed = gzipSync(unpacked);
    const manifest = {
      data_version: version,
      catalog_data_version: catalog,
      schema_version: "1",
      built_at: "2026-10-12 06:00:00",
      bytes: unpacked.length,
      sha256: createHash("sha256").update(packed).digest("hex"),
      datasets: { recalls_us: US },
    };
    writeFileSync(join(served, `${base}.sqlite3.gz`), corrupt ? gzipSync("something else") : packed);
    writeFileSync(join(served, `${base}.json`), JSON.stringify(manifest));
    const names = { gz: `${base}.sqlite3.gz`, json: `${base}.json`, sha256: `${base}.sqlite3.gz.sha256` };
    list.push({
      tag_name: `shop-${version}`,
      draft,
      prerelease: false,
      assets: files.map((kind) => ({ name: names[kind], browser_download_url: `https://example.invalid/${names[kind]}` })),
    });
  }
  // Releases of other kinds are in the same list.
  list.push({ tag_name: "data-2026.09", draft: false, prerelease: false, assets: [] });
  list.push({ tag_name: "v0.2.0", draft: false, prerelease: false, assets: [] });
  writeFileSync(join(served, "releases.json"), JSON.stringify(list));
  const pull = (...args) => run("shop-pull.mjs", ["--root", root, ...args], { SHOP_PULL_DIR: served });
  const shopVersion = () => /SHOP_VERSION: "(.*)"/.exec(readFileSync(join(root, "config/deploy.yml"), "utf8"))[1];
  return { root, pull, shopVersion };
}

test("shop-pull takes the newest release that pairs with the data version of the deploy file", () => {
  const { root, pull, shopVersion } = world("pairs", [
    { version: "2026.10.05", catalog: "2026.09" },
    { version: "2026.10.12", catalog: "2026.09" },
    { version: "2026.10.12.1", catalog: "2026.10" },
    { version: "2026.10.19", catalog: "2026.10", draft: true },
    { version: "2026.10.26", catalog: "2026.09", files: ["gz", "sha256"] },
  ]);
  const { code, out, err } = pull();
  assert.equal(code, 0, err);
  const path = join(root, "data/build/shop/wenmar-open-shop-2026.10.12.sqlite3");
  assert.equal(readFileSync(path, "utf8"), "shop file 2026.10.12 for 2026.09");
  assert.ok(!existsSync(`${path}.part`));
  assert.equal(shopVersion(), "2026.10.12");
  assert.ok(out.includes("shop-2026.10.12.1 pairs with data 2026.10"), out);
  assert.ok(out.includes("SHOP_VERSION 2026.10.05 -> 2026.10.12"), out);
  // Nothing else in the deploy file moved.
  assert.equal(readFileSync(join(root, "config/deploy.yml"), "utf8"), DEPLOY.replace('"2026.10.05"', '"2026.10.12"'));
  // A second run downloads nothing and changes nothing.
  const again = pull();
  assert.equal(again.code, 0, again.err);
  assert.ok(again.out.includes("is already here"), again.out);
  assert.ok(again.out.includes("already names SHOP_VERSION 2026.10.12"), again.out);
  // Another data version is paired on request.
  const other = pull("--data-version", "2026.10");
  assert.equal(other.code, 0, other.err);
  assert.equal(shopVersion(), "2026.10.12.1");
});

test("shop-pull changes nothing when no release pairs, or when the download is not the release's file", () => {
  const none = world("none", [{ version: "2026.10.12", catalog: "2026.08" }]);
  const refused = none.pull();
  assert.equal(refused.code, 1);
  assert.ok(refused.err.includes("pairs with data 2026.09"), refused.err);
  assert.ok(refused.err.includes("Shop data release"), refused.err);
  assert.equal(none.shopVersion(), "2026.10.05");
  assert.ok(!existsSync(join(none.root, "data/build/shop")));

  const empty = world("empty", []);
  const nothing = empty.pull();
  assert.equal(nothing.code, 1);
  assert.ok(nothing.err.includes("no shop data release has been published yet"), nothing.err);

  const bad = world("bad", [{ version: "2026.10.12", catalog: "2026.09", corrupt: true }]);
  const wrong = bad.pull();
  assert.equal(wrong.code, 1);
  assert.ok(wrong.err.includes("its manifest says"), wrong.err);
  assert.equal(bad.shopVersion(), "2026.10.05");
  assert.ok(!existsSync(join(bad.root, "data/build/shop/wenmar-open-shop-2026.10.12.sqlite3")));

  assert.equal(none.pull("--data-version", "soon").code, 1);
});
