#!/bin/sh
# skills-store.sh <agent skills folder> - link sbx's shared skills store into the agent's folder:
# one link per skill, or, with the store read-write, the whole folder unless it holds skills of
# its own. While the filter file exists, a read-only store links only the skills it names.
# skills-store.sh --only [SKILL...] -- <agent skills folder>... - write the filter file, then link.
# Byte-identical in every workload that shares the store (tests/kit-hooks/skills-store-test.sh).
# Test seams, never set in a sandbox: AISBX_SKILLS_STORE, AISBX_SKILLS_STORE_MOUNTS,
# AISBX_SKILLS_FILTER.
store=${AISBX_SKILLS_STORE:-/home/agent/.local/share/aisbx/skills-store}
mounts=${AISBX_SKILLS_STORE_MOUNTS:-/proc/mounts}
filter=${AISBX_SKILLS_FILTER:-/home/agent/.config/aisbx/store-skills}

nl='
'
status=0
filtered=no
[ -f "$filter" ] && filtered=file
if [ "${1-}" = --only ]; then
  shift
  names=$nl
  while [ $# -gt 0 ] && [ "$1" != -- ]; do
    names=$names$1$nl
    shift
  done
  [ "${1-}" = -- ] && shift
  filtered=args
  mkdir -p -- "$(dirname -- "$filter")" 2>/dev/null
  tmp=$filter.tmp.$$
  if printf '# The store skills aisbx links into this sandbox, one per line.%s' "$names" > "$tmp" 2>/dev/null &&
    mv -f -- "$tmp" "$filter" 2>/dev/null; then
    :
  else
    rm -f -- "$tmp"
    echo "aisbx: could not write $filter - the next start links by the last choice" >&2
    status=1
  fi
fi
if [ $# -eq 0 ]; then
  echo "usage: skills-store.sh [--only [SKILL...] --] <agent skills folder>..." >&2
  exit 2
fi

# The store's mount options, or nothing when it is not a mount point.
opts=$(awk -v p="$store" '$2 == p { o = $4 } END { print o }' "$mounts" 2>/dev/null)
case ",$opts," in
  ,,) mode=off ;;
  *,rw,*) mode=rw ;;
  *) mode=ro ;;
esac

ours() { # ours <path>: a link into $store
  [ -L "$1" ] || return 1
  case $(readlink -- "$1") in "$store" | "$store"/*) return 0 ;; esac
  return 1
}

only_ours() { # only_ours <folder>: holds nothing but our links
  for e in "$1"/* "$1"/.[!.]* "$1"/..?*; do
    [ -e "$e" ] || [ -L "$e" ] || continue
    ours "$e" || return 1
  done
  return 0
}

wanted() { # wanted <skill>: linked here - every skill, but a read-only store only the named ones
  [ "$mode" = ro ] || return 0
  case $filtered in
    args)
      case $names in *"$nl$1$nl"*) return 0 ;; esac
      return 1
      ;;
    file) grep -qxF -- "$1" "$filter" 2>/dev/null ;;
    *) return 0 ;;
  esac
}

link() { # link <agent skills folder>
  dest=$1
  if [ "$mode" = rw ]; then
    [ -L "$dest" ] && [ "$(readlink -- "$dest")" = "$store" ] && return 0
    if { [ ! -e "$dest" ] && [ ! -L "$dest" ]; } || { [ -d "$dest" ] && [ ! -L "$dest" ] && only_ours "$dest"; }; then
      rm -f -- "$dest"/* "$dest"/.[!.]* "$dest"/..?* 2>/dev/null
      rmdir -- "$dest" 2>/dev/null
      mkdir -p -- "$(dirname -- "$dest")" 2>/dev/null
      ln -s -- "$store" "$dest" 2>/dev/null && return 0
    fi
    echo "aisbx: $dest holds skills of its own - linking the store's skills into it instead" >&2
  fi

  # Read-only, off, or a taken folder: a real folder with one link per store skill.
  ours "$dest" && rm -f -- "$dest"
  mkdir -p -- "$dest" 2>/dev/null || return 0
  if [ "$mode" != off ]; then
    for s in "$store"/*/; do
      [ -f "${s}SKILL.md" ] || continue
      name=$(basename -- "$s")
      wanted "$name" || continue
      t=$dest/$name
      if [ -e "$t" ] || [ -L "$t" ]; then
        ours "$t" || continue # the agent's own skill of that name stays
      fi
      ln -sfn -- "$store/$name" "$t" 2>/dev/null
    done
  fi
  # Our links that no longer resolve or are not wanted go - and all of them while the store is
  # not mounted.
  for t in "$dest"/* "$dest"/.[!.]* "$dest"/..?*; do
    ours "$t" || continue
    if [ "$mode" = off ] || [ ! -e "$t" ] || ! wanted "$(basename -- "$t")"; then
      rm -f -- "$t"
    fi
  done
}

for dest in "$@"; do
  link "$dest"
done
[ "$status" -eq 0 ] || exit "$status"
exit 0
