#!/bin/sh
# Last install hook (agent): update Claude Code before the first launch, since the cached image can
# hold an old release. Byte-identical in every Claude Code workload.
home=${AISBX_CLAUDE_HOME:-/home/agent}
export HOME="$home" PATH="$home/.local/bin:$PATH"
command -v claude >/dev/null 2>&1 || { echo "aisbx: no claude binary to update"; exit 0; }
limit=${AISBX_CLAUDE_UPDATE_TIMEOUT:-180}
if command -v timeout >/dev/null 2>&1; then
  timeout -k 10 "$limit" claude update
else
  claude update
fi
case $? in
  0) ;;
  124 | 137) echo "aisbx: claude update took longer than ${limit}s - keeping the image's version" ;;
  *) echo "aisbx: could not update Claude Code - keeping the image's version" ;;
esac
exit 0
