// The one-command deploy's decisions, without deploying anything: which
// data and shop release to ship, the two lines it writes into
// config/deploy.yml, what it refuses in git, and the live versions it reads
// back from the service.
//
//   node --test scripts/ship.test.mjs
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { once } from "node:events";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import {
  baseUrlFrom,
  choosePair,
  deployLineUpdate,
  liveVersionsFromEtag,
  preflightProblem,
  verifyLive,
} from "./ship.mjs";

/** A GitHub release list, reduced to what ship reads. */
function release(tag, manifest = true) {
  if (!tag.startsWith("shop-")) return { draft: false, prerelease: false, tag_name: tag, assets: [] };
  const version = tag.slice("shop-".length);
  return {
    draft: false,
    prerelease: false,
    tag_name: tag,
    assets: manifest
      ? [{ name: `wenmar-open-shop-${version}.json`, browser_download_url: `https://example.invalid/${version}.json` }]
      : [],
  };
}

/** Manifests by shop version, as the release's manifest file holds them. */
function manifests(byShop) {
  return (shop) => {
    const manifest = byShop[shop.version];
    assert.ok(manifest, `no fixture manifest for shop ${shop.version}`);
    return manifest;
  };
}

test("chooses the newest shop release whose data release exists", async () => {
  const releases = [
    release("data-2026.10"),
    release("data-2026.09"),
    release("shop-2026.11.09"),
    release("shop-2026.11.02"),
  ];
  const pair = await choosePair(
    releases,
    manifests({
      "2026.11.09": { data_version: "2026.11.09", catalog_data_version: "2026.09" },
      "2026.11.02": { data_version: "2026.11.02", catalog_data_version: "2026.09" },
    }),
  );
  assert.deepEqual(pair, {
    dataVersion: "2026.09",
    shopVersion: "2026.11.09",
    newestDataVersion: "2026.10",
    skippedDataVersion: "2026.10",
  });
});

test("uses the newest pair when the data and the shop both advance", async () => {
  const releases = [
    release("data-2026.11"),
    release("data-2026.10"),
    release("shop-2026.12.07"),
    release("shop-2026.11.30"),
  ];
  const pair = await choosePair(
    releases,
    manifests({
      "2026.12.07": { data_version: "2026.12.07", catalog_data_version: "2026.11" },
      "2026.11.30": { data_version: "2026.11.30", catalog_data_version: "2026.10" },
    }),
  );
  assert.deepEqual(pair, {
    dataVersion: "2026.11",
    shopVersion: "2026.12.07",
    newestDataVersion: "2026.11",
    skippedDataVersion: null,
  });
});

test("skips a shop release whose paired data release is gone", async () => {
  const releases = [
    release("data-2026.09"),
    release("shop-2026.11.09"),
    release("shop-2026.11.02"),
  ];
  const pair = await choosePair(
    releases,
    manifests({
      "2026.11.09": { data_version: "2026.11.09", catalog_data_version: "2026.10" },
      "2026.11.02": { data_version: "2026.11.02", catalog_data_version: "2026.09" },
    }),
  );
  assert.equal(pair.dataVersion, "2026.09");
  assert.equal(pair.shopVersion, "2026.11.02");
});

test("refuses a manifest that names a different shop version", async () => {
  const releases = [release("data-2026.09"), release("shop-2026.11.09")];
  await assert.rejects(
    () => choosePair(releases, manifests({ "2026.11.09": { data_version: "2026.11.02", catalog_data_version: "2026.09" } })),
    /manifest of shop-2026\.11\.09 says it is 2026\.11\.02/,
  );
});

test("refuses when no shop release pairs with any data release", async () => {
  const releases = [release("data-2026.09"), release("shop-2026.11.09")];
  await assert.rejects(
    () => choosePair(releases, manifests({ "2026.11.09": { data_version: "2026.11.09", catalog_data_version: "2026.08" } })),
    /no shop release pairs with a published data release/,
  );
});

test("updates only the line named, leaving comments and the other version alone", () => {
  const before = [
    "builder:",
    "  args:",
    "    # DATA_VERSION: not this comment",
    '    DATA_VERSION: "2026.09"',
    '    SHOP_VERSION: "2026.11.09"',
    "",
  ].join("\n");
  const after = deployLineUpdate(before, "DATA_VERSION", "2026.10");
  assert.equal(
    after,
    [
      "builder:",
      "  args:",
      "    # DATA_VERSION: not this comment",
      '    DATA_VERSION: "2026.10"',
      '    SHOP_VERSION: "2026.11.09"',
      "",
    ].join("\n"),
  );
});

