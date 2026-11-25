#!/bin/sh
# Install hook (agent), after the workload's seed: point ~/.claude/settings.json at this kit's bar
# script and its mod for the subagent rows. Merged, never written over: the workload, rtk and
# headroom write keys there too.
f=${AISBX_STATUSLINE_SETTINGS:-/home/agent/.claude/settings.json}
lib=/usr/local/lib/aisbx/statusline
mkdir -p "$(dirname "$f")"
if ! command -v jq >/dev/null 2>&1; then
  echo "statusline: jq not found - leaving $f alone" >&2
  exit 0
fi
[ -s "$f" ] || printf '{}\n' > "$f"
tmp=$(mktemp) || exit 0
if jq --arg b "$lib/statusline.sh" --arg s "$lib/subagent-statusline.sh" --arg m "$lib/mod" \
  --arg i "${CLAUDE_CODE_PLUGIN_DIRS:-}" '
  .statusLine = {type: "command", command: $b}
  | if (.subagentStatusLine | type) == "object" and .subagentStatusLine.command == $s
    then del(.subagentStatusLine) else . end
  | .env = ((.env // {}) | .CLAUDE_CODE_PLUGIN_DIRS = reduce (($i | split(":"))[], $m) as $d
      (.CLAUDE_CODE_PLUGIN_DIRS // "";
        if $d == "" or (split(":") | index([$d])) then . elif . == "" then $d else . + ":" + $d end))
' "$f" > "$tmp" 2>/dev/null; then
  cat "$tmp" > "$f"
else
  echo "statusline: $f is not valid JSON - left unchanged" >&2
fi
rm -f "$tmp"
exit 0
