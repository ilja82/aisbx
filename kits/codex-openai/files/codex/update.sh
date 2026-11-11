#!/bin/sh
# Install hook (agent): update Codex before the first launch. `codex update` is an npm install into
# the agent-owned tree that NPM_CONFIG_PREFIX names.
export HOME=${AISBX_CODEX_HOME:-/home/agent}
command -v codex >/dev/null 2>&1 || { echo "aisbx: no codex binary to update"; exit 0; }
limit=${AISBX_CODEX_UPDATE_TIMEOUT:-180}
if command -v timeout >/dev/null 2>&1; then
  timeout -k 10 "$limit" codex update
else
  codex update
fi
case $? in
  0) ;;
  124 | 137) echo "aisbx: codex update took longer than ${limit}s - keeping the image's version" ;;
  *) echo "aisbx: could not update Codex - keeping the image's version" ;;
esac
exit 0
