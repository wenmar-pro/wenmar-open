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

`/v1/meta` must show the data version you deployed, an `x-data-version` header, and no `set-cookie` header. The service's log lines show `GET /v1/vin/1HGCM82633A` and never the last six characters of a VIN. That holds at any address: every part of a path that the caller chose is cut to 11 characters, so `GET /v1/vehicles/1HGCM82633A004352` is logged as `GET /v1/vehicles/1HGCM82633A`.

**That check covers the service's log, not the proxy's.** `kamal app logs` shows what the service writes. The Kamal proxy in front of it writes a request log of its own, with each request's whole path and query string, the client's address and the user agent. A decode is `GET /v1/vin/` followed by the whole VIN, so the proxy's log would hold every VIN decoded that way, next to who asked. Batch decodes and MCP calls send VINs in the body, which neither log records.

This has not been checked against the running proxy; it rests on how the proxy logs requests. Check it on the server after the first deploy:

```bash
kamal proxy logs | grep '/v1/vin/' | tail -5
```

If those lines show all 17 characters, the rule that logs keep only the first 11 characters of a VIN is not met on this server, whatever the service does. The service cannot fix that. Choose one before telling anyone the service is live:

- **Accept it and say what is true.** Reword the rule wherever it is stated (the design's operations section, and the site's own description of what is logged): the service's log keeps 11 characters, and the hosting proxy's request log keeps whole addresses for as long as that log is kept.
- **Keep the proxy's log short.** The proxy logs to its container's output, so Docker's log settings for the `kamal-proxy` container decide how much is kept: `docker inspect kamal-proxy --format '{{.HostConfig.LogConfig}}'` shows them. The proxy is shared with `app.wenmarpro.com`, so a change applies to that product's request log too.
- **Keep whole VINs out of it.** Give this service a proxy whose request log is off or leaves out the path, which means its own server or its own proxy.

Until one is chosen, do not say publicly that a VIN's serial number is never logged.

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

## Limits that are not settings

These are fixed in the code. They keep one client, or a few, from using up the container's memory or keeping VIN decodes waiting.

| Limit | Value | Over it |
|---|---|---|
| Address: path and query string | 8 KB | `414` with code `uri_too_long` |
| Request head: request line and all headers | 32 KB | `431` from the HTTP layer, with no JSON body, and the connection is closed |
| Request body | 16 KB | `413` with code `payload_too_large` |
| Requests being answered at once | 512 | `503` with code `unavailable`, at once |
| Time to answer a request | 10 seconds | `503` with code `unavailable` |
| Connections open at once | 1,024 | Further connections wait until one closes |
| Time for a connection to send a request head, including the wait between two requests on a connection kept open | 120 seconds | The connection is closed |
| Heavier reads of the data file at once: free-text search, and years narrowed by `term` or by a vehicle type | Half of `OPEN_CONNECTIONS` | They wait their turn; the other connections stay free for VIN decodes and the rest of the catalog |

`/health` is exempt from the request limit and from the abuse ceiling, so the proxy's check is answered while the service is refusing other requests. It is not exempt from the connection limit.

The 120 seconds must stay longer than the time the proxy keeps an idle connection to the service open, so that the proxy closes first. If the proxy's log ever shows `502` for requests the service never logged, compare the two.

Free-text search reaches model years through an index. On the 2026.09 data file, in a release build on a development machine with every core free, a search is answered in 1 to 8 ms, 50 searches sent at once are all answered within 100 ms, and VIN decodes sent during them take 3 ms. Years narrowed by `term` or by a vehicle type still read every model year: 60 to 110 ms. Neither has been measured in the release image under `--cpus 0.5`. Do that before announcing search, against the container from "Try the image first":

```bash
for q in a chevy+1500 ram+1500+2019+big+horn "a+b+c+d+e+f+g+h&scope=all"; do
  curl -s -o /dev/null -w "%{http_code} %{time_total}s $q\n" "http://localhost:3999/v1/vehicles/search?q=$q"
done
```
