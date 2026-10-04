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

The pages:

```bash
curl -s -o /dev/null -w '%{http_code}\n' https://open.wenmarpro.com/
curl -sI https://open.wenmarpro.com/vin/1HGCM82633A004352 | grep -i -E 'x-robots-tag|cache-control'
curl -s https://open.wenmarpro.com/robots.txt
curl -s https://open.wenmarpro.com/sitemap.xml | head -5
```

The home page answers `200`; the result page carries `x-robots-tag: noindex` and `cache-control: private, max-age=3600`; `robots.txt` names `https://open.wenmarpro.com/sitemap.xml`; and the sitemap's addresses start with `https://open.wenmarpro.com/`. If they start with anything else, `OPEN_BASE_URL` is wrong in `config/deploy.yml`.

What crawlers, agents and a shared link are given:

```bash
curl -s https://open.wenmarpro.com/robots.txt | grep -c '^User-agent: '
curl -sI https://open.wenmarpro.com/assets/fonts/dm-sans-latin-wght.woff2 | grep -i -E '^(content-type|cache-control)'
curl -sI https://open.wenmarpro.com/assets/og.png | grep -i '^content-type'
curl -sI https://open.wenmarpro.com/.well-known/api-catalog | grep -i -E '^(link|content-type)'
curl -s https://open.wenmarpro.com/ | grep -c 'application/ld+json'
curl -s https://open.wenmarpro.com/vin/1HGCM82633A004352 | grep -c -E 'application/ld\+json|og:url|rel="canonical"'
```

They print `22`; `font/woff2` and `public, max-age=31536000, immutable`; `image/png`; the catalog's `link` and `application/linkset+json`; `1`; and `0`. The last one is the point: a result page carries no structured data, no canonical address and no address for a shared link.

How heavy the home page is, with everything it loads:

```bash
total=0
for file in / /assets/site.css /assets/site.js /assets/fonts/dm-sans-latin-wght.woff2 \
    /assets/fonts/jetbrains-mono-latin-400.woff2 /assets/favicon.svg; do
  size=$(curl -s -o /dev/null -w '%{size_download}' "https://open.wenmarpro.com$file")
  echo "$size $file"
  total=$((total + size))
done
echo "$total in all"
```

The total is under 150,000 bytes, and the first line, the page itself, under 40,000. A proxy or a CDN that compresses makes both smaller; the service itself does not compress.

Once the site is live, [ai-discovery.md](ai-discovery.md) lists what to do by hand so that search engines and AI assistants can find it: the sitemap, the MCP Registry, the connector directories. Nothing in this repository does any of it.

`/v1/meta` must show the data version you deployed, an `x-data-version` header, and no `set-cookie` header. The service's log lines show `GET /v1/vin/1HGCM82633A` and never the last six characters of a VIN. That holds at any address: every part of a path that the caller chose is cut to 11 characters, so `GET /v1/vehicles/1HGCM82633A004352` is logged as `GET /v1/vehicles/1HGCM82633A`.

**That check covers the service's log, not the proxy's.** `kamal app logs` shows what the service writes. The Kamal proxy in front of it writes a request log of its own, with each request's whole path and query string, the client's address and the user agent. A decode through the API is `GET /v1/vin/` followed by the whole VIN. A decode on the website is `GET /vin?vin=` followed by what was typed, which redirects to `GET /vin/` followed by the whole VIN. So the proxy's log would hold every VIN decoded in any of those three ways, next to who asked. Batch decodes and MCP calls send VINs in the body, which neither log records.

This has not been checked against the running proxy; it rests on how the proxy logs requests. Check it on the server after the first deploy:

```bash
kamal proxy logs | grep -E '/vin[/?]' | tail -5
```

That matches all three addresses: `/v1/vin/{VIN}`, `/vin/{VIN}` and `/vin?vin={VIN}`. Decode one VIN each way first, so there is a line of each kind to look at.

If those lines show all 17 characters, the rule that logs keep only the first 11 characters of a VIN is not met on this server, whatever the service does. The service cannot fix that. Choose one before telling anyone the service is live:

- **Accept it and say what is true.** The about page already does: under "What it keeps" it says the service's own log keeps 11 characters and that the hosting proxy keeps a request log with whole addresses. Reword the rule the same way in the design's operations section, and add how long the proxy's log is kept if you want the page to say so.
- **Keep the proxy's log short.** The proxy logs to its container's output, so Docker's log settings for the `kamal-proxy` container decide how much is kept: `docker inspect kamal-proxy --format '{{.HostConfig.LogConfig}}'` shows them. The proxy is shared with `app.wenmarpro.com`, so a change applies to that product's request log too.
- **Keep whole VINs out of it.** Give this service a proxy whose request log is off or leaves out the path, which means its own server or its own proxy.

