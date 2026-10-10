# Releasing

A release publishes one version of everything at once:

- three crates on crates.io: `wenmar-vin`, `wenmar-vehicles`, `wenmar-open-db`;
- the npm package `wenmar-open`;
- a GitHub release `v<version>`, with that version's section of the changelog as its notes and the `wenmar-open` command-line tool built for macOS (arm64, x86_64) and Linux (x86_64, arm64).

It starts when a tag `v<version>` is pushed to a commit on `main`, and is run by `.github/workflows/release.yml`. No registry password or token is stored anywhere: crates.io and npm are each told, once, to trust that workflow in this repository.

The data file is released separately, every month, by `.github/workflows/data-release.yml`, as `data-YYYY.MM`. The same run publishes the file to npm as `wenmar-open-data`. A release of the code does not rebuild the data, and a data release does not publish code.

## The monthly release

On the 10th of each month, after the data release at 06:00, run this from a clean checkout of `main`:

```bash
bin/release
```

It is steps 1, 2 and 5 of "Making a release", in one command and with nothing to remember. It refuses rather than half-releasing when the tree is not `main`, when it is dirty, when `main` is not what `origin/main` has, when CI is not green for that commit, when there is no data release to name, or when the version is already tagged. An empty `[Unreleased]` is not a refusal: it stops successfully and says there is nothing to say, because a month with nothing to say is not a release.

It then leaves one commit and one tag in your checkout and prints the two pushes to run. **Nothing is pushed for you.** Read the diff, then:

```bash
git push origin main
git push origin v0.2.0
```

Pushing the tag starts `release.yml`, which is unchanged, and that is what publishes to crates.io, npm, GitHub and the binary archives.

The data version it records is the newest `data-YYYY.MM` release, so run it after the data release. Pass one by hand when the data release has not run yet: `bin/release --data-version 2026.10`.

No secret is stored and no GitHub setting has to change. You push as yourself, and the tag ruleset admits repository administrators, which is how every tag so far was pushed.

`scripts/release.sh` holds the logic and `scripts/release-test.sh` tests it; `bin/release` is the wrapper.

`.github/workflows/monthly-release.yml` does the same thing unattended on the same schedule. It is written and checked, but it is not switched on: its push would be refused by the tag ruleset, because Actions pushes as `github-actions[bot]` rather than as an administrator. Turning it on is one change by hand, described below.

### Seeing what a release would do, without doing it

`scripts/release-bump.sh` does the version work on its own, and commits nothing:

```bash
git clone https://github.com/wenmar-pro/wenmar-open.git /tmp/try-release
cd /tmp/try-release
DATA_VERSION=2026.10 scripts/release-bump.sh
git diff
```

With neither `--patch` nor `--minor` it decides minor or patch from the changelog, following the rule in "Versions". Pass one of those to force it.

## Versions

- Everything has the same version. It is written in `Cargo.toml` (under `[workspace.package]`, and in the three lines for `wenmar-vin`, `wenmar-vehicles` and `wenmar-open-db` under `[workspace.dependencies]`), in `clients/js/package.json` and in `server.json`.
- Until 1.0: a change an application has to react to is a new minor version (`0.1.3` to `0.2.0`). Anything else is a patch (`0.1.3` to `0.1.4`).
- The data file has its own version, `YYYY.MM`, or `YYYY.MM.N` for a rebuild. What ties code to data is the data file's schema version. A version of the code reads data files of one schema version; when that changes, it is a new minor version and the changelog says that older data files must be replaced.
- The npm package `wenmar-open-data` is versioned `<schema>.<YYYYMM>.<rebuild>`: data `2026.09` at schema version 3 is `3.202609.0`. `wenmar-open` names the schema version it reads as an optional peer, `"wenmar-open-data": "^3.0.0"` in `clients/js/package.json`. When the schema version changes, change that range in the same commit; `npm test` in `clients/js` fails until it matches the decoder.
- A schema change needs the code released first, or on the same day. A data release built from `main` after the change is published as the new major version, which no released `wenmar-open` can read until then. People on the old schema keep the last file of that schema; nothing breaks for them, and they get no newer data until they upgrade.
- The API is additive only. Adding a field or an endpoint is a patch or a minor version, never a reason for a `/v2`.

## One-time setup

Do these once, in this order, before the first release. They are done by the owner of the registry accounts, in a browser.

### GitHub

1. The repository must be public. npm will not publish provenance for a package built in a private repository.
2. **Settings, Environments, New environment.** Name it `release`. Under "Deployment branches and tags" choose "Selected branches and tags" and add a tag rule `v*`, so only a release tag can use the environment.
3. Optional: in the same environment, add yourself under "Required reviewers". Each release then waits for your approval before each of its two publishing jobs.
4. **Settings, Rules, Rulesets, New tag ruleset.** Target tags matching `v*`. Restrict creations, updates and deletions, with only repository administrators on the bypass list. Then only an administrator can start a release, and nobody can move a release tag.
5. Nothing further is needed for `bin/release`: you push as yourself, and administrators are already on the bypass list. To run the release unattended instead, see "The monthly release": the `v*` ruleset then needs the GitHub Actions app added to its bypass list, because Actions pushes as `github-actions[bot]` rather than as an administrator. Tags stay restricted to that list, and still cannot be moved or deleted by a person.

