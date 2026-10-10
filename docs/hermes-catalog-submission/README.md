# Submitting wenmar-open to the Hermes MCP catalog

Hermes Agent (`hermes mcp catalog`) lists Nous-approved servers that install
with one command. **There is no community submission tier** — entries are added
by merging a pull request against
[`NousResearch/hermes-agent`](https://github.com/NousResearch/hermes-agent/tree/main/optional-mcps).

Everything below was read on 2026-10-10. Read the linked pages again before
acting: the catalog's manifest format is versioned and still moving.

## What to do

1. Fork and clone `NousResearch/hermes-agent`.
2. Create `optional-mcps/wenmar-open/manifest.yaml` from
   [`wenmar-open.manifest.yaml`](wenmar-open.manifest.yaml) in this folder.
3. Commit on a branch and open a pull request against `main`.

That directory is the whole mechanism: presence means Nous approval.

## Before opening the pull request

Run the two tools from a fresh MCP client and confirm both answers, because
the manifest's `post_install` tells the user they need no credentials, and
that must be true.

```bash
curl -s -X POST https://open.wenmarpro.com/mcp \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{}}}'

curl -s -X POST https://open.wenmarpro.com/mcp \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"wenmar_vin","arguments":{"action":"decode","vin":"KM8K2CAB4PU001140"}}}'
```

The first must answer without a token, a redirect or a 401. The second must
answer with the Hyundai Kona's decode.

## What the manifest says, and why

| Field | Value | Why |
|---|---|---|
| `manifest_version` | `1` | The version the catalog understands today. |
| `name` | `wenmar-open` | The key `hermes mcp install wenmar-open` takes. |
| `description` | One line | Shown in `hermes mcp catalog`. |
| `source` | `https://open.wenmarpro.com/docs` | The picker prints this so a user can verify what they are installing. |
| `transport.type` | `http` | Remote Streamable HTTP. Hermes spawns no process. |
| `auth.type` | `none` | Verified live: `initialize` succeeds anonymously. The same shape as the `alltrails` and `context7` entries. |
| `suggest` | Five keywords, two hosts | Drives the desktop "Add \<server\>" pill. Keywords are matched as completed words, so `vin` will not fire on "vinyl". |

There is no `tools.include` or `tools.exclude`: both tools are worth having
and neither can change anything. A server that exposes the tools capability
and no tools is flagged by `/mcp`, so the `tools:` block is best left off.

## If the pull request is declined

The server is already in the **official MCP Registry** as
`com.wenmarpro/wenmar-open`, which GitHub and several other directories read:

```bash
curl "https://registry.modelcontextprotocol.io/v0.1/servers?search=wenmar-open"
```

Publishing that is described in [`../ai-discovery.md`](../ai-discovery.md).
The card at `https://open.wenmarpro.com/mcp/server-card` (SEP-2127) is the
other route: a client can find this server from the domain alone.