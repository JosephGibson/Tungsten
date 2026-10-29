#!/usr/bin/env bash
# Facts for the tungsten-next skill, in one call. Read-only and offline: origin/* are
# local refs as of the last fetch. Needs GNU date/stat and /proc (the owner's Linux host).
set -u
cd "$(git rev-parse --show-toplevel)" || exit 1
now=$(date +%s)
# A file written under 3 minutes ago means a session is probably mid-step.
LIVE=180

age() {
  local s=$((now - $1))
  if [ "$s" -lt 3600 ]; then echo "$((s / 60))m ago"
  elif [ "$s" -lt 172800 ]; then echo "$((s / 3600))h ago"
  else echo "$((s / 86400))d ago"; fi
}
ppid_of() { sed 's/^.*) //' "/proc/$1/stat" 2>/dev/null | cut -d' ' -f2; }

echo "== git"
br=$(git branch --show-current)
echo "branch: ${br:-detached} · upstream: $(git rev-parse --abbrev-ref '@{upstream}' 2>/dev/null || echo none)"
echo "version: $(sed -n '/^\[workspace.package\]/,/^\[/s/^version *= *"\(.*\)"/\1/p' Cargo.toml) (Cargo.toml)"
fh=$(git rev-parse --git-path FETCH_HEAD)
fetch=never; [ -e "$fh" ] && fetch=$(age "$(stat -c %Y "$fh")")
tag=$(git describe --tags --abbrev=0 2>/dev/null)
newest=$(git for-each-ref --sort=-v:refname --count=1 --format='%(refname:short)' 'refs/tags/v*')
base=$tag
v=${newest#v}; v=${v%.0}
if [ -n "$newest" ] && [ "$newest" != "$tag" ]; then
  # Releases through 0.108 were squash-merged, so their tags sit off this history: count
  # from the release's squash commit on main ("Update 0.NN: …") instead.
  sq=$(git log --format=%h -1 --grep="^Update ${v//./\\.}:" HEAD)
  base=${sq:-$tag}
fi
if [ -n "$newest" ]; then
  onmain=no
  if git merge-base --is-ancestor "$newest" origin/main 2>/dev/null ||
    git log --format=%s -60 origin/main 2>/dev/null | grep -q "^Update $v:"; then onmain=yes; fi
  echo "newest tag: $newest · history counted from: ${base:-none} · on origin/main: $onmain (last fetch $fetch)"
fi
if [ -n "$base" ]; then
  n=$(git rev-list --count "$base..HEAD")
  echo "commits since $base: $n"
  git --no-pager log --oneline -30 "$base..HEAD" | sed 's/^/  /'
  [ "$n" -gt 30 ] && echo "  +$((n - 30)) older"
fi

echo "== git status --short"
st=$(git status --short)
echo "${st:-clean}"
recent=$( { git diff --name-only -z HEAD; git ls-files -o --exclude-standard -z; } |
  xargs -0 -r stat -c '%Y %n' 2>/dev/null | awk -v t=$((now - LIVE)) '$1 > t { print substr($0, index($0, " ") + 1) }' | head -5 | tr '\n' ' ')
echo "edited in the last $((LIVE / 60))m: ${recent:-none}"

echo "== sessions"
mine=" "; p=$$
while [ "${p:-0}" -gt 1 ]; do mine="$mine$p "; p=$(ppid_of "$p"); done
others=""
for p in $(pgrep -x 'claude|codex'); do
  case "$mine" in *" $p "*) continue ;; esac
  name=$(cat "/proc/$p/comm" 2>/dev/null) || continue
  # A child with its parent's name is a worker of that client, not another session.
  [ "$(cat "/proc/$(ppid_of "$p")/comm" 2>/dev/null)" = "$name" ] && continue
  cwd=$(readlink "/proc/$p/cwd" 2>/dev/null) || continue
  case "$cwd/" in "$PWD/"*) others="$others $name pid $p (up $(ps -o etime= -p "$p" | tr -d ' '));" ;; esac
done
echo "other agent sessions in this tree:${others:- none}"
cap=$(pgrep -af 'scripts/perf\.py' | head -1)
echo "perf capture running: ${cap:-none}"

echo "== frozen records (a banner naming D-215; not edited again)"
d215=$(git log --format=%h -1 -S'## D-215 ' -- DECISIONS.md)
echo "D-215 in HEAD: ${d215:-no (the cutover itself is uncommitted, so its banner edits show below)}"
hits=0
for f in $(git grep -lE 'Frozen [0-9-]+ \(`?D-215' -- docs ':!docs/plans/archive'); do
  t=$(git status --short -- "$f" | cut -c1-2 | tr -d " ")
  c=""; [ -n "$d215" ] && c=$(git log --format=%h "$d215..HEAD" -- "$f" | tr '\n' ' ')
  [ -z "$t$c" ] && continue
  hits=1
  echo "$f:${t:+ tree $t}${c:+ · commits since D-215: $c}"
done
[ "$hits" = 1 ] || echo "no edits since D-215"

echo "== ROADMAP.md"
if [ -e ROADMAP.md ]; then
  awk '/^## Now/ { f = 1 } /^## Later/ { f = 0 } f' ROADMAP.md
  echo "Now items: $(awk '/^## Now/ { f = 1; next } /^## / { f = 0 } f && /^- /' ROADMAP.md | wc -l)"
else
  echo "missing"
fi
