#!/usr/bin/env node
// Fetches the newest shop data release that pairs with the data file the
// server image will be built around, and names it in config/deploy.yml.
//
//   node scripts/shop-pull.mjs [--data-version YYYY.MM] [--root DIR]
//
// The data version is the DATA_VERSION line of config/deploy.yml unless
// --data-version gives one. The shop file is put in data/build/shop/ under
// the name the Dockerfile copies, and the SHOP_VERSION line of
// config/deploy.yml is set to its version. Nothing is committed or deployed.
//
// A shop file holds the ids of one data file's catalog, so only a release
// whose manifest names that data version will do: the server refuses to
// start over any other pair.
//
// It needs no credential: the releases are public. Set GH_TOKEN to raise
// GitHub's rate limit.
//
// Environment:
//   SHOP_PULL_DIR   read releases.json and every release file from this
//                   directory instead of the network. For the tests.
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, renameSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { gunzipSync } from "node:zlib";

const RELEASES = "https://api.github.com/repos/wenmar-pro/wenmar-open/releases?per_page=100";
/** Manifests read before giving up: two months of weekly releases. */
const MOST_MANIFESTS = 8;
const VERSION = /^[0-9]{4}\.[0-9]{2}\.[0-9]{2}(\.[1-9][0-9]{0,3})?$/;

function fail(message) {
  console.error(`shop-pull: ${message}`);
  process.exit(1);
}

const { values } = parseArgs({
  options: {
    "data-version": { type: "string" },
    root: { type: "string" },
    help: { type: "boolean", short: "h", default: false },
  },
});
if (values.help) {
  console.log("usage: node scripts/shop-pull.mjs [--data-version YYYY.MM] [--root DIR]");
  process.exit(0);
}
const root = values.root ?? fileURLToPath(new URL("../", import.meta.url));
const deployFile = join(root, "config/deploy.yml");
const local = process.env.SHOP_PULL_DIR;

/** The value of the one line `    NAME: "value"` in config/deploy.yml. */
function deployLine(name) {
  const lines = readFileSync(deployFile, "utf8")
    .split("\n")
    .filter((line) => new RegExp(`^ *${name}: `).test(line));
  if (lines.length !== 1) fail(`config/deploy.yml has ${lines.length} ${name} lines, not 1.`);
  return lines[0].replace(/^[^:]*: */, "").replace(/^"(.*)"$/, "$1");
}

const dataVersion = values["data-version"] ?? deployLine("DATA_VERSION");
if (!/^[0-9]{4}\.[0-9]{2}(\.[0-9]+)?$/.test(dataVersion)) {
  fail(`${dataVersion} is not a data version like 2026.09 or 2026.09.1.`);
}

/** The bytes of a release file, or of the release list when `name` is undefined. */
async function get(url, name) {
  if (local !== undefined) return readFileSync(join(local, name ?? "releases.json"));
  const headers = { "user-agent": "wenmar-open shop-pull", accept: name === undefined ? "application/vnd.github+json" : "*/*" };
  if (process.env.GH_TOKEN) headers.authorization = `Bearer ${process.env.GH_TOKEN}`;
  const response = await fetch(url, { headers });
  if (!response.ok) fail(`${url} answered HTTP ${response.status}.`);
  return Buffer.from(await response.arrayBuffer());
}

const numbers = (version) => version.split(".").map(Number);
function newerFirst(left, right) {
  const [a, b] = [numbers(left.version), numbers(right.version)];
  for (let index = 0; index < 4; index += 1) {
    const difference = (b[index] ?? 0) - (a[index] ?? 0);
    if (difference !== 0) return difference;
  }
  return 0;
}

let list;
try {
  list = JSON.parse((await get(RELEASES)).toString("utf8"));
} catch (error) {
  fail(`the release list could not be read: ${error.message}`);
}
if (!Array.isArray(list)) fail("the release list is not a list.");

const candidates = [];
for (const release of list) {
  const tag = typeof release?.tag_name === "string" ? release.tag_name : "";
  if (release?.draft || release?.prerelease || !tag.startsWith("shop-")) continue;
  const version = tag.slice("shop-".length);
  if (!VERSION.test(version)) continue;
  const base = `wenmar-open-shop-${version}`;
  const asset = (name) => (release.assets ?? []).find((found) => found?.name === name)?.browser_download_url;
  const [packed, manifest] = [asset(`${base}.sqlite3.gz`), asset(`${base}.json`)];
  if (typeof packed !== "string" || typeof manifest !== "string") continue;
  candidates.push({ version, base, packed, manifest });
}
candidates.sort(newerFirst);
if (candidates.length === 0) fail("no shop data release has been published yet.");

let chosen;
for (const candidate of candidates.slice(0, MOST_MANIFESTS)) {
  let manifest;
  try {
    manifest = JSON.parse((await get(candidate.manifest, `${candidate.base}.json`)).toString("utf8"));
  } catch (error) {
    fail(`the manifest of shop-${candidate.version} could not be read: ${error.message}`);
  }
  if (manifest.data_version !== candidate.version) {
    fail(`the manifest of shop-${candidate.version} says it is ${manifest.data_version}.`);
  }
  console.log(`shop-${candidate.version} pairs with data ${manifest.catalog_data_version}`);
  if (manifest.catalog_data_version === dataVersion) {
    chosen = { ...candidate, manifest };
    break;
  }
}
if (chosen === undefined) {
  fail(
    `none of the newest ${Math.min(candidates.length, MOST_MANIFESTS)} shop releases pairs with data ${dataVersion}. ` +
      'Run the "Shop data release" workflow from main, which builds one against the newest data release, then run this again.',
  );
}

const directory = join(root, "data/build/shop");
const path = join(directory, `${chosen.base}.sqlite3`);
if (existsSync(path) && statSync(path).size === chosen.manifest.bytes) {
  console.log(`${path} is already here.`);
} else {
  const packed = await get(chosen.packed, `${chosen.base}.sqlite3.gz`);
  const sha256 = createHash("sha256").update(packed).digest("hex");
  if (sha256 !== chosen.manifest.sha256) {
    fail(`the download of shop-${chosen.version} has SHA-256 ${sha256}; its manifest says ${chosen.manifest.sha256}. Nothing was changed.`);
  }
  let unpacked;
  try {
    unpacked = gunzipSync(packed);
  } catch (error) {
    fail(`the download of shop-${chosen.version} could not be unpacked: ${error.message}. Nothing was changed.`);
  }
  if (unpacked.length !== chosen.manifest.bytes) {
    fail(`the unpacked file is ${unpacked.length} bytes; its manifest says ${chosen.manifest.bytes}. Nothing was changed.`);
  }
  mkdirSync(directory, { recursive: true });
  writeFileSync(`${path}.part`, unpacked);
  renameSync(`${path}.part`, path);
  console.log(`${path}: ${unpacked.length} bytes`);
}

const before = deployLine("SHOP_VERSION");
if (before === chosen.version) {
  console.log(`config/deploy.yml already names SHOP_VERSION ${chosen.version}.`);
} else {
  const text = readFileSync(deployFile, "utf8").replace(/^( *SHOP_VERSION: ).*$/m, `$1"${chosen.version}"`);
  writeFileSync(deployFile, text);
  console.log(`config/deploy.yml: SHOP_VERSION ${before} -> ${chosen.version}`);
}
console.log(`shop-pull: shop data ${chosen.version} for data ${dataVersion} is in place. Commit config/deploy.yml, then deploy.`);
