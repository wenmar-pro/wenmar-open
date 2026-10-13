#!/usr/bin/env bash
# Publishes server.json to the MCP Registry, as docs/ai-discovery.md says.
#
# The name com.wenmarpro/wenmar-open is proven with the domain: the registry
# checks a signature against the TXT record at wenmarpro.com. The private
# key of that record is in Bitwarden Secrets Manager, as the PEM file
# openssl wrote, under the name in MCP_REGISTRY_KEY (default: key.pem). It
# is read from there, held in a file only this script can read, and removed
# when the script ends.
#
# Needs bws with BWS_ACCESS_TOKEN set, jq, openssl and mcp-publisher.
#
#   scripts/registry-publish.sh           validate, sign in and publish
#   scripts/registry-publish.sh --check   fetch the key and validate; publish nothing
#
# The registry takes a version once. Publish after the version in
# server.json is deployed, so the listing and the live server agree.
set -euo pipefail
cd "$(dirname "$0")/.."

check=0
case "${1:-}" in
  "") ;;
  --check) check=1 ;;
  *) echo "usage: $0 [--check]" >&2; exit 2 ;;
esac

for tool in bws jq openssl mcp-publisher; do
  command -v "$tool" >/dev/null || { echo "missing: $tool" >&2; exit 1; }
done
[ -n "${BWS_ACCESS_TOKEN:-}" ] || { echo "BWS_ACCESS_TOKEN is not set" >&2; exit 1; }

name="${MCP_REGISTRY_KEY:-key.pem}"
domain="wenmarpro.com"

umask 077
pem="$(mktemp)"
trap 'rm -f "$pem"' EXIT

bws secret list | jq -r --arg name "$name" \
  '[.[] | select(.key == $name)] | if length == 1 then .[0].value else empty end' >"$pem"
[ -s "$pem" ] || { echo "no single secret named '$name' in Bitwarden" >&2; exit 1; }
openssl pkey -in "$pem" -noout 2>/dev/null \
  || { echo "the secret '$name' is not a private key in PEM form" >&2; exit 1; }

# The 32 bytes of the Ed25519 key, as hex: what mcp-publisher signs with.
private_key="$(openssl pkey -in "$pem" -noout -text | grep -A3 "priv:" | tail -n +2 | tr -d ' :\n')"
public_key="$(openssl pkey -in "$pem" -pubout -outform DER | tail -c 32 | base64)"
echo "The TXT record at $domain must hold: v=MCPv1; k=ed25519; p=$public_key"

mcp-publisher validate server.json
version="$(jq -r .version server.json)"
if [ "$check" = 1 ]; then
  echo "Checked. Version $version was not published."
  exit 0
fi

mcp-publisher login dns --domain "$domain" --private-key "$private_key"
mcp-publisher publish
curl -fsS "https://registry.modelcontextprotocol.io/v0.1/servers?search=wenmar-open" \
  | jq '.servers[].server | {name, version, description}'
