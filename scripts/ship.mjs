#!/usr/bin/env node
// One command to put the newest data and the newest shop file that pairs
// with it live, then prove the service is serving them.
//
//   node scripts/ship.mjs [--dry-run] [--root DIR]
//
// It picks the newest shop release whose manifest names a data release that
// exists, downloads both files through the checks that already guard them,
// writes their versions into config/deploy.yml, commits and pushes that one
// file, runs the deploy checks and `kamal deploy`, then reads the live
// service back. When the newest data release has no shop release yet, it
// ships the newest complete pair and says so; it never ships a pair that
// disagrees.
//
// It deploys from a clean main. Uncommitted changes other than
// config/deploy.yml stop it, as do unpushed commits; a main that is only
// behind origin is fast-forwarded. Only config/deploy.yml is committed, as
// "deploy: data X, shop Y".
//
// Environment:
//   BWS_ACCESS_TOKEN  required by .kamal/secrets; checked before anything runs
//   GH_TOKEN          raises GitHub's API rate limit. Optional.
//   SHIP_RELEASES_DIR read releases.json and every manifest from this
//                     directory instead of the network. For the tests.
//   SHIP_BASE_URL     the address to verify. Defaults to OPEN_BASE_URL in
//                     config/deploy.yml, then https://open.wenmarpro.com.
import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const RELEASES = "https://api.github.com/repos/wenmar-pro/wenmar-open/releases?per_page=100";
/** Manifests read before giving up: two months of weekly releases. */
const MOST_MANIFESTS = 8;
const DATA_VERSION = /^[0-9]{4}\.[0-9]{2}(\.[0-9]+)?$/;
const SHOP_VERSION = /^[0-9]{4}\.[0-9]{2}\.[0-9]{2}(\.[1-9][0-9]{0,3})?$/;

const numbers = (version) => version.split(".").map(Number);

function newerFirst(left, right) {
  const [a, b] = [numbers(left.version), numbers(right.version)];
  for (let index = 0; index < 4; index += 1) {
    const difference = (b[index] ?? 0) - (a[index] ?? 0);
    if (difference !== 0) return difference;
  }
  return 0;
}

/** The data releases in a GitHub release list, newest first. */
function dataReleases(list) {
  return list
    .filter((release) => release?.draft !== true && release?.prerelease !== true)
    .map((release) => {
      const tag = typeof release?.tag_name === "string" ? release.tag_name : "";
      if (!tag.startsWith("data-")) return undefined;
      const version = tag.slice("data-".length);
      return DATA_VERSION.test(version) ? { ...release, version } : undefined;
    })
    .filter((release) => release !== undefined)
    .sort(newerFirst);
}

/** The shop releases in a GitHub release list that have a manifest, newest first. */
function shopReleases(list) {
  return list
    .filter((release) => release?.draft !== true && release?.prerelease !== true)
    .map((release) => {
      const tag = typeof release?.tag_name === "string" ? release.tag_name : "";
      if (!tag.startsWith("shop-")) return undefined;
      const version = tag.slice("shop-".length);
      if (!SHOP_VERSION.test(version)) return undefined;
      const name = `wenmar-open-shop-${version}.json`;
      const url = (release.assets ?? []).find((asset) => asset?.name === name)?.browser_download_url;
      return typeof url === "string" ? { ...release, version, manifestName: name, manifestUrl: url } : undefined;
    })
    .filter((release) => release !== undefined)
    .sort(newerFirst);
}

/**
 * The newest pair that can actually be shipped: a shop release whose manifest
 * names a data release that exists. `readManifest` takes a shop candidate and
 * returns its manifest, from the network or from a directory.
 */
