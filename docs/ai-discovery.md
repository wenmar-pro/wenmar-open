# Getting found: search engines, AI assistants and directories

This is what to do by hand after `open.wenmarpro.com` is live. Nothing in this repository does any of it.

**What this can and cannot do.** Nothing here makes an assistant choose this service. An assistant uses a tool because a person connected it, or fetches an address because it found one that looked right. What this work does is make the service easy to find, cheap to try and hard to use wrongly: no key, one request, a plain answer, and a description in each place an assistant or its builder looks. Whether people ask for it is up to them.

Everything below was read on 2026-10-01, except the Server Card and Hermes sections, which were read on 2026-10-10. The MCP Registry, the connector directories and the AI crawlers' rules all change; read the linked page again before acting on a step.

```bash
base=https://open.wenmarpro.com
curl -s $base/robots.txt
curl -s $base/llms.txt | head -20
curl -s -o /dev/null -w '%{http_code} %{size_download}\n' $base/llms-full.txt
curl -si $base/.well-known/api-catalog | head -12
curl -si $base/mcp/server-card | head -12
curl -s  $base/.well-known/ai-catalog.json
curl -s $base/sitemap.xml | head -5
curl -s -X POST $base/mcp \
  -H 'Content-Type: application/json' \
  -H 'MCP-Protocol-Version: 2026-07-28' -H 'Mcp-Method: server/discover' \
  -d '{"jsonrpc":"2.0","id":1,"method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}'
curl -s -X POST $base/mcp -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{}}}'
```

- `robots.txt` has three groups and names `https://open.wenmarpro.com/sitemap.xml`. If it names another address, `OPEN_BASE_URL` is wrong.
- The discover answer lists `2026-07-28` first among `supportedVersions`. The initialize answer has `"protocolVersion":"2025-11-25"`.

## Search engines

