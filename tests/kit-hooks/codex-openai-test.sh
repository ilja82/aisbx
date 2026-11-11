#!/bin/sh
# Tests the codex-openai kit's update hook, files/codex/update.sh, run against a stub codex.
# Needs sh only: no root, no sbx, no network.
#
#   sh tests/kit-hooks/codex-openai-test.sh

root=$(cd "$(dirname "$0")/../.." && pwd) || exit 2
kit=$root/kits/codex-openai
spec=$kit/codex-openai.yaml
update=$kit/files/codex/update.sh
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

check "the update hook runs as the agent" sh -c 'grep -A2 "codex/update.sh" "$1" | grep -q "user: agent"' sh "$spec"
check "the update hook ends in exit 0" sh -c 'grep -vE "^ *#" "$1" | tail -1 | grep -qx "exit 0"' sh "$update"

stubs=$(mktemp -d "${TMPDIR:-/tmp}/codex-update.XXXXXX") || exit 2
trap 'rm -rf "$stubs"' EXIT
mkdir -p "$stubs/bin" "$stubs/home" "$stubs/plain"
printf '#!/bin/sh\necho "$*" >> "%s"\nexit "${STUB_EXIT:-0}"\n' "$stubs/log" > "$stubs/bin/codex"
chmod +x "$stubs/bin/codex"
hook() {   # hook [VAR=value...]: the update hook as the sandbox runs it
  env -i PATH="$stubs/bin:/usr/bin:/bin" AISBX_CODEX_HOME="$stubs/home" "$@" sh "$update" > "$stubs/out" 2>&1
}

hook
check "update: exits 0 and runs codex update" sh -c '[ "$1" = 0 ] && grep -qx update "$2"' sh "$?" "$stubs/log"
hook STUB_EXIT=3
check "update: a failed update never fails the create" same "$?" 0
check "update: and says so" grep -q "could not update Codex" "$stubs/out"
if command -v timeout >/dev/null 2>&1; then
  printf '#!/bin/sh\nexec sleep 30\n' > "$stubs/bin/codex"
  started=$(date +%s)
  hook AISBX_CODEX_UPDATE_TIMEOUT=1
  status=$?
  took=$(($(date +%s) - started))
  check "update: a hanging update never fails the create" same "$status" 0
  check "update: and ends at the time limit" test "$took" -lt 10
  check "update: and says so" grep -q "codex update took longer than 1s" "$stubs/out"
  printf '#!/bin/sh\necho "$*" >> "%s"\n' "$stubs/log" > "$stubs/bin/codex"
  for tool in sh sleep; do ln -sf "$(command -v "$tool")" "$stubs/plain/$tool"; done
  : > "$stubs/log"
  env -i PATH="$stubs/bin:$stubs/plain" AISBX_CODEX_HOME="$stubs/home" "$stubs/plain/sh" "$update" \
    > "$stubs/out" 2>&1
  check "update: without timeout, still updates" sh -c '[ "$1" = 0 ] && grep -qx update "$2"' sh "$?" "$stubs/log"
else
  echo "skip update: no timeout on this host, the time limit is not checked"
fi

if [ "$failures" -gt 0 ]; then
  echo "$failures check(s) failed"
  exit 1
fi
echo "all checks passed"
exit 0