export async function choosePair(releases, readManifest) {
  const data = dataReleases(releases);
  const shops = shopReleases(releases);
  if (data.length === 0) throw new Error("no data release has been published yet.");
  if (shops.length === 0) throw new Error("no shop data release has been published yet.");
  const newestDataVersion = data[0].version;
  const published = new Set(data.map((release) => release.version));

  for (const shop of shops.slice(0, MOST_MANIFESTS)) {
    let manifest;
    try {
      manifest = await readManifest(shop);
    } catch (error) {
      throw new Error(`the manifest of shop-${shop.version} could not be read: ${error.message}`);
    }
    if (manifest?.data_version !== shop.version) {
      throw new Error(`the manifest of shop-${shop.version} says it is ${manifest?.data_version}.`);
    }
    if (!published.has(manifest.catalog_data_version)) continue;
    return {
      dataVersion: manifest.catalog_data_version,
      shopVersion: shop.version,
      newestDataVersion,
      skippedDataVersion: newestDataVersion === manifest.catalog_data_version ? null : newestDataVersion,
    };
  }
  throw new Error("no shop release pairs with a published data release.");
}

/** The value of the one line `    NAME: "value"` or `    NAME: value` in a file. */
function deployLineValue(text, name) {
  const lines = text.split("\n").filter((line) => new RegExp(`^ *${name}: `).test(line));
  if (lines.length !== 1) throw new Error(`config/deploy.yml has ${lines.length} ${name} lines, not 1.`);
  return lines[0].replace(/^[^:]*: */, "").replace(/^"(.*)"$/, "$1").trim();
}

/** The text with the one `    NAME: "value"` line replaced. */
export function deployLineUpdate(text, name, value) {
  const lines = text.split("\n").filter((line) => new RegExp(`^ *${name}: `).test(line));
  if (lines.length !== 1) throw new Error(`config/deploy.yml has ${lines.length} ${name} lines, not 1.`);
  return text.replace(new RegExp(`^( *${name}: ).*$`, "m"), `$1"${value}"`);
}

/** The data and shop versions in a weak ETag, or null when it carries none. */
export function liveVersionsFromEtag(etag) {
  const match =
    /^(?:W\/)?"([0-9]{4}\.[0-9]{2}(?:\.[0-9]+)?)\+([0-9]{4}\.[0-9]{2}\.[0-9]{2}(?:\.[0-9]+)?)-/.exec(
      etag ?? "",
    );
  return match === null ? null : { dataVersion: match[1], shopVersion: match[2] };
}

/** The address to verify: the environment, then deploy.yml, then the public one. */
export function baseUrlFrom(env, text) {
  if (env.SHIP_BASE_URL) return env.SHIP_BASE_URL;
  try {
    return deployLineValue(text, "OPEN_BASE_URL");
  } catch {
    return "https://open.wenmarpro.com";
  }
}

/** Why the tree cannot be shipped from, or null when it can. */
export function preflightProblem({ branch, status, ahead, behind, hasKamal, hasToken }) {
  if (branch !== "main") return `this is on ${branch || "a detached HEAD"}, and ship deploys from main.`;
  const unrelated = status
    .split("\n")
    .filter((line) => line.trim() !== "")
    .filter((line) => line.slice(3).trim() !== "config/deploy.yml");
  if (unrelated.length > 0) {
    return `the working tree has uncommitted changes in ${unrelated[0].slice(3).trim() || "it"}; commit or stash them first.`;
  }
  if (ahead > 0 && behind > 0) return `main and origin/main have diverged (${ahead} ahead, ${behind} behind).`;
  if (ahead > 0) return `main has ${ahead} unpushed commit(s); push them first.`;
  if (!hasKamal) return "kamal is not on PATH.";
  if (!hasToken) return "BWS_ACCESS_TOKEN is not set; kamal's secrets read it from Bitwarden.";
  return null;
}

/**
 * Wait for the service to answer, then hold it to the pair that was shipped.
 * `fetchImpl` and `sleep` are seams for the tests.
 */