1. **Google Search Console** (<https://search.google.com/search-console>). Add `open.wenmarpro.com` as a property and verify it with a DNS TXT record; the server cannot host a verification file. Then give it `https://open.wenmarpro.com/sitemap.xml`.
2. **Bing Webmaster Tools** (<https://www.bing.com/webmasters>). It can import the property from Search Console. Give it the same sitemap. Bing's index is also what several assistants search.
3. **Look at one page of each kind** in Google's Rich Results Test (<https://search.google.com/test/rich-results>) and in the Schema.org validator (<https://validator.schema.org/>): the home page, `/makes/honda`, `/makes/honda/civic/2019`, `/wmi/1HG`, `/guides/model-year`. Expect breadcrumbs to be recognised and no errors. A model-year page is a `Car` with no price; if a tool calls it a product with a missing price, tell the maintainers: it must stay a bare `Car`.
4. **Paste the home page's address into a chat or a message** that shows link previews. The preview shows the wordmark image, the title and the description.

What is deliberately absent: FAQ, how-to and question-and-answer markup. Google stopped showing FAQ results on 2026-05-07 and how-to results in 2023, and its question-and-answer markup is only for pages where visitors submit answers.

Result pages for single VINs never appear in a search engine: they carry `noindex`. If one ever shows up in Search Console's indexed pages, that is a bug to report.

## AI crawlers

`robots.txt` sets the policy; there is nothing to submit. It welcomes the crawlers that build training sets and AI search indexes on everything except single VINs, and welcomes the fetchers that act for one person everywhere. The tokens and their sources are in `crates/open-server/src/site/seo.rs`. Re-read the operators' pages twice a year: tokens are renamed and added.

- Anthropic: <https://privacy.claude.com/en/articles/8896518-does-anthropic-crawl-data-from-the-web-and-how-can-site-owners-block-the-crawler>
- OpenAI: <https://developers.openai.com/api/docs/bots>
- Google: <https://developers.google.com/search/docs/crawling-indexing/google-common-crawlers>
- Perplexity: <https://docs.perplexity.ai/guides/bots>

`llms.txt` and `llms-full.txt` are there for an agent that is pointed at the site, and for a person who pastes one into a chat. Google says its Search does not use such files, and no AI company has said its crawler reads them. Do not expect traffic from them.

**To see whether assistants use the service,** look in the proxy's log for requests to `/v1/vin/` and `/mcp` whose user agent is `Claude-User`, `ChatGPT-User`, `Perplexity-User` or a similar name. Remember what `docs/deploy.md` says of that log: it holds whole VINs.

**The request limit and assistants.** The limit is 600 requests a minute for one address. An assistant's requests come from its company's servers, so many people's questions arrive from a few addresses: Anthropic's from `160.79.104.0/21`. If the service's log shows `over the abuse ceiling` for such an address, raise `OPEN_RATE_LIMIT`.

## The MCP Registry

The official registry is at <https://registry.modelcontextprotocol.io>. It is in preview: "breaking changes or data resets may occur before general availability". Other directories, GitHub's among them, read from it.

`server.json` in the repository root describes the hosted endpoint under the name `com.wenmarpro/wenmar-open`. A name under `com.wenmarpro` must be proven with the domain. Docs: <https://modelcontextprotocol.io/registry/quickstart> and <https://modelcontextprotocol.io/registry/authentication>.

1. The site must be live: "a remote server MUST be publicly accessible at its specified URL".
2. Install the publisher: `brew install mcp-publisher`.
3. Make a key and a DNS record for `wenmarpro.com`:

   ```bash
   openssl genpkey -algorithm Ed25519 -out key.pem
   PUBLIC_KEY="$(openssl pkey -in key.pem -pubout -outform DER | tail -c 32 | base64)"
   echo "wenmarpro.com. IN TXT \"v=MCPv1; k=ed25519; p=${PUBLIC_KEY}\""
   ```

   Add the printed TXT record at the apex of `wenmarpro.com`. Keep `key.pem` out of the repository.
4. Sign in and publish, from the repository root:

   ```bash
   PRIVATE_KEY="$(openssl pkey -in key.pem -noout -text | grep -A3 "priv:" | tail -n +2 | tr -d ' :\n')"
   mcp-publisher login dns --domain wenmarpro.com --private-key "${PRIVATE_KEY}"
   mcp-publisher validate server.json
   mcp-publisher publish
   curl "https://registry.modelcontextprotocol.io/v0.1/servers?search=wenmar-open"
   ```

5. `version` in `server.json` is the server's version, and a test keeps the two equal. Publish again when it changes.

Not done, and why: nothing else. A "server card" is now served at `/mcp/server-card`, `/.well-known/mcp/server-card.json` and `/.well-known/mcp`, and `/.well-known/ai-catalog.json` points at it. The next section says how.

## The MCP Server Card

SEP-2127 reached Final as an Extensions Track SEP. It is a static JSON document
that says what this server is and where to connect, for a client that has a
domain and no connection yet. The values are built from the same constants
`server/discover` answers with, and a test fails if the two ever disagree.

| Address | Media type | What it is for |
|---|---|---|
| `/mcp/server-card` | `application/mcp-server-card+json` | The address SEP-2127 reserves: the card hangs off the streamable-HTTP URL |
| `/.well-known/mcp/server-card.json` | the same | An alias, for a client that only looks under `.well-known` |
| `/.well-known/mcp` | the same | The same, without the `.json` |
| `/.well-known/ai-catalog.json` | `application/ai-catalog+json` | The route that actually answers "which servers does this domain serve?" |

The SEP recommends *against* `.well-known` for a card, on the grounds that
`.well-known` is for site-wide metadata and the AI Catalog already carries each
card's exact `url`. All four are served anyway: a card may sit at any
unreserved URI, and an alias that costs nothing is worth more than an argument
about which address is correct.

**The AI Catalog is the one that matters.** A client given only
`https://open.wenmarpro.com` fetches `/.well-known/ai-catalog.json`, reads the
entry whose `type` is `application/mcp-server-card+json`, and fetches the
`url` it names. Without the catalog there is nothing for a client to read.

Two things the card deliberately does not carry, both by decision of the SEP:

- **No tools, resources or prompts.** A static document cannot say what a given
  client may reach; that varies with identity, configuration and deployment.
- **No `capabilities`.** The same reasoning. A card that advertises them is
  ignored by a conforming client.

The card answers 304 to `If-None-Match`, and carries an `ETag` and
`Cache-Control: public, max-age=3600`. Check it, and the catalog, with:

```bash
curl -si https://open.wenmarpro.com/mcp/server-card | head -12
curl -s  https://open.wenmarpro.com/.well-known/ai-catalog.json
```

## Claude

- **Anyone can add it as a custom connector**: Customize, Connectors, Add custom connector, and the address `https://open.wenmarpro.com/mcp`. No sign-in is asked for. <https://claude.com/docs/connectors/custom/add-unlisted>
- **Claude Code**: `claude mcp add --transport http wenmar-open https://open.wenmarpro.com/mcp`. <https://code.claude.com/docs/en/mcp>
- **The API**: the MCP connector takes the address in `mcp_servers`. <https://platform.claude.com/docs/en/agents-and-tools/mcp-connector>
- **Without MCP at all**, Claude with web fetch can read `https://open.wenmarpro.com/v1/vin/<VIN>` when a person gives it the address or it finds the site. `robots.txt` allows `Claude-User` there.

**The Connectors Directory** is where Claude's users find connectors without knowing the address. <https://claude.com/docs/connectors/building/submission>

1. Submit at <https://claude.ai/directory/manage>: Submit new, MCP connector. It needs a paid Claude plan.
2. What it asks for, and what is ready:

   | Asked for | Status |
   |---|---|
   | Every tool has a `title`, and `readOnlyHint` or `destructiveHint` | Ready. Both tools have a title and `readOnlyHint: true`. |
   | Tool names of 64 characters or fewer | Ready. |
   | No sign-in, or OAuth 2.0 | Ready. "No authentication for public data" is allowed. |
   | A server name (100 characters), a one-liner (200), a description (2,000), one to five categories | Write them. A start is under "Words for a listing" below. |
   | A documentation address | `https://open.wenmarpro.com/docs` |
   | A privacy policy address | **Decide.** `https://open.wenmarpro.com/about` says what is kept. If a policy page is wanted, it is Wenmar Pro's to write. |
   | A support contact | **Decide.** An address at `wenmarpro.com`, or the repository's issues once it is public. |
   | An icon | `crates/open-server/assets/apple-touch-icon.png`, or the mark at a larger size from the brand files. |
   | Test instructions, and that every tool was run | Run both tools from a custom connector first and say so. |

3. A result may be at most about 150,000 characters in Claude, and 25,000 tokens in Claude Code. A batch of 50 decodes is under the first and can be over the second.

## ChatGPT

Two ways, from least work to most.

1. **A custom GPT with an action.** In the GPT builder, add an action and import `https://open.wenmarpro.com/v1/openapi.json`. Choose no authentication. Limits that apply: 300 characters for an endpoint's description, 700 for a parameter's, 100,000 characters for a request or an answer, 45 seconds. <https://developers.openai.com/api/docs/actions/production>. Check the description lengths first:

   ```bash
   curl -s https://open.wenmarpro.com/v1/openapi.json \
     | jq -r '.paths[][] | select(type=="object") | [(.description // "" | length), .operationId] | @tsv' | sort -rn | head
   ```

   A public GPT asks for a privacy policy address; see the same decision above.
2. **The MCP server in ChatGPT.** With developer mode on (Settings, Security and login), add `https://open.wenmarpro.com/mcp` at <https://chatgpt.com/plugins>. <https://developers.openai.com/apps-sdk/deploy/connect-chatgpt>. Listing it for everyone is a submission at <https://platform.openai.com/plugins>, which asks for a verified individual or business, test cases, a video, and four addresses: website, support, privacy policy and terms of service. It also asks for a token served at `https://open.wenmarpro.com/.well-known/openai-apps-challenge`. **The server does not serve that address.** It is a small change when the time comes: one route that answers a token from a setting.

## Other directories and lists

| Where | How | Note |
|---|---|---|
| Hermes Agent | A pull request against `optional-mcps/` in `NousResearch/hermes-agent` | No community tier; entries are merged by review. Written up in `docs/hermes-catalog-submission` |
| GitHub MCP Registry | Publish to the official registry, then ask GitHub to include the server | The instruction is from 2025 and may have changed |
| Smithery | <https://smithery.ai/new>, with the `/mcp` address | Needs Streamable HTTP, which this is |
| mcp.so | <https://mcp.so/submit> | Asks for a repository address: after the repository is public |
| Glama | <https://glama.ai/mcp/connectors>, "Add Connector" | |
| Docker MCP catalog | A pull request to <https://github.com/docker/mcp-registry> | Has a wizard for remote servers |
| PulseMCP | Not taking submissions; it reads the official registry | |
| public-apis | A pull request to <https://github.com/public-apis/public-apis> | Read its contributing rules: one line, free API, no marketing words |
| APIs.guru | The form at <https://apis.guru/add-api/> | Give it the OpenAPI address |

## Words for a listing

Plain, as on the site.

- **Name:** Wenmar Open
- **One line:** Free VIN decoder and vehicle catalog from NHTSA data. No key, no account.
- **Description:** Wenmar Open decodes a 17-character VIN into year, make, model, trim, engine, drivetrain and the safety equipment the manufacturer reported, and looks up vehicles by year, make, model, submodel and engine. The data is NHTSA's vPIC, rebuilt each month. It is read-only, needs no key and no account, and is run by Wenmar Pro, shop management software for independent auto repair shops. A wrong check digit is reported as a warning, not an error, because many genuine VINs from outside North America fail the check.
- **Categories:** automotive, reference data, developer tools.