Until one is chosen, do not say publicly that a VIN's serial number is never logged. The about page does not say it: its words are in `crates/open-server/src/site/pages.rs`, in `about`. If the lines show no whole VIN, or once whole VINs are kept out of the proxy's log, the sentence there about the hosting proxy can be taken out; change it only after this check, and change the test beside it (`the_about_page_claims_only_what_is_known_to_be_true`) with it.

The about page also does not say that Wenmar Pro decodes VINs through this service. Add that sentence when Wenmar Pro has been switched over to it, and not before.

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

## Look at the pages

Tests check what the pages contain. Only looking checks how they look. Before the first deploy, and after any change to the stylesheet or the templates, run `mise run serve` and go through this list in a browser.

- **Phone width.** At 320 pixels wide, nothing scrolls sideways on `/`, `/vin/1HGCM82633A004352`, `/makes`, `/makes/honda`, `/makes/honda/civic/2019`, `/guides/model-year` and `/docs`. A code block on `/docs` scrolls inside its own box. The header is two rows: the wordmark and "by Wenmar Pro", then the navigation.
- **The wordmark.** "Wenmar" in the text colour and "Open" in red, as one word, in bold DM Sans. "by Wenmar Pro" is small and grey beside it.
- **One red action.** Apart from the wordmark, the only red thing on the home page is the Decode button. A result page, a list of makes and a guide have none. The outline around whatever has keyboard focus is red too; that is the focus ring, and it moves.
- **Type.** Text is in DM Sans, and VINs, codes and code blocks are in JetBrains Mono. In the browser's network panel the only font requests are two files under `/assets/fonts/`, and no request goes to another host. With those two files blocked, every page is still readable in the system's own faces, and the text does not jump when they are unblocked.
- **Without JavaScript.** With scripts turned off, type a VIN and press Enter; pick a year and a make and press the button. Both work. The copy and print buttons are not shown.
- **With JavaScript.** On a result page, Copy puts the whole sheet on the clipboard as text, with the vehicle and the VIN on its first two lines, and "Copied" appears beside the buttons. Print opens the print dialogue.
- **Keyboard only.** Tab from the top of the home page: the first stop is "Skip to content", every link and field shows a clear outline when it has focus, and Enter submits each form.
- **Dark scheme.** Switch the system to dark. Text is readable on every page, fields have a visible edge, a warning on a result page is amber on dark amber, and the reds are the same two as in light.
- **Print.** Print preview of a result page, from the light scheme and again from the dark one, shows black text on white: the wordmark in black, the heading, the VIN, the tables and the data version. No navigation, buttons, links list or Wenmar Pro line.
- **A calculator.** At 320 pixels wide, `/tools/parts-matrix` shows eight rows of three boxes with a figure such as `1000.00` whole in each, the header's six entries are on one row under the wordmark, and only the result table scrolls sideways, inside its own box. In the dark scheme a field that could not be read has an amber edge and an amber message. Print preview shows the rows that hold something and the result, and no examples, button or navigation.
- **The labor rate calculator.** At 320 pixels wide, `/tools/labor-rate` shows twelve boxes, one to a row, each as wide as the page's column, and its two result tables wrap inside that column with nothing scrolling sideways. Print preview shows both parts of the form and both results, and no button or navigation.
- **A warning.** `/vin/1HGCM82633A004353` (a wrong check digit) shows the decode with one amber line above the tables, which says what position 9 should be and links to the guide.
- **A long name.** The make and the trim with the longest names wrap inside the page at 320 pixels. Find them with:

  ```bash
  sqlite3 -readonly data/build/wenmar-open-*.sqlite3 \
    "SELECT slug, length(name) FROM catalog_make WHERE light = 1 ORDER BY length(name) DESC LIMIT 3"
  ```

- **A shared link.** `/assets/og.png` is the wordmark on off-white with one line under it.
- **A screen reader,** if one is at hand: the page title is read first, the wordmark is read as "Wenmar Open, home", tables are announced with their captions, the VIN field is announced as "VIN", and "Copied" is spoken after Copy is pressed.

## A new data version

Each month, or when a data release is published:

1. Put the new data file in `data/build/` (step 3 above).
2. Change `DATA_VERSION` in `config/deploy.yml` and commit.
3. `kamal deploy`.
4. `curl -s https://open.wenmarpro.com/v1/meta` shows the new version.

Cached answers carry the old version in their `ETag`, so clients fetch fresh ones on their own.

The same holds for a deploy that changes the server and not the data. The `ETag` of every page and answer, and the address of the stylesheet and the script (`?v=`), carry a build id that is worked out from the templates, the assets and the code when the image is built. A reworded page or a fixed stylesheet therefore reaches visitors who hold the old one without the crate's version number being changed. Building the same source again gives the same id, so a redeploy that changes nothing keeps caches warm.

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
