#!/bin/sh
# Tests the host-gradle / host-maven hook files/host-jvm-caches/startup.sh, byte-identical in
# kits/host-gradle and kits/host-maven.
#
#   sh tests/kit-hooks/host-jvm-caches-test.sh --static   both kits (descriptor, Dockerfile) and the script
#   sh tests/kit-hooks/host-jvm-caches-test.sh            the static checks, then the hook's cases, for real
#
# Needs no root, no sudo and no sbx. The cases run in scratch folders, with HOME there too, so they
# touch nothing outside.

root=$(cd "$(dirname "$0")/../.." && pwd) || exit 2
rel=files/host-jvm-caches/startup.sh
gradle_spec=$root/kits/host-gradle/host-gradle.yaml
maven_spec=$root/kits/host-maven/host-maven.yaml
gradle_recipe=$root/kits/host-gradle/host-gradle.dockerfile
maven_recipe=$root/kits/host-maven/host-maven.dockerfile
hook=$root/kits/host-gradle/$rel
stub=$root/kits/host-gradle/files/bin/gcloud
stub_hook=$root/kits/host-gradle/files/host-gradle/stub-path.sh

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
not() { ! "$@"; }
same() { [ "$1" = "$2" ]; }
quiet() { "$@" 2>/dev/null; }

# --- the text check ------------------------------------------------------------------------------

tool_name=$(basename "$stub")

