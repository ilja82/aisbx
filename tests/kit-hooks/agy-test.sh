#!/bin/sh
# Tests the agy kit's startup hook, files/agy/settings.sh, against a scratch home.
# Needs sh and jq only: no root, no sbx, no network.
#
#   sh tests/kit-hooks/agy-test.sh

root=$(cd "$(dirname "$0")/../.." && pwd) || exit 2
script=$root/kits/agy/files/agy/settings.sh
command -v jq >/dev/null 2>&1 || { echo "BLOCKED: jq"; exit 127; }
failures=0
check() {   # check <what> <command>...: one line per check, counted
  what=$1
  shift
  if "$@"; then
    printf 'ok   %s\n' "$what"
  else
    printf 'FAIL %s\n' "$what"
    failures=$((failures + 1))
  fi
}
same() { [ "$1" = "$2" ]; }

scratch=$(mktemp -d "${TMPDIR:-/tmp}/agy-test.XXXXXX") || exit 2
trap 'rm -rf "$scratch"' EXIT
H=$scratch/home
mcp=$H/.gemini/config/mcp_config.json
settings=$H/.gemini/antigravity-cli/settings.json
mkdir -p "$scratch/bin"
hook() {   # hook <mode|mcp> [VAR=value...]: the hook as the sandbox runs it
  what=$1
  shift
  env -i PATH="$scratch/bin:/usr/bin:/bin" AISBX_AGY_HOME="$H" MCP_GATEWAY_URL=http://gw/mcp "$@" \
    sh "$script" "$what" > "$scratch/out" 2>&1
}

hook mcp MCP_SENTINEL_TOKEN_NAME=proxy-managed
check "mcp: exits 0" same "$?" 0
check "mcp: the gateway in the user MCP config" same \
  "$(jq -c '.mcpServers["mcp-gateway"]' "$mcp")" '{"serverUrl":"http://gw/mcp","headers":{"Authorization":"Bearer proxy-managed"}}'
jq '.mcpServers.mine = {"serverUrl": "http://x"}' "$mcp" > "$scratch/t" && cat "$scratch/t" > "$mcp"
hook mcp
check "mcp: a server of its own stays" same "$(jq -r '.mcpServers.mine.serverUrl' "$mcp")" http://x

hook mode SBX_CRED_GOOGLE_MODE=apikey
check "mode: an API key selects Gemini" same "$(jq -r .modelProvider "$settings")" gemini
hook mode
check "mode: a sign-in removes it" same "$(jq 'has("modelProvider")' "$settings")" false

printf 'not json\n' > "$mcp"
hook mcp
check "mcp: a file that is not JSON exits 0" same "$?" 0
check "mcp: and is left as it is" same "$(cat "$mcp")" "not json"
check "mcp: and says so" grep -q "is not valid JSON - left unchanged" "$scratch/out"

printf '{"mcpServers":{}}\n' > "$mcp"
printf '#!/bin/sh\nkill -9 $$\n' > "$scratch/bin/jq"
chmod +x "$scratch/bin/jq"
hook mcp
check "mcp: a jq killed by a signal exits 0" same "$?" 0
check "mcp: and leaves the file as it is" same "$(cat "$mcp")" '{"mcpServers":{}}'
check "mcp: and names the signal, not invalid JSON" sh -c \
  'grep -q "jq was stopped by signal 9" "$1" && ! grep -q "not valid JSON" "$1"' sh "$scratch/out"
rm "$scratch/bin/jq"

if [ "$failures" -gt 0 ]; then
  echo "$failures check(s) failed"
  exit 1
fi
echo "all checks passed"
exit 0