export async function verifyLive({
  baseUrl,
  dataVersion,
  shopVersion,
  fetchImpl = fetch,
  sleep = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds)),
  log = () => {},
}) {
  let health;
  for (let attempt = 0; attempt < 24; attempt += 1) {
    try {
      const response = await fetchImpl(`${baseUrl}/health`, { headers: { "cache-control": "no-cache" } });
      if (response.ok) {
        health = await response.json();
        break;
      }
    } catch {
      // The old container, or the proxy, may not answer while Kamal switches.
    }
    log("ship: waiting for the service to answer /health");
    await sleep(5000);
  }
  if (health === undefined) throw new Error(`the service at ${baseUrl} did not answer /health within two minutes.`);
  if (health.data_version !== dataVersion) {
    throw new Error(`the service says data ${health.data_version}, and ship shipped data ${dataVersion}.`);
  }

  const meta = await fetchImpl(`${baseUrl}/v1/meta`, { headers: { "cache-control": "no-cache" } });
  if (!meta.ok) throw new Error(`${baseUrl}/v1/meta answered HTTP ${meta.status}.`);
  const etag = meta.headers.get("etag");
  const live = liveVersionsFromEtag(etag);
  if (live === null) throw new Error(`${baseUrl}/v1/meta has an ETag that names no versions: ${etag}.`);
  if (live.dataVersion !== dataVersion || live.shopVersion !== shopVersion) {
    throw new Error(
      `the service is on data ${live.dataVersion} and shop ${live.shopVersion}, ` +
        `and ship shipped data ${dataVersion} and shop ${shopVersion}.`,
    );
  }
}

/** Run a command in the repository, inheriting its output unless captured. */
function run(root, command, args, { capture = false, allowFailure = false } = {}) {
  const done = spawnSync(command, args, {
    cwd: root,
    encoding: "utf8",
    stdio: capture ? ["ignore", "pipe", "pipe"] : "inherit",
  });
  if (done.error) throw new Error(`${command} could not be run: ${done.error.message}`);
  if (!allowFailure && done.status !== 0) throw new Error(`${command} ${args.join(" ")} exited ${done.status}.`);
  return done;
}

function output(root, command, args) {
  return run(root, command, args, { capture: true }).stdout;
}

function has(command) {
  return spawnSync("sh", ["-c", `command -v ${command}`], { stdio: "ignore" }).status === 0;
}

async function releaseList(env) {
  const local = env.SHIP_RELEASES_DIR;
  if (local !== undefined) return JSON.parse(readFileSync(join(local, "releases.json"), "utf8"));
  const headers = { "user-agent": "wenmar-open ship", accept: "application/vnd.github+json" };
  if (env.GH_TOKEN) headers.authorization = `Bearer ${env.GH_TOKEN}`;
  const response = await fetch(RELEASES, { headers });
  if (!response.ok) throw new Error(`${RELEASES} answered HTTP ${response.status}.`);
  return response.json();
}

function manifestReader(env) {
  const local = env.SHIP_RELEASES_DIR;
  return async (shop) => {
    if (local !== undefined) return JSON.parse(readFileSync(join(local, shop.manifestName), "utf8"));
    const headers = { "user-agent": "wenmar-open ship", accept: "*/*" };
    if (env.GH_TOKEN) headers.authorization = `Bearer ${env.GH_TOKEN}`;
    const response = await fetch(shop.manifestUrl, { headers });
    if (!response.ok) throw new Error(`${shop.manifestUrl} answered HTTP ${response.status}.`);
    return response.json();
  };
}

function preflight(root, env, { dryRun }) {
  const branch = output(root, "git", ["branch", "--show-current"]).trim();
  const status = output(root, "git", ["status", "--porcelain"]);
  run(root, "git", ["fetch", "--quiet", "origin"]);
  const counts = output(root, "git", ["rev-list", "--left-right", "--count", "HEAD...origin/main"])
    .trim()
    .split(/\s+/);
  const ahead = Number(counts[0] ?? 0);
  const behind = Number(counts[1] ?? 0);
  const problem = preflightProblem({
    branch,
    status,
    ahead,
    behind,
    hasKamal: has("kamal"),
    hasToken: Boolean(env.BWS_ACCESS_TOKEN),
  });
  if (problem !== null) throw new Error(problem);
  if (!dryRun && behind > 0) {
    console.log(`ship: fast-forwarding main over ${behind} commit(s) on origin`);
    run(root, "git", ["pull", "--ff-only", "--quiet"]);
  }
}