The `release` environment needs no change either way: it already admits deployment from a `v*` tag, and that tag is pushed before `release.yml` starts.

Nothing is added under "Secrets and variables".

### crates.io

A crate must exist before it can be given a trusted publisher, so the first version of each crate is published by hand. Do this part when the first release's commit is on `main`: see "The first release" below.

1. Sign in to crates.io with the GitHub account that will own the crates.
2. **Account Settings, API Tokens, New Token.** Name it `first publish`. Scopes: `publish-new` and `publish-update`. Expiry: one day.
3. On a clean checkout of `main`, at the commit that will be tagged:

   ```bash
   mise run release-check
   cargo login
   cargo publish --locked -p wenmar-vin -p wenmar-vehicles -p wenmar-open-db
   cargo logout
   ```

   `cargo login` asks for the token. The one `cargo publish` command publishes the three in dependency order.
4. For each of the three crates, open `https://crates.io/crates/<name>/settings`, find **Trusted Publishing**, and add a GitHub publisher:

   | Field | Value |
   |---|---|
   | Repository owner | `wenmar-pro` |
   | Repository name | `wenmar-open` |
   | Workflow filename | `release.yml` |
   | Environment | `release` |

5. On the same page of each crate, turn on the setting that requires trusted publishing for new versions. A leaked token can then publish nothing.
6. **Account Settings, API Tokens:** revoke `first publish`.

### npm

The package `wenmar-open` already exists: version `0.1.0` is published, and it holds the hosted client only. So no publish by hand is needed.

1. Sign in to npmjs.com as the package's owner. Open the package `wenmar-open`, then **Settings**, then **Trusted Publisher**, and choose **GitHub Actions**:

   | Field | Value |
   |---|---|
   | Organization or user | `wenmar-pro` |
   | Repository | `wenmar-open` |
   | Workflow filename | `release.yml` |
   | Environment name | `release` |
   | Allowed actions | tick `npm publish` |

   A trusted publisher created after 3 September 2026 may only stage a release (`npm stage publish`) unless `npm publish` is ticked. The workflow uses `npm publish`.
2. On the same Settings page, under **Publishing access**, choose **Require two-factor authentication and disallow tokens**.

npm does not check these values when they are saved. Every field is case-sensitive, and the workflow file name must include `.yml`. A mistake shows up as a failed `npm` job in the first release; correct the field and run the job again.

To approve each npm release by hand instead: leave `npm publish` unticked, change `npm publish --provenance --access public` in `release.yml` to `npm stage publish --access public`, and approve the staged version on npmjs.com with two-factor authentication after each release.

### npm, for the data package

`wenmar-open-data` is published by the data release, not by the code release, so it trusts a different workflow in a different environment.

1. **GitHub: Settings, Environments, New environment.** Name it `data-release`. Under "Deployment branches and tags" choose "Selected branches and tags" and add the branch `main`. Optional: add yourself as a required reviewer; each monthly run then waits for you before it publishes to npm.
2. **npmjs.com**, as the owner of `wenmar-open-data` (the name is held by a placeholder version, which is what a trusted publisher needs to attach to): open the package, **Settings**, **Trusted Publisher**, **GitHub Actions**:

   | Field | Value |
   |---|---|
   | Organization or user | `wenmar-pro` |
   | Repository | `wenmar-open` |
   | Workflow filename | `data-release.yml` |
   | Environment name | `data-release` |
   | Allowed actions | tick `npm publish` |

3. On the same page, under **Publishing access**, choose **Require two-factor authentication and disallow tokens**.

The first time, publish the data that is already released: run "Data release" from `main` with "Run workflow". The `release` job finds `data-2026.09` (or the current one) already released and does nothing; the `npm` job downloads that release's file and publishes it.

The package is about 49 MB to upload and 167 MB unpacked. npm documents no size limit; the reports of refused packages begin at about 230 MB packed.

To check a data package after a run:

```bash
npm view wenmar-open-data version dist.unpackedSize
```

## Making a release

1. **On `dev`, set the version.** Replace `0.1.0` with the new version in the four lines of `Cargo.toml`, then:

   ```bash
   (cd clients/js && npm version --no-git-tag-version 0.2.0)
   cargo check --workspace
   UPDATE_OPENAPI=1 cargo test --workspace --all-features --test it openapi
   ```

   The first updates `package.json` and `package-lock.json`; the second, `Cargo.lock`; the third, the version inside `crates/open-server/openapi.json`. `server.json` at the repository root names the hosted server for the MCP registry and takes the version by hand: replace its `version` too, and the check in step 3 refuses a release where it disagrees. `wenmar-open` `0.1.0` is already on npm, with the hosted client only, and npm takes a version once: the release that carries the offline mode is `0.2.0`, so this step is not skipped for it.