test("refuses to invent a version line that is not there", () => {
  assert.throws(() => deployLineUpdate("SHOP_VERSION: \"2026.11.09\"\n", "DATA_VERSION", "2026.10"), /has 0 DATA_VERSION lines/);
});

test("refuses two version lines, because it cannot know which one", () => {
  const text = '  DATA_VERSION: "2026.09"\n  DATA_VERSION: "2026.10"\n';
  assert.throws(() => deployLineUpdate(text, "DATA_VERSION", "2026.11"), /has 2 DATA_VERSION lines/);
});

test("reads the data and shop versions out of the weak ETag", () => {
  assert.deepEqual(liveVersionsFromEtag('W/"2026.09+2026.11.09-0.4.0-6715fdec686d201e"'), {
    dataVersion: "2026.09",
    shopVersion: "2026.11.09",
  });
});

test("returns nothing for an ETag that carries no versions", () => {
  assert.equal(liveVersionsFromEtag('W/"build-abc"'), null);
});

test("allows a clean main that is only behind origin", () => {
  assert.equal(
    preflightProblem({ branch: "main", status: "", ahead: 0, behind: 3, hasKamal: true, hasToken: true }),
    null,
  );
});

test("refuses a branch other than main", () => {
  assert.match(
    preflightProblem({ branch: "feature", status: "", ahead: 0, behind: 0, hasKamal: true, hasToken: true }),
    /main/,
  );
});

test("refuses an uncommitted tree", () => {
  assert.match(
    preflightProblem({ branch: "main", status: " M Cargo.toml", ahead: 0, behind: 0, hasKamal: true, hasToken: true }),
    /uncommitted/,
  );
});

test("refuses unpushed commits on main", () => {
  assert.match(
    preflightProblem({ branch: "main", status: "", ahead: 2, behind: 0, hasKamal: true, hasToken: true }),
    /unpushed/,
  );
});

test("refuses a diverged main", () => {
  assert.match(
    preflightProblem({ branch: "main", status: "", ahead: 2, behind: 1, hasKamal: true, hasToken: true }),
    /diverged/,
  );
});

test("refuses without kamal", () => {
  assert.match(
    preflightProblem({ branch: "main", status: "", ahead: 0, behind: 0, hasKamal: false, hasToken: true }),
    /kamal/,
  );
});

test("refuses without the Bitwarden token that kamal's secrets need", () => {
  assert.match(
    preflightProblem({ branch: "main", status: "", ahead: 0, behind: 0, hasKamal: true, hasToken: false }),
    /BWS_ACCESS_TOKEN/,
  );
});

test("allows a config/deploy.yml left by a ship that stopped before committing", () => {  assert.equal(
    preflightProblem({
      branch: "main",
      status: " M config/deploy.yml",
      ahead: 0,
      behind: 0,
      hasKamal: true,
      hasToken: true,
    }),
    null,
  );
});

/** The two answers verifyLive reads, from a service that is running the pair. */
function liveService({ data = "2026.09", shop = "2026.11.09" } = {}) {
  return async (url) => {
    if (url.endsWith("/health")) return { ok: true, json: async () => ({ data_version: data, status: "ok" }) };
    if (url.endsWith("/v1/meta")) {
      return {
        ok: true,
        headers: new Map([["etag", `W/"${data}+${shop}-0.4.0-6715fdec686d201e"`]]),
      };
    }
    throw new Error(`unexpected URL ${url}`);
  };
}

test("accepts the live service when its data and shop are the ones shipped", async () => {
  await verifyLive({
    baseUrl: "https://example.invalid",
    dataVersion: "2026.09",
    shopVersion: "2026.11.09",
    fetchImpl: liveService(),
    sleep: async () => {},
  });
});

test("rejects a live service still serving the old shop file", async () => {
  await assert.rejects(
    verifyLive({
      baseUrl: "https://example.invalid",
      dataVersion: "2026.09",
      shopVersion: "2026.11.09",
      fetchImpl: liveService({ shop: "2026.11.02" }),
      sleep: async () => {},
    }),
    /shop 2026\.11\.02/,
  );
});

test("rejects a live service still serving the old data file", async () => {
  await assert.rejects(
    verifyLive({
      baseUrl: "https://example.invalid",
      dataVersion: "2026.09",
      shopVersion: "2026.11.09",
      fetchImpl: liveService({ data: "2026.08", shop: "2026.11.09" }),
      sleep: async () => {},
    }),
    /data 2026\.08/,
  );
});

test("takes the verification address from the environment first", () => {
  assert.equal(
    baseUrlFrom({ SHIP_BASE_URL: "http://localhost:3999" }, '    OPEN_BASE_URL: https://open.wenmarpro.com\n'),
    "http://localhost:3999",
  );
});

