#!/bin/sh
# Tests the host-gradle-properties hook files/host-gradle-properties/startup.sh.
#
#   sh tests/kit-hooks/host-gradle-properties-test.sh --static   the descriptor, the Dockerfile and the script
#   sh tests/kit-hooks/host-gradle-properties-test.sh            the hook's cases, for real
#
# Needs no root, no sudo and no sbx: a fake mountinfo, scratch folders, HOME there too.

root=$(cd "$(dirname "$0")/../.." && pwd) || exit 2
kit=$root/kits/host-gradle-properties
spec=$kit/host-gradle-properties.yaml
recipe=$kit/host-gradle-properties.dockerfile
hook=$kit/files/host-gradle-properties/startup.sh

failures=0
check() {
  what=$1
  shift
  if "$@"; then
    printf 'ok   %s\n' "$what"
  else
    printf 'FAIL %s\n' "$what"
    failures=$((failures + 1))
  fi
}
not() { ! "$@"; }
same() { [ "$1" = "$2" ]; }
mode_of() { stat -c %a "$1" 2>/dev/null || stat -f %Lp "$1"; }

static() {
  check "kit-spec v3" grep -qx 'schemaVersion: "3"' "$spec"
  check "a mixin" grep -qx 'kind: mixin' "$spec"
  check "the name warns about secrets" grep -qxF 'displayName: Gradle properties file (may contain secrets!)' "$spec"
  check "the description warns about secrets" grep -q 'Beware: This file may contain secrets!' "$spec"
  check "the marker" grep -qx 'ENV SBX_KIT_HOST_GRADLE_PROPERTIES=1' "$recipe"
  check "its image is the hook (FROM scratch)" grep -qx 'FROM scratch' "$recipe"
  check "its image puts files/ at /usr/local/lib/aisbx/" \
    grep -qx 'COPY --chmod=0755 files/ /usr/local/lib/aisbx/' "$recipe"
  check "one startup entry, as the agent" sh -c \
    'b=$(sed -n "/^      startup:\$/,/^  - /p" "$1"); [ "$(printf "%s\n" "$b" | grep -c "^        - command:")" -eq 1 ] && printf "%s\n" "$b" | grep -qx "          user: agent"' sh "$spec"
  check "the entry runs the hook" grep -qxF \
    '        - command: ["sh", "/usr/local/lib/aisbx/host-gradle-properties/startup.sh"]' "$spec"
  check "no network rules" not sh -c 'grep -vE "^ *#" "$1" | grep -q network-policy' sh "$spec"
  check "no volumes" not sh -c 'grep -vE "^ *#" "$1" | grep -q volume@1' sh "$spec"
  check "has agent instructions" grep -q 'type: com.docker.sandbox/agent-context@1' "$spec"
  check "the hook parses (sh -n)" sh -n "$hook"
  check "the hook has no set -e" not grep -Eq '^[[:space:]]*set -[a-z]*e' "$hook"
  check "the hook ends in exit 0" same "$(tail -n 1 "$hook")" "exit 0"
  check "the hook names no network tool" not grep -Eqw 'curl|wget|nc|ssh' "$hook"
  check "the hook writes through mktemp" grep -q 'mktemp' "$hook"
  check "the hook names no fixed temporary file" not grep -q 'gradle.properties.tmp' "$hook"
}

scratch=""
cleanup() { [ -n "$scratch" ] && rm -rf "$scratch"; }
trap cleanup EXIT INT TERM

rc=0
run() {
  env HOME="$H" HOST_GRADLE_PROPERTIES_MOUNTINFO="$M" sh "$hook" > "$scratch/out" 2>&1
  rc=$?
}
mounted() {   # mounted <dir>: a mountinfo line for that mount point, among others
  printf '36 35 98:0 / /proc rw - proc proc rw\n1 2 8:1 / %s ro - ext4 /dev/x ro\n' "$1" > "$M"
}
unmounted() { printf '36 35 98:0 / /proc rw - proc proc rw\n' > "$M"; }