2. **Cut the changelog.** In `CHANGELOG.md`, rename `## [Unreleased]` to `## [0.2.0] - 2026-11-03` (the version and today's date) and add a new, empty `## [Unreleased]` above it.

3. **Check.**

   ```bash
   mise run release-check
   ```

   It runs the tests, checks that every version agrees and that the changelog has the section, checks that every action in `release.yml` is pinned to a commit, and runs `cargo publish --dry-run` and `npm pack --dry-run`. It publishes nothing. It is run before the commit, on the changes of steps 1 and 2 as they are in the working tree. Commit when it passes.

4. **Merge `dev` into `main`** in the usual way, and wait for CI on `main`.

5. **Tag `main` and push the tag.**

   ```bash
   git switch main
   git pull
   git tag v0.2.0
   git push origin v0.2.0
   ```

6. **Watch the run** under Actions, "Release". If the `release` environment has a required reviewer, approve the `crates` job and then the `npm` job.

7. **Look at the result.**

   ```bash
   npm view wenmar-open version
   cargo search wenmar-vin
   ```

   The npm page shows a provenance statement naming this repository and the workflow. The GitHub release's notes are the changelog's section, and it has four archives and SHA256SUMS.

## The first release

`wenmar-open` `0.1.0` is published on npm, and it holds the hosted client only. The first release that carries the offline mode is `0.2.0`, and it is made by the steps of "Making a release" above.

The first release differs in one way: the crates are published by hand, because crates.io has nothing to attach a trusted publisher to until they exist. In order:

1. Do the GitHub and npm parts of the one-time setup.
2. Do steps 2 to 4 of "Making a release": cut the changelog for `0.1.0`, run `mise run release-check`, merge to `main`.
3. Do the crates.io part of the one-time setup, from that commit on `main`.
4. Do steps 5 to 7: tag `v0.1.0` and push the tag. The `crates` job finds all three crates on crates.io and publishes nothing, and the release is created.

## The binaries

The release has one archive per platform, `wenmar-open-<target>.tar.gz`, each holding the single file `wenmar-open`, and `SHA256SUMS`.

| Target | Runs on |
|---|---|
| `aarch64-apple-darwin` | macOS on Apple silicon |
| `x86_64-apple-darwin` | macOS on Intel |
| `x86_64-unknown-linux-gnu` | Linux on x86_64, glibc 2.35 or later |
| `aarch64-unknown-linux-gnu` | Linux on arm64, glibc 2.35 or later |

After a release, check one by hand:

```bash
curl -fsSL -o wenmar-open.tar.gz https://github.com/wenmar-pro/wenmar-open/releases/latest/download/wenmar-open-aarch64-apple-darwin.tar.gz
curl -fsSL https://github.com/wenmar-pro/wenmar-open/releases/latest/download/SHA256SUMS | grep aarch64-apple-darwin
shasum -a 256 wenmar-open.tar.gz
tar -xzf wenmar-open.tar.gz
./wenmar-open --version
```

The two checksums must be the same, and the version must be the one released. The macOS binaries are not signed or notarized: one downloaded with a browser is stopped by Gatekeeper until it is allowed in System Settings, and one downloaded with `curl` is not.

If a `binaries` job fails, the version is already published and the release exists. Use "Re-run failed jobs"; the archive is attached when the job passes, and `checksums` runs after all four.

## If a release fails

- **`verify` failed.** Nothing was published. Fix the cause on `dev`, merge, delete the tag (`git push origin :refs/tags/v0.2.0`, then `git tag -d v0.2.0`) and tag the new commit. Deleting a tag is safe only while nothing has been published for it.
- **A later job failed.** Something may be published. Do not delete the tag. Fix what is outside the repository (a registry setting, an outage) and use "Re-run failed jobs". Each step skips what is already published.
- **The data release's `npm` job failed.** The GitHub release exists and is fine. Fix the cause and use "Re-run failed jobs", or run the workflow again from `main`: the `release` job finds the release and does nothing, and the `npm` job publishes the released file. A data package that is wrong cannot be replaced: publish a rebuild (`data_version` `2026.09.1` in "Run workflow", which becomes `3.202609.1`) and `npm deprecate wenmar-open-data@3.202609.0 "use 3.202609.1"`.
- **A published version is wrong.** A version on crates.io or npm cannot be replaced. Release a new patch version, then mark the bad one: `cargo yank --version 0.2.0 wenmar-vin` (and the other two crates), and `npm deprecate wenmar-open@0.2.0 "use 0.2.1"`. Both need you to be signed in.

## What a release does not do

- It does not deploy `open.wenmarpro.com`. That is `docs/deploy.md`.
- It does not build or publish a data file or `wenmar-open-data`. The data release does.
- It does not deploy. The release writes the data version into `config/deploy.yml` and commits it to `main`; `kamal deploy` stays a command a person runs.