test("otherwise verifies the address config/deploy.yml publishes", () => {
  assert.equal(
    baseUrlFrom({}, '    OPEN_BASE_URL: https://open.wenmarpro.com\n'),
    "https://open.wenmarpro.com",
  );
});

test("falls back to the public address when config/deploy.yml names none", () => {
  assert.equal(baseUrlFrom({}, "builder:\n"), "https://open.wenmarpro.com");
});

test("a dry run picks the pair, names the skipped data release, and changes nothing", () => {
  const scratch = mkdtempSync(join(tmpdir(), "wenmar-open-ship-"));
  try {
    const repo = join(scratch, "repo");
    const origin = join(scratch, "origin.git");
    mkdirSync(join(repo, "config"), { recursive: true });
    const git = (...args) => {
      const done = spawnSync("git", args, { cwd: repo, encoding: "utf8" });
      assert.equal(done.status, 0, done.stderr);
      return done.stdout;
    };
    git("init", "--quiet", "--initial-branch=main");
    git("config", "user.name", "test");
    git("config", "user.email", "test@example.invalid");
    writeFileSync(
      join(repo, "config/deploy.yml"),
      [
        "builder:",
        "  args:",
        '    DATA_VERSION: "2026.09"',
        '    SHOP_VERSION: "2026.11.02"',
        "env:",
        "  clear:",
        "    OPEN_BASE_URL: https://open.example.invalid",
        "",
      ].join("\n"),
    );
    git("add", "--all");
    git("commit", "--quiet", "-m", "initial");
    spawnSync("git", ["init", "--quiet", "--bare", origin], { encoding: "utf8" });
    git("remote", "add", "origin", origin);
    git("push", "--quiet", "-u", "origin", "main");

    const releases = join(scratch, "releases");
    mkdirSync(releases);
    writeFileSync(
      join(releases, "releases.json"),
      JSON.stringify([
        release("data-2026.10"),
        release("data-2026.09"),
        release("shop-2026.11.09"),
        release("shop-2026.11.02"),
      ]),
    );
    writeFileSync(
      join(releases, "wenmar-open-shop-2026.11.09.json"),
      JSON.stringify({ data_version: "2026.11.09", catalog_data_version: "2026.09" }),
    );
    writeFileSync(
      join(releases, "wenmar-open-shop-2026.11.02.json"),
      JSON.stringify({ data_version: "2026.11.02", catalog_data_version: "2026.09" }),
    );

    const stub = join(scratch, "stub");
    mkdirSync(stub);
    writeFileSync(join(stub, "kamal"), "#!/bin/sh\nexit 0\n", { mode: 0o755 });

    const ship = fileURLToPath(new URL("./ship.mjs", import.meta.url));
    const done = spawnSync("node", [ship, "--dry-run", "--root", repo], {
      cwd: repo,
      encoding: "utf8",
      env: {
        ...process.env,
        PATH: `${stub}:${process.env.PATH}`,
        BWS_ACCESS_TOKEN: "test",
        SHIP_RELEASES_DIR: releases,
      },
    });
    assert.equal(done.status, 0, done.stderr);
    assert.match(done.stdout, /newest data release is 2026\.10/);
    assert.match(done.stdout, /newest complete pair is data 2026\.09 with shop 2026\.11\.09/);
    assert.match(done.stdout, /would fetch data 2026\.09 and shop 2026\.11\.09/);
    const deploy = readFileSync(join(repo, "config/deploy.yml"), "utf8");
    assert.match(deploy, /DATA_VERSION: "2026\.09"/);
    assert.match(deploy, /SHOP_VERSION: "2026\.11\.02"/);
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
});

test("the whole way through: downloads, commits, pushes, deploys, and checks the live pair", async () => {
  const scratch = mkdtempSync(join(tmpdir(), "wenmar-open-ship-full-"));
  let server;
  try {
    const repo = join(scratch, "repo");
    const origin = join(scratch, "origin.git");
    const logs = join(scratch, "logs");
    mkdirSync(join(repo, "config"), { recursive: true });
    mkdirSync(join(repo, "scripts"), { recursive: true });
    mkdirSync(logs);

    const git = (...args) => {
      const done = spawnSync("git", args, { cwd: repo, encoding: "utf8" });
      assert.equal(done.status, 0, done.stderr);
      return done.stdout;
    };
    git("init", "--quiet", "--initial-branch=main");
    git("config", "user.name", "test");
    git("config", "user.email", "test@example.invalid");
    writeFileSync(
      join(repo, "config/deploy.yml"),
      [
        "builder:",
        "  args:",
        '    DATA_VERSION: "2026.09"',
        '    SHOP_VERSION: "2026.11.02"',
        "env:",
        "  clear:",
        "    OPEN_BASE_URL: https://open.example.invalid",
        "",
      ].join("\n"),
    );
    // Only the external edges are doubles. The release files and the live
    // service are below; git is real.
    writeFileSync(join(repo, "scripts/data-pull.sh"), "#!/bin/sh\nexit 0\n", { mode: 0o755 });
    writeFileSync(join(repo, "scripts/deploy-check.sh"), "#!/bin/sh\nexit 0\n", { mode: 0o755 });
    writeFileSync(
      join(repo, "scripts/shop-pull.mjs"),
      [
        'import { readFileSync, writeFileSync } from "node:fs";',
        'const root = process.argv[process.argv.indexOf("--root") + 1];',
        'const file = `${root}/config/deploy.yml`;',
        'const text = readFileSync(file, "utf8").replace(/^( *SHOP_VERSION: ).*$/m, `$1"2026.11.09"`);',
        "writeFileSync(file, text);",
        "",
      ].join("\n"),
      { mode: 0o755 },
    );
    git("add", "--all");
    git("commit", "--quiet", "-m", "initial");
    spawnSync("git", ["init", "--quiet", "--bare", origin], { encoding: "utf8" });
    git("remote", "add", "origin", origin);
    git("push", "--quiet", "-u", "origin", "main");

    const releases = join(scratch, "releases");
    mkdirSync(releases);
    writeFileSync(
      join(releases, "releases.json"),
      JSON.stringify([
        release("data-2026.10"),
        release("data-2026.09"),
        release("shop-2026.11.09"),
        release("shop-2026.11.02"),
      ]),
    );
    writeFileSync(
      join(releases, "wenmar-open-shop-2026.11.09.json"),
      JSON.stringify({ data_version: "2026.11.09", catalog_data_version: "2026.09" }),
    );
    writeFileSync(
      join(releases, "wenmar-open-shop-2026.11.02.json"),
      JSON.stringify({ data_version: "2026.11.02", catalog_data_version: "2026.09" }),
    );

    const stub = join(scratch, "stub");
    mkdirSync(stub);
    const kamalLog = join(logs, "kamal");
    writeFileSync(join(stub, "kamal"), `#!/bin/sh\necho "$@" >> "${kamalLog}"\nexit 0\n`, { mode: 0o755 });

    const serverFile = join(scratch, "service.mjs");
    writeFileSync(
      serverFile,
      [
        'import { createServer } from "node:http";',
        "const server = createServer((request, response) => {",
        '  if (request.url === "/health") {',
        '    response.setHeader("content-type", "application/json");',
        '    response.end(JSON.stringify({ data_version: "2026.09", status: "ok" }));',
        '  } else if (request.url === "/v1/meta") {',
        '    response.setHeader("etag", \'W/"2026.09+2026.11.09-0.4.0-6715fdec686d201e"\');',
        '    response.end("{}");',
        "  } else {",
        "    response.statusCode = 404;",
        "    response.end();",
        "  }",
        "});",
        'server.listen(0, "127.0.0.1", () => console.log(server.address().port));',
        "",
      ].join("\n"),
    );
    server = spawn(process.execPath, [serverFile], { stdio: ["ignore", "pipe", "inherit"] });
    const [chunk] = await once(server.stdout, "data");
    const port = Number(chunk.toString().trim());

    const ship = fileURLToPath(new URL("./ship.mjs", import.meta.url));
    const done = spawnSync("node", [ship, "--root", repo], {
      cwd: repo,
      encoding: "utf8",
      env: {
        ...process.env,
        PATH: `${stub}:${process.env.PATH}`,
        BWS_ACCESS_TOKEN: "test",
        SHIP_RELEASES_DIR: releases,
        SHIP_BASE_URL: `http://127.0.0.1:${port}`,
      },
    });
    assert.equal(done.status, 0, done.stderr);
    assert.match(done.stdout, /newest complete pair is data 2026\.09 with shop 2026\.11\.09/);
    assert.match(done.stdout, /live on data 2026\.09 and shop 2026\.11\.09/);

    const deploy = readFileSync(join(repo, "config/deploy.yml"), "utf8");
    assert.match(deploy, /DATA_VERSION: "2026\.09"/);
    assert.match(deploy, /SHOP_VERSION: "2026\.11\.09"/);
    assert.equal(
      spawnSync("git", ["--git-dir", origin, "log", "--format=%s", "-1", "main"], { encoding: "utf8" }).stdout.trim(),
      "deploy: data 2026.09, shop 2026.11.09",
    );
    assert.equal(readFileSync(kamalLog, "utf8").trim(), "deploy");
  } finally {
    if (server !== undefined) server.kill();
    rmSync(scratch, { recursive: true, force: true });
  }
});