allowed_tokens() {   # allowed_tokens <path under the root>: the stub path forms that one file may hold
  case $1 in
    tests/*.sh)
      printf '%s' "kits/host-gradle/files/bin/$tool_name|\\\$root/kits/host-gradle/files/bin/$tool_name|/usr/local/lib/aisbx/bin/$tool_name" ;;
    kits/host-gradle/files/host-gradle/stub-path.sh)
      printf '%s' "/usr/local/lib/aisbx/bin/$tool_name" ;;
    kits/host-gradle/host-gradle.dockerfile)
      printf '%s' "files/bin/$tool_name|/usr/local/lib/aisbx/bin/$tool_name" ;;
  esac
}

text_clean() {   # text_clean <root>: no file under it names what the helper is for, bar its own path in its own files
  scan_root=$1
  forbidden="(^|[^a-z])$(printf 'ga%s' r)([^a-z]|$)|$tool_name|$(printf 'artifact%sregistry' ' ?')"
  listing=$(mktemp) || return 1
  if [ -e "$scan_root/.git" ] && command -v git >/dev/null 2>&1; then
    git -C "$scan_root" -c core.quotePath=false ls-files -co --exclude-standard > "$listing.git" || {
      rm -f "$listing" "$listing.git"
      return 1
    }
    while IFS= read -r rel; do printf '%s/%s\n' "$scan_root" "$rel"; done < "$listing.git" > "$listing"
    rm -f "$listing.git"
  else
    find "$scan_root" \( -name target -o -name .git -o -name node_modules \) -prune -o -type f -print > "$listing"
  fi
  hits=0
  while IFS= read -r file; do
    [ -f "$file" ] || continue
    tokens=$(allowed_tokens "${file#"$scan_root"/}")
    if [ -n "$tokens" ]; then
      lead="[[:space:]\"']|^[[:space:]]*[A-Za-z_][A-Za-z_0-9]*=|\\\$\\{[A-Za-z_][A-Za-z_0-9]*:-"
      strip="s#(^|$lead)($tokens)(\$|[[:space:]\"')}])#\\1\\3#g"
      case $file in *.dockerfile) strip="/^COPY /{$strip;}" ;; esac
    else
      strip="s#^\$##"
    fi
    if LC_ALL=C sed -E -e "$strip" -e "$strip" "$file" | LC_ALL=C grep -aiqE "$forbidden"; then
      printf 'named in %s\n' "$file" >&2
      hits=$((hits + 1))
    fi
  done < "$listing"
  rm -f "$listing"
  [ "$hits" -eq 0 ]
}

# --- static --------------------------------------------------------------------------------------

startup_entries() {   # startup_entries <descriptor> <n>: exactly n startup entries, all as the agent
  block=$(sed -n '/^      startup:$/,/^  - /p' "$1")
  [ "$(printf '%s\n' "$block" | grep -c '^        - command:')" -eq "$2" ] \
    && [ "$(printf '%s\n' "$block" | grep -c '^          user: agent$')" -eq "$2" ]
}
says() { sed -n '/type: com.docker.sandbox\/agent-context@1/,$p' "$1" | grep -q "$2"; }
code() { grep -vE '^ *#' "$1"; }   # a file without its comment lines

static_kit() {   # static_kit <kit> <tool> <descriptor> <dockerfile> <startup entries>
  kit=$1 tool=$2 spec=$3 recipe=$4 entries=$5
  check "$kit: kit-spec v3" grep -qx 'schemaVersion: "3"' "$spec"
  check "$kit: a mixin" grep -qx 'kind: mixin' "$spec"
  check "$kit: has a displayName" grep -q '^displayName: .' "$spec"
  check "$kit: has a description" grep -q '^description: ' "$spec"
  check "$kit: the marker points at kits/statusline/statusline.dockerfile" \
    grep -q 'See kits/statusline/statusline.dockerfile for why' "$recipe"
  check "$kit: its image is the hook and the ENV (FROM scratch)" grep -qx 'FROM scratch' "$recipe"
  check "$kit: its image puts files/ at /usr/local/lib/aisbx/" \
    grep -qx 'COPY --chmod=0755 files/ /usr/local/lib/aisbx/' "$recipe"
  check "$kit: no network rules" not sh -c 'grep -vE "^ *#" "$1" | grep -q network-policy' sh "$spec"
  check "$kit: no volumes" not sh -c 'grep -vE "^ *#" "$1" | grep -q volume@1' sh "$spec"
  check "$kit: no install hooks" not grep -q '^      install:' "$spec"
  check "$kit: exactly the $entries expected startup entries, as the agent" startup_entries "$spec" "$entries"
  check "$kit: the startup entry runs the hook for $tool" grep -qxF \
    "        - command: [\"sh\", \"/usr/local/lib/aisbx/host-jvm-caches/startup.sh\", \"$tool\"]" "$spec"
  check "$kit: has agent instructions" grep -q 'type: com.docker.sandbox/agent-context@1' "$spec"
  check "$kit: instructions: read-only" says "$spec" 'read-only'
  check "$kit: instructions: never write under the mounts" says "$spec" 'Never write'
  check "$kit: ships the hook" test -f "$root/kits/$kit/$rel"
}

static() {
  static_kit host-gradle gradle "$gradle_spec" "$gradle_recipe" 2
  static_kit host-maven maven "$maven_spec" "$maven_recipe" 1
  check "host-gradle: marker SBX_KIT_HOST_GRADLE" grep -qx 'ENV SBX_KIT_HOST_GRADLE=1' "$gradle_recipe"
  check "host-gradle: GRADLE_RO_DEP_CACHE" grep -qx \
    'ENV GRADLE_RO_DEP_CACHE=/home/agent/.host-caches/gradle-caches' "$gradle_recipe"
  check "host-gradle: no MAVEN_OPTS" not sh -c 'grep -vE "^ *#" "$1" | grep -q MAVEN_OPTS' sh "$gradle_recipe"
  check "host-gradle: no old marker" not grep -q 'SBX_KIT_HOST_CACHES' "$gradle_recipe"
  check "host-maven: marker SBX_KIT_HOST_MAVEN" grep -qx 'ENV SBX_KIT_HOST_MAVEN=1' "$maven_recipe"
  check "host-maven: MAVEN_OPTS with the chained tail" grep -qxF \
    'ENV MAVEN_OPTS="-Dmaven.repo.local.tail=/home/agent/.host-caches/m2-repository -Dmaven.repo.local.tail.ignoreAvailability=true"' \
    "$maven_recipe"
  check "host-maven: no GRADLE_RO_DEP_CACHE" not sh -c 'grep -vE "^ *#" "$1" | grep -q GRADLE_RO_DEP_CACHE' sh "$maven_recipe"
  check "host-maven: no old marker" not grep -q 'SBX_KIT_HOST_CACHES' "$maven_recipe"
  check "the two hooks are byte-identical" cmp -s "$root/kits/host-gradle/$rel" "$root/kits/host-maven/$rel"
  check "the stub parses (sh -n)" sh -n "$stub"
  expected_stub=$(mktemp) || { echo "BLOCKED: no scratch file"; exit 127; }
  printf '#!/bin/sh\necho "dummy-token"\nexit 0\n' > "$expected_stub"
  check "the stub is exactly three lines, with its final newline" cmp -s "$stub" "$expected_stub"
  rm -f "$expected_stub"
  check "the stub is executable (0755)" same "$(stat -c %a "$stub" 2>/dev/null || stat -f %Lp "$stub")" 755
  check "the second startup entry runs the stub hook" grep -qxF \
    '        - command: ["sh", "/usr/local/lib/aisbx/host-gradle/stub-path.sh"]' "$gradle_spec"
  check "the stub hook parses (sh -n)" sh -n "$stub_hook"
  check "the stub hook has no set -e" not grep -Eq '^[[:space:]]*set -[a-z]*e' "$stub_hook"
  check "the stub hook ends in exit 0" same "$(tail -n 1 "$stub_hook")" "exit 0"
  check "host-gradle ships changes.txt of one short line" sh -c \
    '[ "$(wc -l < "$1")" -eq 1 ] && [ "$(wc -c < "$1")" -le 121 ]' sh "$root/kits/host-gradle/changes.txt"
  check "no public file names the helper's purpose (its own path is the one exception)" text_clean "$root"
  fixtures=$(mktemp -d) || { echo "BLOCKED: no scratch folder"; exit 127; }
  mkdir -p "$fixtures/core/src" "$fixtures/tests" "$fixtures/kits/host-gradle/files/host-gradle" "$fixtures/kits/x"
  printf 'stub=$root/kits/host-gradle/files/bin/%s\nstub=${S:-/usr/local/lib/aisbx/bin/%s}\n' "$tool_name" "$tool_name" \
    > "$fixtures/tests/ok.sh"
  check "the text check passes a test with only the stub's two paths" text_clean "$fixtures"
  printf 'COPY --chmod=0755 files/bin/%s /usr/local/lib/aisbx/bin/%s\n' "$tool_name" "$tool_name" \
    > "$fixtures/kits/host-gradle/host-gradle.dockerfile"
  check "…and the Dockerfile's COPY of the stub" text_clean "$fixtures"
  printf 'stub=${HOST_GRADLE_STUB:-/usr/local/lib/aisbx/bin/%s}\n' "$tool_name" \
    > "$fixtures/kits/host-gradle/files/host-gradle/stub-path.sh"
  check "…and the hook's default stub path" text_clean "$fixtures"
  printf 'let p = "/x/y=/usr/local/lib/aisbx/bin/%s";\n' "$tool_name" > "$fixtures/core/src/a.rs"
  check "…and fails on the stub path after an equals sign inside another path" not quiet text_clean "$fixtures"
  rm "$fixtures/core/src/a.rs"
  printf '# copies /usr/local/lib/aisbx/bin/%s\n' "$tool_name" > "$fixtures/kits/host-gradle/host-gradle.dockerfile"
  check "…and fails on the stub path in a Dockerfile comment" not quiet text_clean "$fixtures"
  printf 'COPY --chmod=0755 files/bin/%s /usr/local/lib/aisbx/bin/%s\n' "$tool_name" "$tool_name" \
    > "$fixtures/kits/host-gradle/host-gradle.dockerfile"
  for bad in "let s = \"$tool_name auth\";" "let p = \"/opt/sdk/bin/$tool_name\";" \
    "let p = \"/usr/local/lib/aisbx/bin/$tool_name\";"; do
    printf '%s\n' "$bad" > "$fixtures/core/src/a.rs"
    check "…and fails on a Rust line: $(printf '%s' "$bad" | sed "s/$tool_name/<stub>/")" not quiet text_clean "$fixtures"
  done
  rm "$fixtures/core/src/a.rs"
  for bad in 'cp "$stub" "$S/%s"' 'ln -s x "$H/.local/bin/%s"' 'see kits/x/files/bin/%s' \
    'see kits/host-gradle/files/bin/%s.sh' 'run /usr/local/lib/aisbx/bin/%sx' \
    'see other/kits/host-gradle/files/bin/%s' 'see kits/host-gradle/files/bin/%s/more' \
    'run /usr/local/lib/aisbx/bin/%s/more' 'run -/usr/local/lib/aisbx/bin/%s' \
    'run x-kits/host-gradle/files/bin/%s' 'run /opt/usr/local/lib/aisbx/bin/%s' \
    'run files/bin/%s'; do
    printf "$bad\n" "$tool_name" > "$fixtures/tests/bad.sh"
    check "…and fails on a test line: $bad" not quiet text_clean "$fixtures"
  done
  rm "$fixtures/tests/bad.sh"
  for prose in 'run /usr/local/lib/aisbx/bin/%s' 'see kits/host-gradle/files/bin/%s'; do
    printf "$prose\n" "$tool_name" > "$fixtures/README.md"
    check "…and fails on the stub path in prose: $prose" not quiet text_clean "$fixtures"
  done
  rm "$fixtures/README.md"
  printf 'COPY --chmod=0755 files/bin/%s /x\n' "$tool_name" > "$fixtures/kits/x/x.dockerfile"
  check "…and fails on the same COPY in another kit's Dockerfile" not quiet text_clean "$fixtures"
  rm "$fixtures/kits/x/x.dockerfile"
  printf 'COPY --chmod=0755 files/bin/%s-x /x\n' "$tool_name" > "$fixtures/kits/host-gradle/host-gradle.dockerfile"
  check "…and fails on a longer name in the Dockerfile" not quiet text_clean "$fixtures"
  printf 'see kits/host-gradle/files/bin/%s and the %s %s\n' "$tool_name" "Artifact" "Registry" > "$fixtures/tests/ok.sh"
  check "…and fails on a forbidden word beside the allowed path" not quiet text_clean "$fixtures"
  printf 'the %s plugin\n' "$(printf 'GA%s' R)" > "$fixtures/tests/ok.sh"
  check "…and fails on the other short form" not quiet text_clean "$fixtures"
  rm -rf "$fixtures"
  check "the hook parses (sh -n)" sh -n "$hook"
  check "the hook names no network tool" not grep -Eqw 'curl|wget|nc|ssh' "$hook"
  check "the hook has no set -e" not grep -Eq '^[[:space:]]*set -[a-z]*e' "$hook"
  check "the hook ends in exit 0" same "$(tail -n 1 "$hook")" "exit 0"
  check "the default base is /home/agent/.host-caches" grep -qF \
    'base=${HOST_JVM_BASE:-/home/agent/.host-caches}' "$hook"
  check "the default host homes are /Users/*/, /home/*/ and /?/Users/*/" grep -qF \
    'homes=${HOST_JVM_HOMES:-/Users/*/ /home/*/ /?/Users/*/}' "$hook"
}

