#!/bin/sh
# Startup hook (agent) of host-gradle: puts the image's small helper command on the agent's PATH.
# Idempotent. Never fails the boot, since the startup dispatcher stops on the first error.
# Test seams (tests/kit-hooks/host-jvm-caches-test.sh), never set in a sandbox: HOST_GRADLE_STUB,
# HOST_GRADLE_SYSTEM_BIN, HOST_GRADLE_ENV_FILE, HOST_GRADLE_BASH_PATH.

stub=${HOST_GRADLE_STUB:-/usr/local/lib/aisbx/bin/gcloud}
system_bin=${HOST_GRADLE_SYSTEM_BIN:-/usr/local/bin}
user_bin=$HOME/.local/bin
envfile=${HOST_GRADLE_ENV_FILE:-/etc/sandbox-persistent.sh}
name=${stub##*/}

[ -x "$stub" ] || exit 0

link_in() {
  dir=$1
  [ -d "$dir" ] || { [ "$dir" = "$user_bin" ] && mkdir -p "$dir" 2>/dev/null; }
  [ -d "$dir" ] && [ -w "$dir" ] || return 1
  link=$dir/$name
  if [ -L "$link" ]; then
    :
  elif [ -e "$link" ]; then
    return 1
  else
    ln -s "$stub" "$link" 2>/dev/null
  fi
  [ -L "$link" ] && [ "$(readlink "$link")" = "$stub" ]
}

if link_in "$system_bin"; then
  chosen=$system_bin
elif link_in "$user_bin"; then
  chosen=$user_bin
else
  exit 0
fi

bash_path=${HOST_GRADLE_BASH_PATH:-$(BASH_ENV="$envfile" bash -c 'printf %s "$PATH"' 2>/dev/null)}
first=
oldifs=$IFS
IFS=:
for d in $bash_path; do
  if [ -n "$d" ] && [ -x "$d/$name" ] && [ ! -d "$d/$name" ]; then
    first=$d
    break
  fi
done
IFS=$oldifs
if [ "$first" != "$chosen" ] || [ "$chosen" = "$user_bin" ]; then
  case $chosen in
    *[\"\$\`\\]*) exit 0 ;;
  esac
  line=$(printf 'case "$(command -v %s 2>/dev/null)" in "%s/%s") ;; *) export PATH="%s:$PATH" ;; esac' \
    "$name" "$chosen" "$name" "$chosen")
  if ! grep -qxF "$line" "$envfile" 2>/dev/null; then
    { [ -s "$envfile" ] && [ -n "$(tail -c 1 "$envfile")" ] && printf '\n'; printf '%s\n' "$line"; } \
      >> "$envfile" 2>/dev/null
  fi
fi

exit 0
