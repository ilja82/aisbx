#!/bin/sh
# Install hook (agent): install the npm range OPENCODE_LITELLM_VERSION names before the first launch.
# `--force`: 1.x's `opencode-ai` and 2.x's `@opencode/cli` both install an `opencode` binary, and npm
# refuses to link one over the other without it. Keep the fallback equal to the Dockerfile's knob.
export HOME=${AISBX_OPENCODE_HOME:-/home/agent}
v=${OPENCODE_LITELLM_VERSION:-2}
limit=${AISBX_OPENCODE_UPDATE_TIMEOUT:-180}
set -- npm install -g --force "@opencode/cli@$v"
if command -v timeout >/dev/null 2>&1; then
  timeout -k 10 "$limit" "$@" >/dev/null 2>&1
else
  "$@" >/dev/null 2>&1
fi
case $? in
  0) ;;
  124 | 137) echo "aisbx: the OpenCode $v update took longer than ${limit}s - keeping the image's version" ;;
  *) echo "aisbx: could not update OpenCode $v - keeping the image's version" ;;
esac
exit 0