# --- behaviour -----------------------------------------------------------------------------------

scratch=""
cleanup() { [ -n "$scratch" ] && rm -rf "$scratch"; }
trap cleanup EXIT INT TERM

# fresh <case>: a host home HU under $R, the sandbox home H (base B = H/.host-caches).
fresh() {
  R=$scratch/$1/roots
  HU=$R/Users/u
  H=$scratch/$1/agent
  B=$H/.host-caches
  mkdir -p "$HU" "$H"
}
rc=0
rooted_homes() {   # the hook's own default search list, each pattern under $R
  list=$(sed -n 's/^homes=\${HOST_JVM_HOMES:-\(.*\)}$/\1/p' "$hook")
  out=""
  set -f
  for pattern in $list; do out="$out $R$pattern"; done
  set +f
  printf '%s' "${out# }"
}
hook() {   # hook <tool>: the hook as the startup dispatcher runs it, plus the test seams
  env HOME="$H" HOST_JVM_BASE="$B" HOST_JVM_HOMES="$(rooted_homes)" \
    sh "$hook" "$@" >> "$scratch/hook.log" 2>&1
  rc=$?
}
links_to() { [ -L "$1" ] && [ "$(readlink -- "$1")" = "$2" ]; }
real_dir() { [ -d "$1" ] && [ ! -L "$1" ]; }
absent() { [ ! -e "$1" ] && [ ! -L "$1" ]; }