async function main() {
  const { values } = parseArgs({
    options: {
      "dry-run": { type: "boolean", default: false },
      root: { type: "string" },
      help: { type: "boolean", short: "h", default: false },
    },
  });
  if (values.help) {
    console.log("usage: node scripts/ship.mjs [--dry-run] [--root DIR]");
    console.log("");
    console.log("Downloads the newest complete data+shop pair, commits config/deploy.yml,");
    console.log("deploys it with Kamal, and verifies the live service. See docs/deploy.md.");
    return;
  }

  const root = values.root ?? fileURLToPath(new URL("../", import.meta.url));
  const env = process.env;
  const pair = await choosePair(await releaseList(env), manifestReader(env));

  if (pair.skippedDataVersion !== null) {
    console.log(
      `ship: newest data release is ${pair.skippedDataVersion}, and no shop release pairs with it yet; ` +
        "not shipping it.",
    );
    console.log('ship: the "Shop data release" workflow on main makes one; until then, ship uses the newest pair that exists.');
  }
  console.log(`ship: newest complete pair is data ${pair.dataVersion} with shop ${pair.shopVersion}.`);

  preflight(root, env, { dryRun: values["dry-run"] });

  const deployFile = join(root, "config/deploy.yml");
  if (values["dry-run"]) {
    console.log(`ship: would fetch data ${pair.dataVersion} and shop ${pair.shopVersion}, commit config/deploy.yml,`);
    console.log(`ship: run ${join(root, "scripts/deploy-check.sh")} and kamal deploy, then verify the live pair.`);
    return;
  }

  console.log(`ship: fetching data ${pair.dataVersion}`);
  run(root, join(root, "scripts/data-pull.sh"), [pair.dataVersion]);

  const before = readFileSync(deployFile, "utf8");
  writeFileSync(deployFile, deployLineUpdate(before, "DATA_VERSION", pair.dataVersion));

  console.log(`ship: fetching shop ${pair.shopVersion}, built against data ${pair.dataVersion}`);
  run(root, "node", [join(root, "scripts/shop-pull.mjs"), "--data-version", pair.dataVersion, "--root", root]);

  const written = readFileSync(deployFile, "utf8");
  const dataVersion = deployLineValue(written, "DATA_VERSION");
  const shopVersion = deployLineValue(written, "SHOP_VERSION");
  if (dataVersion !== pair.dataVersion || shopVersion !== pair.shopVersion) {
    throw new Error(
      `config/deploy.yml names data ${dataVersion} and shop ${shopVersion}, and ship chose data ` +
        `${pair.dataVersion} and shop ${pair.shopVersion}. Nothing was deployed.`,
    );
  }

  run(root, "git", ["add", "config/deploy.yml"]);
  const staged = run(root, "git", ["diff", "--cached", "--quiet"], { allowFailure: true });
  if (staged.status === 1) {
    run(root, "git", ["commit", "--quiet", "-m", `deploy: data ${dataVersion}, shop ${shopVersion}`]);
    run(root, "git", ["push", "--quiet", "origin", "main"]);
    console.log(`ship: pushed "deploy: data ${dataVersion}, shop ${shopVersion}" to main`);
  } else if (staged.status !== 0) {
    throw new Error(`git diff --cached --quiet exited ${staged.status}.`);
  } else {
    console.log("ship: config/deploy.yml already names this pair; nothing to commit.");
  }

  console.log("ship: checking the files the image will copy");
  run(root, join(root, "scripts/deploy-check.sh"));

  console.log("ship: deploying with kamal");
  run(root, "kamal", ["deploy"]);

  const baseUrl = baseUrlFrom(env, written);
  console.log(`ship: verifying ${baseUrl}`);
  await verifyLive({
    baseUrl: baseUrl.replace(/\/+$/, ""),
    dataVersion,
    shopVersion,
    log: (line) => console.log(line),
  });
  console.log(`ship: live on data ${dataVersion} and shop ${shopVersion}.`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    await main();
  } catch (error) {
    console.error(`ship: ${error.message}`);
    process.exit(1);
  }
}
