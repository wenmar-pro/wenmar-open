# Deploying open.wenmarpro.com

This is the procedure for putting Wenmar Open on the internet and keeping it up to date. It is written for the person who operates `app.wenmarpro.com`.

## What runs where

- One container, `wenmar-open`, on the same server as `app.wenmarpro.com`.
- The Kamal proxy already on that server terminates TLS and routes `open.wenmarpro.com` to it.
- The container is limited to half a CPU and 256 MB of memory. It uses about 125 MB.
- The image holds the server binary and one data file. There is no volume, no database to back up, and no secret.

## Before the first deploy

Do these once.

1. **DNS.** Add an `A` record for `open.wenmarpro.com` pointing at the server that hosts `app.wenmarpro.com` (and an `AAAA` record if that server has one). The proxy asks Let's Encrypt for a certificate on the first request, which fails if the name does not resolve yet. Check:

   ```bash
   dig +short app.wenmarpro.com
   dig +short open.wenmarpro.com
   ```

   Both must print the same address.

2. **Cookies.** The product's session cookie must belong to `app.wenmarpro.com` alone, so the public service never receives it. Sign in to the product and look at the cookie in the browser's developer tools, or:

   ```bash
   curl -sI https://app.wenmarpro.com/ | grep -i '^set-cookie'
   ```

   No cookie may have `Domain=wenmarpro.com` or `Domain=.wenmarpro.com`. A cookie with no `Domain` attribute is right.

3. **A data file.** The image is built around `data/build/wenmar-open-<version>.sqlite3`.

   ```bash
   mise run data
   mise run parity
   mise run catalog-parity
   ```

   Or download the file from the newest `data-YYYY.MM` release into `data/build/`.

4. **The data version.** Set `builder.args.DATA_VERSION` in `config/deploy.yml` to that version and commit the change.

5. **Tools.** Docker must be running on the machine you deploy from. Kamal is not one of this repository's tools; use the one you use for `wenmar`. The registry in `config/deploy.yml` is the same one `wenmar` uses.

## Try the image first

```bash
docker build --build-arg DATA_VERSION=2026.09 -t wenmar-open:local .
docker run --rm -p 3999:3000 --cpus 0.5 --memory 256m wenmar-open:local
```

The build ends by opening the data file; it fails if the file is missing or was built for another schema version. In a second terminal:

```bash
curl -s http://localhost:3999/health
curl -s http://localhost:3999/v1/vin/1HGCM82633A004352
docker stats --no-stream
```

Stop it with Ctrl-C.

## Deploy

Deploy from a clean checkout of `main`. Kamal builds from the working directory, because the data file is not in git, so anything uncommitted there would be built into the image.

```bash
git status --short        # must print nothing
kamal deploy
```

The image is built for `amd64`. On an Apple Silicon machine the first build takes a long time.

If Kamal reports that the proxy is not running on the server, something is wrong with the server, not with this service: the proxy is the one `wenmar` already uses. Do not run `kamal setup` without understanding why.

## Check it

```bash
curl -s https://open.wenmarpro.com/health
curl -si https://open.wenmarpro.com/v1/meta
curl -s https://open.wenmarpro.com/v1/vin/1HGCM82633A004352
kamal app logs | tail -20
```

`/v1/meta` must show the data version you deployed, an `x-data-version` header, and no `set-cookie` header. The log lines show `GET /v1/vin/1HGCM82633A` and never the last six characters of a VIN.

**Check that the abuse ceiling sees real addresses.** This is the one setting that cannot be tested before the service is behind the real proxy. From your own machine, send 601 requests that each claim a different address:

```bash
for i in $(seq 1 601); do
  curl -s -o /dev/null -w '%{http_code}\n' -H "X-Forwarded-For: 10.0.0.$((i % 250))" \
    https://open.wenmarpro.com/v1/meta
done | sort | uniq -c
kamal app logs | grep 'abuse ceiling' | tail -1
```

Expected: 600 answers of `200` and one of `429`, and a log line `over the abuse ceiling address=` followed by your own public address.

- If every answer is `200`, the forged header is being believed. Set `OPEN_TRUSTED_PROXIES` to `0` in `config/deploy.yml`, deploy, and investigate before going further.
- If the address in the log is a private one such as `172.18.0.3`, the proxy is not passing the client's address on, and every visitor is being counted as one. Add `forward_headers: true` under `proxy:` in `config/deploy.yml`, deploy, and run the check again.

## A new data version

Each month, or when a data release is published:

1. Put the new data file in `data/build/` (step 3 above).
2. Change `DATA_VERSION` in `config/deploy.yml` and commit.
3. `kamal deploy`.
4. `curl -s https://open.wenmarpro.com/v1/meta` shows the new version.

Cached answers carry the old version in their `ETag`, so clients fetch fresh ones on their own.

## If something goes wrong

- **The new container does not become healthy.** Kamal leaves the previous one serving. `kamal app logs` shows why; the usual cause is a data file of the wrong schema version, which the image build should already have refused.
- **A bad release is live.** `kamal rollback <version>` returns to an earlier image; `kamal app containers` lists the versions on the server.
- **The container is killed for memory.** Set `OPEN_CONNECTIONS: "2"` under `env.clear` in `config/deploy.yml` and deploy. Each connection keeps its own copy of the list of makes.
- **A flood of traffic.** The container's limits protect the product, but the traffic still arrives at the product's address. Put a CDN in front of `open.wenmarpro.com`, or move the service to its own server. Neither needs a code change; with a CDN in front, set `OPEN_TRUSTED_PROXIES` to `2`.

## Settings

| Variable | Default | Meaning |
|---|---|---|
| `OPEN_DATA` | none | Path of the data file. Required. |
| `PORT` | `3000` | Port to listen on. |
| `OPEN_BASE_URL` | `https://open.wenmarpro.com` | The public address, used in links. |
| `OPEN_TRUSTED_PROXIES` | `0` | Reverse proxies in front that add to `X-Forwarded-For`. `1` in production. |
| `OPEN_RATE_LIMIT` | `600` | Requests one address may make in a minute. |
| `OPEN_CONNECTIONS` | `4` | Read-only connections to the data file. |
| `RUST_LOG` | `info,turso_core=error,tantivy=warn` | Log level. |