# A host Gradle dist with its .ok marker and unpacked home; a Maven dist with its unpacked home.
host_gradle() {
  mkdir -p "$HU/.gradle/caches/modules-2" "$HU/.gradle/wrapper/dists/gradle-8.10-bin/h1/gradle-8.10"
  : > "$HU/.gradle/wrapper/dists/gradle-8.10-bin/h1/gradle-8.10-bin.zip.ok"
}
host_maven() {
  mkdir -p "$HU/.m2/repository/org" "$HU/.m2/wrapper/dists/apache-maven-3.9.9-bin/m1/apache-maven-3.9.9"
}

behaviour() {
  scratch=$(mktemp -d) || { echo "BLOCKED: no scratch folder"; exit 127; }
  : > "$scratch/hook.log"

  echo "# 1. gradle wires and seeds only Gradle"
  fresh gradle
  host_gradle
  host_maven
  hook gradle
  check "exit 0" same "$rc" 0
  check "gradle-caches links the host cache" links_to "$B/gradle-caches" "$HU/.gradle/caches"
  check "gradle-wrapper-dists links the host dists" links_to "$B/gradle-wrapper-dists" "$HU/.gradle/wrapper/dists"
  d=$H/.gradle/wrapper/dists/gradle-8.10-bin/h1
  check "the Gradle dist is seeded as a real hash dir" real_dir "$d"
  check "…linking the unpacked home" links_to "$d/gradle-8.10" "$B/gradle-wrapper-dists/gradle-8.10-bin/h1/gradle-8.10"
  check "…with its .ok marker" test -f "$d/gradle-8.10-bin.zip.ok"
  check "no m2-repository" absent "$B/m2-repository"
  check "no m2-wrapper-dists" absent "$B/m2-wrapper-dists"
  check "no Maven dist seeded" absent "$H/.m2"
  hook gradle
  check "a second run exits 0" same "$rc" 0
  check "…and changes nothing" links_to "$d/gradle-8.10" "$B/gradle-wrapper-dists/gradle-8.10-bin/h1/gradle-8.10"

  echo "# 2. maven wires and seeds only Maven"
  fresh maven
  host_gradle
  host_maven
  hook maven
  check "exit 0" same "$rc" 0
  check "m2-repository links the host repository" links_to "$B/m2-repository" "$HU/.m2/repository"
  check "m2-wrapper-dists links the host dists" links_to "$B/m2-wrapper-dists" "$HU/.m2/wrapper/dists"
  check "the Maven dist is seeded as a link" links_to \
    "$H/.m2/wrapper/dists/apache-maven-3.9.9-bin/m1" "$B/m2-wrapper-dists/apache-maven-3.9.9-bin/m1"
  check "no gradle-caches" absent "$B/gradle-caches"
  check "no gradle-wrapper-dists" absent "$B/gradle-wrapper-dists"
  check "no Gradle dist seeded" absent "$H/.gradle"
  hook maven
  check "a second run exits 0" same "$rc" 0

  echo "# 3. nothing mounted"
  fresh none
  hook gradle
  check "gradle exits 0" same "$rc" 0
  check "gradle-caches is an empty real dir" real_dir "$B/gradle-caches"
  check "gradle-wrapper-dists is an empty real dir" real_dir "$B/gradle-wrapper-dists"
  hook maven
  check "maven exits 0" same "$rc" 0
  check "m2-repository is an empty real dir" real_dir "$B/m2-repository"
  check "m2-wrapper-dists is an empty real dir" real_dir "$B/m2-wrapper-dists"

  echo "# 4. the host drops a dist and the mount"
  fresh drop
  host_gradle
  host_maven
  hook gradle
  hook maven
  rm -rf "$HU/.gradle/wrapper/dists/gradle-8.10-bin" "$HU/.m2/wrapper/dists/apache-maven-3.9.9-bin"
  hook gradle
  check "gradle exits 0" same "$rc" 0
  check "the dangling Gradle seed is pruned" absent "$H/.gradle/wrapper/dists/gradle-8.10-bin/h1"
  hook maven
  check "maven exits 0" same "$rc" 0
  check "the dangling Maven seed is pruned" absent "$H/.m2/wrapper/dists/apache-maven-3.9.9-bin/m1"
  rm -rf "$HU/.m2"
  hook maven
  check "with the repository gone, m2-repository becomes a real dir" real_dir "$B/m2-repository"

  echo "# 5a. the helper command"
  check "the stub prints dummy-token with any arguments" same "$("$stub" auth print-access-token --x)" "dummy-token"
  "$stub" a b >/dev/null
  check "the stub exits 0" same "$?" 0
  bash_bin=$(command -v bash 2>/dev/null)
  mkdir -p "$scratch/empty1" "$scratch/empty2"
  empty=$scratch/empty1
  fresh stubs
  S=$H/stub-src
  mkdir -p "$S" "$H/sys bin"
  cp "$stub" "$S/$tool_name"
  stub_run() {   # stub_run <system bin dir> <PATH of a non-interactive bash>
    env HOME="$H" HOST_GRADLE_STUB="$S/$tool_name" HOST_GRADLE_SYSTEM_BIN="$1" HOST_GRADLE_BASH_PATH="${2:-$empty}" \
      HOST_GRADLE_ENV_FILE="$H/persistent.sh" sh "$stub_hook" >> "$scratch/hook.log" 2>&1
    rc=$?
  }
  by_name() {   # by_name <PATH> <command>: a fresh non-interactive bash that reads the persistent file
    env -i HOME="$H" PATH="$1" BASH_ENV="$H/persistent.sh" "$bash_bin" -c "$2" </dev/null 2>/dev/null
  }
  other_command() {   # other_command <dir> <word>: a command of the helper's name that prints <word>
    mkdir -p "$1"
    printf '#!/bin/sh\necho %s\n' "$2" > "$1/$tool_name"
    chmod 755 "$1/$tool_name"
  }
  stub_run "$H/sys bin" "$H/sys bin"
  check "exit 0" same "$rc" 0
  check "linked into the system folder, a path with a space" links_to "$H/sys bin/$tool_name" "$S/$tool_name"
  check "…and not the home folder" absent "$H/.local/bin"
  check "…and the link runs" same "$("$H/sys bin/$tool_name" x)" "dummy-token"
  check "a folder on the PATH needs no export" absent "$H/persistent.sh"
  stub_run "$H/sys bin" "$H/sys bin"
  check "a second run exits 0 and keeps the link" links_to "$H/sys bin/$tool_name" "$S/$tool_name"
  other_command "$H/later" later
  stub_run "$H/sys bin" "$H/sys bin:$H/later"
  check "another command later on the PATH than the link: exit 0" same "$rc" 0
  check "…the link goes" absent "$H/sys bin/$tool_name"
  check "…and the other command stays" grep -q later "$H/later/$tool_name"
  fresh stubs2
  mkdir -p "$H/sys"
  printf 'mine\n' > "$H/sys/$tool_name"
  stub_run "$H/sys" "$H/sys"
  check "a foreign file that is no command stays" same "$(cat "$H/sys/$tool_name")" mine
  check "…and the home folder gets the link instead" links_to "$H/.local/bin/$tool_name" "$S/$tool_name"
  rm -rf "$H/.local" "$H/persistent.sh" "$H/sys/$tool_name"
  ln -s /elsewhere "$H/sys/$tool_name"
  stub_run "$H/sys" "$H/sys"
  check "a foreign link stays" same "$(readlink "$H/sys/$tool_name")" /elsewhere
  fresh stubs3
  stub_run "/nonexistent/dir" "$empty"
  check "no writable system folder: the home folder is made" links_to "$H/.local/bin/$tool_name" "$S/$tool_name"
  check "…and one export line goes to the persistent file" same "$(grep -c 'export PATH=' "$H/persistent.sh")" 1
  stub_run "/nonexistent/dir" "$empty"
  check "a second run exits 0" same "$rc" 0
  check "…keeps the link" links_to "$H/.local/bin/$tool_name" "$S/$tool_name"
  check "…and adds no second export line" same "$(grep -c 'export PATH=' "$H/persistent.sh")" 1
  other_command "$H/later" later
  if [ -n "$bash_bin" ]; then
    found=$(by_name "$scratch/empty1:$scratch/empty2" "$tool_name x")
    check "a fresh non-interactive bash runs the helper by name" same "$found" "dummy-token"
    found=$(by_name "$scratch/empty1:$scratch/empty2:$H/.local/bin" 'echo "$PATH"')
    check "…and the export does not repeat a folder already on the PATH" same "$found" "$scratch/empty1:$scratch/empty2:$H/.local/bin"
    found=$(by_name "$scratch/empty1:$H/later" "$tool_name x")
    check "…and a shell that finds another command of that name runs that one" same "$found" "later"
  fi
  stub_run "/nonexistent/dir" "$H/later"
  check "another command on the PATH at a later start: exit 0" same "$rc" 0
  check "…the helper's link goes" absent "$H/.local/bin/$tool_name"
  check "…and its export line goes" same "$(grep -c 'export PATH=' "$H/persistent.sh")" 0
  fresh stubs5
  other_command "$H/foreign" foreign
  stub_run "/nonexistent/dir" "$H/foreign:$H/.local/bin"
  check "another command on the PATH at the first start: exit 0" same "$rc" 0
  check "…it stays" grep -q foreign "$H/foreign/$tool_name"
  check "…no link is made" absent "$H/.local/bin/$tool_name"
  check "…and no export line" absent "$H/persistent.sh"
  if [ -n "$bash_bin" ]; then
    check "…so a name-based call runs it" same "$(by_name "$H/foreign:$H/.local/bin" "$tool_name x")" "foreign"
  fi
  fresh stubs9
  stub_run "/nonexistent/dir" "$S"
  check "the helper's own folder on the PATH is no other command" links_to "$H/.local/bin/$tool_name" "$S/$tool_name"
  fresh stubs7
  mkdir -p "$H/.local/bin"
  stub_run "/nonexistent/dir" "$H/.local/bin"
  check "the home folder first on the startup PATH: exit 0" same "$rc" 0
  check "…still writes the guarded export for later shells" same "$(grep -c 'export PATH=' "$H/persistent.sh")" 1
  if [ -n "$bash_bin" ]; then
    found=$(by_name "$empty" "$tool_name x")
    check "…so a later shell without that folder runs the helper" same "$found" "dummy-token"
  fi
  fresh stubs6
  printf 'export KEEP=1' > "$H/persistent.sh"
  stub_run "/nonexistent/dir" "$empty"
  check "a persistent file without a final newline: exit 0" same "$rc" 0
  check "…keeps its own line whole" same "$(head -n 1 "$H/persistent.sh")" "export KEEP=1"
  check "…and puts the export on a line of its own" same "$(grep -c '^command -v ' "$H/persistent.sh")" 1
  if [ -n "$bash_bin" ]; then
    found=$(by_name "$empty" "echo \$KEEP; $tool_name x")
    check "…so the next non-interactive bash reads it and runs the helper" same "$found" "1
dummy-token"
  fi
  other_command "$H/other" other
  stub_run "/nonexistent/dir" "$H/other"
  check "the line goes again when another command comes: exit 0" same "$rc" 0
  check "…and the file's own line stays" same "$(cat "$H/persistent.sh")" "export KEEP=1"
  fresh stubs4
  mkdir -p "$H/.local"
  : > "$H/.local/bin"
  stub_run "/nonexistent/dir"
  check "nothing writable: exit 0" same "$rc" 0
  check "…and no export line" absent "$H/persistent.sh"

  echo "# 5. an own download is never clobbered, an unknown tool does nothing"
  fresh own
  host_gradle
  mkdir -p "$H/.gradle/wrapper/dists/gradle-8.10-bin/h1/gradle-8.10"
  : > "$H/.gradle/wrapper/dists/gradle-8.10-bin/h1/mine"
  hook gradle
  check "exit 0" same "$rc" 0
  check "the own hash dir stays" test -f "$H/.gradle/wrapper/dists/gradle-8.10-bin/h1/mine"
  check "…and its unpacked home is no link" real_dir "$H/.gradle/wrapper/dists/gradle-8.10-bin/h1/gradle-8.10"
  fresh unknown
  hook
  check "no tool: exit 0" same "$rc" 0
  hook ant
  check "an unknown tool: exit 0" same "$rc" 0
  check "…and nothing made" absent "$B"

  echo "# 6. a Windows host mounts its home at /<drive>/Users/<user>"
  fresh win
  HU=$R/c/Users/u
  mkdir -p "$HU"
  host_gradle
  host_maven
  hook gradle
  check "exit 0" same "$rc" 0
  check "gradle-caches links the Windows host cache" links_to "$B/gradle-caches" "$HU/.gradle/caches"
  check "gradle-wrapper-dists links the Windows host dists" links_to "$B/gradle-wrapper-dists" "$HU/.gradle/wrapper/dists"
  d=$H/.gradle/wrapper/dists/gradle-8.10-bin/h1
  check "the Gradle dist is seeded" links_to "$d/gradle-8.10" "$B/gradle-wrapper-dists/gradle-8.10-bin/h1/gradle-8.10"
  check "…with its .ok marker" test -f "$d/gradle-8.10-bin.zip.ok"
  hook maven
  check "maven exits 0" same "$rc" 0
  check "m2-repository links the Windows host repository" links_to "$B/m2-repository" "$HU/.m2/repository"
  check "the Maven dist is seeded" links_to \
    "$H/.m2/wrapper/dists/apache-maven-3.9.9-bin/m1" "$B/m2-wrapper-dists/apache-maven-3.9.9-bin/m1"
  fresh winlong
  mkdir -p "$R/cc/Users/u/.gradle/caches"
  hook gradle
  check "a two-letter folder is no drive" real_dir "$B/gradle-caches"
}

case ${1:-} in
  --static) static ;;
  "") static; behaviour ;;
  *) echo "usage: sh tests/kit-hooks/host-jvm-caches-test.sh [--static]" >&2; exit 2 ;;
esac
if [ "$failures" -gt 0 ]; then
  [ -n "$scratch" ] && [ -s "$scratch/hook.log" ] && { echo "--- hook output"; cat "$scratch/hook.log"; }
  echo "$failures check(s) failed"
  exit 1
fi
echo "all checks passed"
exit 0