behaviour() {
  scratch=$(mktemp -d) || { echo "BLOCKED: no scratch folder"; exit 127; }
  H=$scratch/home
  M=$scratch/mountinfo
  S=$scratch/Users/u/state/aisbx-gradle-properties
  mkdir -p "$H" "$S"
  : > "$S/.aisbx-staging"
  sample="demo=hunter2-sample"
  target=$H/.gradle/gradle.properties

  echo "# 1. the mount is there"
  printf '%s\n' "$sample" > "$S/gradle.properties"
  mounted "$S"
  run
  check "exit 0" same "$rc" 0
  check "the file is copied" cmp -s "$S/gradle.properties" "$target"
  check "mode 0600" same "$(mode_of "$target")" 600
  check "no content in the output" not grep -q hunter2 "$scratch/out"
  run
  check "a second run exits 0 and changes nothing" cmp -s "$S/gradle.properties" "$target"

  echo "# 2. the host file changed"
  printf 'other=1\n' > "$S/gradle.properties"
  run
  check "an untouched copy is overwritten" cmp -s "$S/gradle.properties" "$target"

  echo "# 3. the agent edited it"
  printf 'edited=1\n' >> "$target"
  printf 'newer=1\n' > "$S/gradle.properties"
  run
  check "exit 0" same "$rc" 0
  check "the edited file stays" grep -qx 'edited=1' "$target"
  check "…and is not replaced" not cmp -s "$S/gradle.properties" "$target"
  unmounted
  run
  check "with the mount gone, an edited file stays" grep -qx 'edited=1' "$target"

  echo "# 4. the mount is gone"
  rm -rf "$H/.gradle"
  mounted "$S"
  run
  check "copied again" cmp -s "$S/gradle.properties" "$target"
  unmounted
  run
  check "exit 0" same "$rc" 0
  check "an untouched copy is removed" test ! -e "$target"
  check "…with its checksum record" test ! -e "$H/.gradle/.aisbx-gradle-properties.sum"
  mounted "$S"
  rm -f "$S/gradle.properties"
  run
  check "a mount without the file: exit 0" same "$rc" 0
  check "…and no file" test ! -e "$target"

  echo "# 5. a file of the agent's own, no mount"
  mkdir -p "$H/.gradle"
  printf 'mine=1\n' > "$target"
  unmounted
  run
  check "stays" grep -qx 'mine=1' "$target"
  M=$scratch/missing
  run
  check "no mountinfo: exit 0" same "$rc" 0
  check "…and the file stays" grep -qx 'mine=1' "$target"

  echo "# 6. a mount path with escapes"
  rm -rf "$H/.gradle"
  M=$scratch/mountinfo
  E="$scratch/Users/u/my state"
  mkdir -p "$E/aisbx-gradle-properties"
  printf '%s\n' "$sample" > "$E/aisbx-gradle-properties/gradle.properties"
  : > "$E/aisbx-gradle-properties/.aisbx-staging"
  escaped=$(printf '%s' "$E/aisbx-gradle-properties" | sed 's/ /\\040/g')
  printf '36 35 98:0 / /proc rw - proc proc rw\n1 2 8:1 / %s ro - ext4 /dev/x ro\n' "$escaped" > "$M"
  run
  check "a path with a space is decoded" cmp -s "$E/aisbx-gradle-properties/gradle.properties" "$target"
  run
  check "…and a second run keeps the copy" cmp -s "$E/aisbx-gradle-properties/gradle.properties" "$target"

  echo "# 7. fresh private temporary files, no links followed"
  rm -rf "$H/.gradle"
  mkdir -p "$H/.gradle" "$scratch/elsewhere"
  mounted "$S"
  printf 'demo=again\n' > "$S/gradle.properties"
  : > "$scratch/elsewhere/victim"
  ln -s "$scratch/elsewhere/victim" "$H/.gradle/gradle.properties.tmp"
  ln -s "$scratch/elsewhere/victim" "$H/.gradle/.aisbx-gradle-properties.sum"
  printf 'precious\n' > "$H/.gradle/gradle.properties.tmp.keep"
  run
  check "the copy is made" cmp -s "$S/gradle.properties" "$target"
  check "a stale link is not written through" test ! -s "$scratch/elsewhere/victim"
  check "the checksum record is a file of its own" test -f "$H/.gradle/.aisbx-gradle-properties.sum" -a ! -L "$H/.gradle/.aisbx-gradle-properties.sum"
  check "no temporary file is left" same "$(ls -A "$H/.gradle" | grep -c '^\.[a-z.-]*\.[A-Za-z0-9]\{6\}$')" 0
  rm -f "$target" "$H/.gradle/.aisbx-gradle-properties.sum"
  ln -s "$scratch/elsewhere/victim" "$target"
  run
  check "a link in place of the file is left alone" test -L "$target" -a ! -s "$scratch/elsewhere/victim"
  rm -f "$target"
  mkdir "$target"
  run
  check "a folder in place of the file is left alone" test -d "$target" -a ! -e "$target/gradle.properties"
  rmdir "$target"
  rm -rf "$H/.gradle"
  ln -s "$scratch/elsewhere" "$H/.gradle"
  run
  check "a linked ~/.gradle is left alone" test ! -e "$scratch/elsewhere/gradle.properties"
  rm -f "$H/.gradle"

  echo "# 8. a second mount with the same folder name"
  P=$scratch/Users/u/project/aisbx-gradle-properties
  mkdir -p "$P"
  printf 'project=1\n' > "$P/gradle.properties"
  printf '36 35 98:0 / /proc rw - proc proc rw\n1 2 8:1 / %s rw - ext4 /dev/x rw\n1 2 8:1 / %s ro - ext4 /dev/x ro\n' "$P" "$S" > "$M"
  printf 'demo=staged\n' > "$S/gradle.properties"
  run
  check "the staging mount wins over a project folder of the same name" grep -qx 'demo=staged' "$target"
  rm -f "$S/gradle.properties"
  run
  check "with the staged file gone, the project's file is not copied" test ! -e "$target"
}

case ${1:-} in
  --static) static ;;
  "") behaviour ;;
  *) echo "usage: sh tests/kit-hooks/host-gradle-properties-test.sh [--static]" >&2; exit 2 ;;
esac
if [ "$failures" -gt 0 ]; then
  echo "$failures check(s) failed"
  exit 1
fi
echo "all checks passed"
exit 0
