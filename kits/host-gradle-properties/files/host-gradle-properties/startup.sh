#!/bin/sh
# Startup hook (agent) of host-gradle-properties: copies the staged host gradle.properties to
# ~/.gradle/gradle.properties. Idempotent. Never fails the boot, since the startup dispatcher
# stops on the first error. Prints no file content.
# Test seam (tests/kit-hooks/host-gradle-properties-test.sh), never set in a sandbox:
# HOST_GRADLE_PROPERTIES_MOUNTINFO.

mountinfo=${HOST_GRADLE_PROPERTIES_MOUNTINFO:-/proc/self/mountinfo}
dir=$HOME/.gradle
target=$dir/gradle.properties
sumfile=$dir/.aisbx-gradle-properties.sum
newline='
'

[ -r "$mountinfo" ] || exit 0
[ -L "$dir" ] && exit 0
if [ -L "$target" ] || { [ -e "$target" ] && [ ! -f "$target" ]; }; then
  exit 0
fi

sum_of() { cksum < "$1" 2>/dev/null | cut -d ' ' -f 1,2; }

decode() {
  rest=$1
  decoded=
  while :; do
    case $rest in
      *'\'[0-7][0-7][0-7]*) ;;
      *) break ;;
    esac
    head=${rest%%'\'[0-7][0-7][0-7]*}
    tail=${rest#"$head"}
    oct=${tail#?}
    oct=${oct%"${oct#???}"}
    rest=${tail#????}
    case $oct in
      000) ch= ;;
      012) ch=$newline ;;
      *) ch=$(printf "\\$oct") ;;
    esac
    decoded=$decoded$head$ch
  done
  decoded=$decoded$rest
}

staged=""
while read -r _ _ _ _ point _; do
  case "${point##*/}" in
    aisbx-gradle-properties)
      decode "$point"
      if [ -f "$decoded/.aisbx-staging" ] && [ -f "$decoded/gradle.properties" ] \
        && [ ! -L "$decoded/gradle.properties" ]; then
        staged=$decoded/gradle.properties
        break
      fi
      ;;
  esac
done < "$mountinfo"

untouched() {
  [ ! -e "$target" ] && return 0
  [ -f "$sumfile" ] && [ "$(sum_of "$target")" = "$(cat "$sumfile" 2>/dev/null)" ]
}

publish() {
  if [ -d "$2" ] && [ ! -L "$2" ]; then
    rm -f "$1"
    return 1
  fi
  mv -f "$1" "$2" 2>/dev/null || { rm -f "$1"; return 1; }
}

if [ -n "$staged" ]; then
  if untouched; then
    umask 077
    [ -d "$dir" ] || mkdir -p "$dir" 2>/dev/null
    tmp=$(mktemp "$dir/.gradle.properties.XXXXXX" 2>/dev/null) || exit 0
    if cat "$staged" > "$tmp" 2>/dev/null && chmod 0600 "$tmp"; then
      sum=$(sum_of "$tmp")
      sumtmp=$(mktemp "$dir/.aisbx-gradle-properties.sum.XXXXXX" 2>/dev/null)
      if [ -n "$sumtmp" ] && printf '%s\n' "$sum" > "$sumtmp" && publish "$tmp" "$target"; then
        publish "$sumtmp" "$sumfile"
      else
        rm -f "$tmp" "$sumtmp"
      fi
    else
      rm -f "$tmp"
    fi
  fi
elif [ -e "$target" ] && untouched; then
  rm -f "$target" "$sumfile"
fi

exit 0
