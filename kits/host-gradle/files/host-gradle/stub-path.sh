#!/bin/sh
# Startup hook (agent) of host-gradle: puts the image's small helper command on the agent's PATH, unless another
# command of that name is on it.
# Idempotent. Never fails the boot, since the startup dispatcher stops on the first error.
# Test seams (tests/kit-hooks/host-jvm-caches-test.sh), never set in a sandbox: HOST_GRADLE_STUB,
# HOST_GRADLE_SYSTEM_BIN, HOST_GRADLE_ENV_FILE, HOST_GRADLE_BASH_PATH, HOST_GRADLE_BASH.

stub=${HOST_GRADLE_STUB:-/usr/local/lib/aisbx/bin/gcloud}
system_bin=${HOST_GRADLE_SYSTEM_BIN:-/usr/local/bin}
user_bin=$HOME/.local/bin
envfile=${HOST_GRADLE_ENV_FILE:-/etc/sandbox-persistent.sh}
bash=${HOST_GRADLE_BASH:-bash}
name=${stub##*/}

[ -x "$stub" ] || exit 0

ours() {
  [ -L "$1" ] && [ "$(readlink "$1")" = "$stub" ]
}

line_for() {
  printf 'command -v %s >/dev/null 2>&1 || export PATH="%s:$PATH"' "$name" "$1"
}

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
  ours "$link"
}

path_of() {
  command -v "$bash" >/dev/null 2>&1 || return 0
  rc_env=$1
  shift
  if command -v timeout >/dev/null 2>&1; then
    set -- timeout -s KILL 10 "$bash" "$@"
  else
    set -- "$bash" "$@"
  fi
  env BASH_ENV="$rc_env" "$@" 'printf "\n@aisbx-path@%s\n" "$PATH"' 2>/dev/null </dev/null \
    | sed -n 's/^@aisbx-path@//p' | tail -n 1
}

bash_path=${HOST_GRADLE_BASH_PATH:-$(path_of "$envfile" -c)}
if [ -n "${HOST_GRADLE_BASH_PATH:-}" ]; then
  seen=$bash_path
else
  seen=$bash_path:$(path_of "" -lic):$(path_of "" -ic):$PATH
fi
other=
oldifs=$IFS
IFS=:
for d in $seen; do
  if [ -n "$d" ] && [ -x "$d/$name" ] && [ ! -d "$d/$name" ] && ! ours "$d/$name" && [ "$d/$name" != "$stub" ]; then
    other=$d
    break
  fi
done
IFS=$oldifs

if [ -n "$other" ]; then
  for dir in "$system_bin" "$user_bin"; do
    if ours "$dir/$name"; then
      rm -f "$dir/$name"
    fi
  done
  if [ -f "$envfile" ]; then
    a=$(line_for "$system_bin")
    b=$(line_for "$user_bin")
    if grep -qxF -e "$a" -e "$b" "$envfile" 2>/dev/null && tmp=$(mktemp 2>/dev/null); then
      grep -vxF -e "$a" -e "$b" "$envfile" > "$tmp" 2>/dev/null
      cat "$tmp" > "$envfile" 2>/dev/null
      rm -f "$tmp"
    fi
  fi
  exit 0
fi

if link_in "$system_bin"; then
  chosen=$system_bin
elif link_in "$user_bin"; then
  chosen=$user_bin
else
  exit 0
fi

first=
IFS=:
for d in $bash_path; do
  if [ -n "$d" ] && ours "$d/$name"; then
    first=$d
    break
  fi
done
IFS=$oldifs
if [ "$first" != "$chosen" ] || [ "$chosen" = "$user_bin" ]; then
  case $chosen in
    *[\"\$\`\\]*) exit 0 ;;
  esac
  line=$(line_for "$chosen")
  if ! grep -qxF "$line" "$envfile" 2>/dev/null; then
    { [ -s "$envfile" ] && [ -n "$(tail -c 1 "$envfile")" ] && printf '\n'; printf '%s\n' "$line"; } \
      >> "$envfile" 2>/dev/null
  fi
fi

exit 0
