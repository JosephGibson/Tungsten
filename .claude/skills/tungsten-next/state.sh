#!/usr/bin/env bash
# Facts for the tungsten-next skill, in one call. Read-only and offline: origin/* are
# local refs as of the last fetch. Needs GNU date/stat and /proc (the owner's Linux host).
set -u
cd "$(git rev-parse --show-toplevel)" || exit 1
plans=docs/plans/1.0
readme=$plans/README.md
impl=$plans/implementation-plan.md
br=$(git branch --show-current)
now=$(date +%s)
# A file written under 3 minutes ago means a session is probably mid-step.
LIVE=180
# Within a step a session writes every few minutes; a longer pause is a stop for the
# owner, a build or capture, or another session. Each group is one burst of writes:
# read neighbouring groups in the same areas as one session that paused.
GAP=300
# Show only the newest groups.
SHOW=8

stamp() { date -d "@$1" '+%m-%d %H:%M'; }
age() {
  local s=$((now - $1))
  if [ "$s" -lt 3600 ]; then echo "$((s / 60))m ago"
  elif [ "$s" -lt 172800 ]; then echo "$((s / 3600))h ago"
  else echo "$((s / 86400))d ago"; fi
}
# What the release commit (git add -A) does with a path.
state() {
  if [ ! -e "$1" ] && [ ! -L "$1" ]; then echo missing
  elif git check-ignore -q -- "$1"; then echo ignored
  elif [ -z "$(git ls-files -- "$1" | head -1)" ]; then echo untracked
  elif [ -n "$(git status --porcelain -- "$1" | head -1)" ]; then echo modified
  else echo committed; fi
}
ppid_of() { sed 's/^.*) //' "/proc/$1/stat" 2>/dev/null | cut -d' ' -f2; }

# "<mtime> <path>" for every changed or untracked file, oldest first.
changes=$( { git diff --name-only -z HEAD; git ls-files -o --exclude-standard -z; } \
  | xargs -0 -r stat -c '%Y %n' 2>/dev/null | sort -n)

echo "== git"
up=$(git rev-parse --abbrev-ref '@{upstream}' 2>/dev/null || echo none)
echo "branch: $br · upstream: $up"
case "$br" in
  0.*) [ "$up" = "origin/$br" ] || echo "  WARN: upstream should be origin/$br (git push -u origin $br); a push from here goes to $up" ;;
esac
fh=$(git rev-parse --git-path FETCH_HEAD)
if [ -e "$fh" ]; then f=$(stat -c %Y "$fh"); echo "last fetch: $(stamp "$f") ($(age "$f"))"; else echo "last fetch: never"; fi
echo "head: $(git --no-pager log --oneline -1)"
echo "origin/main: $(git --no-pager log --oneline -1 origin/main 2>/dev/null || echo unknown)"
if git merge-base --is-ancestor origin/main HEAD 2>/dev/null; then
  echo "origin/main in HEAD: yes"
else
  echo "origin/main in HEAD: no (merge main before tagging)"
fi
case "$br" in
  0.*)
    c=$(git rev-list --left-right --count "origin/$br...HEAD" 2>/dev/null) && echo "vs origin/$br: ${c#*[[:space:]]} ahead, ${c%%[[:space:]]*} behind"
    tag="v${br}.0"
    if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then echo "tag $tag: exists"; else echo "tag $tag: none"; fi
    if git --no-pager log --format=%s -60 origin/main 2>/dev/null | grep -q "^Update $br:"; then
      echo "release $br: on origin/main"
    else
      echo "release $br: not on origin/main as of the last fetch"
    fi ;;
esac

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
cap=$(pgrep -af 'scripts/bench\.py' | head -1)
echo "perf capture running: ${cap:-none}"
# A connected NoMachine client runs the encoder; captures need it gone (workflow §6.3).
enc=$(pgrep -x nxcodec.bin | head -1)
[ -n "$enc" ] && enc="nxcodec.bin running (up $(ps -o etime= -p "$enc" | tr -d ' '))"
echo "remote-desktop encoder: ${enc:-none}"
waiters=""
for p in $(pgrep -f nxcodec); do
  case "$mine" in *" $p "*) continue ;; esac
  [ "$(cat "/proc/$p/comm" 2>/dev/null)" = nxcodec.bin ] && continue
  waiters="$waiters pid $p (up $(ps -o etime= -p "$p" | tr -d ' '));"
done
echo "waiting on the encoder:${waiters:- none}"

echo "== edits (last write of each changed or untracked file; a pause over $((GAP / 60))m starts a group)"
if [ -z "$changes" ]; then
  echo "none"
else
  printf '%s\n' "$changes" | awk -v gap="$GAP" -v now="$now" -v keep="$SHOW" '
    function stamp(t,  c, s) { c = "date -d @" t " \"+%m-%d %H:%M\""; c | getline s; close(c); return s }
    function ago(t,  s) { s = now - t; return s < 3600 ? int(s / 60) "m ago" : s < 172800 ? int(s / 3600) "h ago" : int(s / 86400) "d ago" }
    function flush(  a, b, i, j, k, m, out, tmp) {
      a = stamp(first); b = stamp(last)
      if (substr(a, 1, 5) == substr(b, 1, 5)) b = substr(b, 7)
      m = 0; for (k in cnt) keys[++m] = k
      for (i = 1; i <= m; i++) for (j = i + 1; j <= m; j++)
        if (cnt[keys[j]] > cnt[keys[i]]) { tmp = keys[i]; keys[i] = keys[j]; keys[j] = tmp }
      out = ""
      for (i = 1; i <= m && i <= 6; i++) out = out (i > 1 ? ", " : "") keys[i] (cnt[keys[i]] > 1 ? " (" cnt[keys[i]] ")" : "")
      if (m > 6) out = out ", +" (m - 6) " more"
      line[++g] = a "–" b " (" ago(last) ") · " n (n > 1 ? " files · " : " file · ") out
      n = 0; delete cnt; delete keys
    }
    {
      t = $1; f = substr($0, index($0, " ") + 1)
      if (n && t - last > gap) flush()
      if (!n) first = t
      n++; last = t; newest = f
      k = split(f, part, "/"); area = part[1]
      for (i = 2; i < k && i <= 3; i++) area = area "/" part[i]
      cnt[area]++
    }
    END {
      flush()
      if (g > keep) print "+" (g - keep) " earlier groups from " substr(line[1], 1, 11)
      for (i = (g > keep ? g - keep + 1 : 1); i <= g; i++) print line[i]
      print "newest: " newest " (" ago(last) ")"
    }'
fi
recent=$(printf '%s\n' "$changes" | awk -v t=$((now - LIVE)) '$1 > t { print substr($0, index($0, " ") + 1) }' | head -5 | tr '\n' ' ')
echo "edited in the last $((LIVE / 60))m: ${recent:-none}"

echo "== release commit (git add -A takes every ?? path; ignored paths stay out)"
git status --porcelain | awk '$1 != "??" { c[$1]++ }
  END { split("M modified D deleted A added R renamed", w); for (i = 1; i < 8; i += 2) name[w[i]] = w[i + 1]
        for (k in c) out = out (out ? ", " : "") c[k] " " (k in name ? name[k] : k)
        print "  tracked: " (out ? out : "no changes") }'
del=$(git ls-files -d | head -5 | tr '\n' ' ')
[ -z "$del" ] || echo "  deleted: $del"
untracked=$(git status --porcelain --untracked-files=normal | sed -n 's/^?? //p')
[ -n "$untracked" ] || echo "  untracked: none"
printf '%s\n' "$untracked" | while IFS= read -r u; do
  [ -n "$u" ] || continue
  note=""
  case "$u" in
    *__pycache__*|*.pyc|*.orig|*.rej|*~|*.swp|*.DS_Store|*perf.data*|*.log) note=" JUNK: delete before the release" ;;
  esac
  if [ -L "${u%/}" ]; then
    note=" -> $(readlink "${u%/}")$note"
  elif [ -d "$u" ]; then
    note=" ($(git ls-files -o --exclude-standard -- "$u" | wc -l) files, $(du -sh "$u" | cut -f1))$note"
  elif [ "$(stat -c %s "$u")" -gt 5242880 ]; then
    note=" LARGE ($(du -sh "$u" | cut -f1))$note"
  fi
  echo "  ?? $u$note"
done
for l in .agents/skills/*; do
  [ -L "$l" ] || continue
  t=.claude/skills/${l##*/}
  if git check-ignore -q -- "$t/SKILL.md"; then
    echo "  WARN: $t/ is ignored ($(git check-ignore -v -- "$t/SKILL.md" | cut -f1)), so the release ships $l without its target"
  fi
done

echo "== now ($readme $(state "$readme"))"
sed -n '/^## Now/,/^A session/p' "$readme" | grep '^- '
rt=$(stat -c %Y "$readme")
[ "$(state "$readme")" = committed ] && rt=$(git log -1 --format=%ct)
lag=$(printf '%s\n' "$changes" | awk -v t="$rt" -v r="$readme" -v i="$impl" \
  '$1 > t { f = substr($0, index($0, " ") + 1); if (f != r && f != i) { n++; if (n <= 4) l = l " " f } } END { if (n) print n " files, newest:" l; else print "none" }')
echo "edited after the Now lines were last written: $lag"

echo "== register (open rows; $impl $(state "$impl"))"
awk '/^## 10\. Register/{f=1} /^## 11\./{f=0} f' "$impl" \
  | grep '^| ' | grep -v -e '^| Candidate' -e '^| ---' | grep -viE '\| *(released|done)[^|]*\| *$' | head -5
newd=$(comm -13 <(git show HEAD:DECISIONS.md | grep -oE '^## D-[0-9]+' | sort) <(grep -oE '^## D-[0-9]+' DECISIONS.md | sort) | sed 's/^## //' | tr '\n' ' ')
echo "decisions not in HEAD: ${newd:-none}"

row=$(grep -m1 '^| Step 0 |' "$impl")
case "$row" in
  "" | *[Rr]eleased*) ;;
  *)
    dec() { if grep -q "^## D-$1 " DECISIONS.md; then git show HEAD:DECISIONS.md | grep -q "^## D-$1 " && echo committed || echo uncommitted; else echo missing; fi; }
    echo "== step 0 items (workflow §8)"
    echo "0a plan check: plan_files() $(grep -q '^def plan_files' scripts/check-repo.py && echo present || echo missing) in scripts/check-repo.py ($(state scripts/check-repo.py))"
    echo "0b headroom: D-106 $(dec 106), docs/assets.md $(state docs/assets.md)"
    echo "0c milestone skill: .claude/skills/tungsten-milestone/ $(state .claude/skills/tungsten-milestone/SKILL.md)"
    echo "0d api snapshot: D-107 $(dec 107), scripts/public-api.sh $(state scripts/public-api.sh), api/ $(state api), just api $(grep -q '^api:' justfile && echo present || echo missing)" ;;
esac

echo "== milestone and QA plans"
found=0
for p in "$plans"/phase*.md; do
  [ -e "$p" ] || continue
  found=1
  st=$(grep -m1 '^- \*\*status:\*\*' "$p" | sed 's/^- \*\*status:\*\* *//')
  ap=$(grep -m1 -n '^Approved' "$p" | cut -d: -f1)
  steps=$(grep -c '^### Step ' "$p")
  ev=$(awk '/^## Evidence/{f=1;next} /^## /{f=0} f' "$p" | sed -nE 's/^\| *(Step +)?([0-9]+) *\|.*/\2/p' | sort -un | tr '\n' ' ')
  miss=none
  for i in $(seq 1 "$steps"); do case " $ev" in *" $i "*) ;; *) miss=$i; break ;; esac; done
  # Steps without evidence whose commands (code spans and fenced lines) take a timing
  # capture: the runner's run, suite or capacity without --allow-background, or criterion.
  caps=$(awk -v ev=" $ev" '
    /^### Step [0-9]+/ { s = $3 + 0; next }
    /^## / { s = 0 }
    !s { next }
    /^```/ { fence = !fence; next }
    { n = split($0, part, "`")
      for (i = 1; i <= n; i++) {
        if (!fence && i % 2) continue
        c = part[i]
        if (c ~ /--allow-background/) continue
        if (c ~ /(just perf|bench\.py) +(run|suite|capacity)/ || (c ~ /cargo bench/ && c !~ /--no-run/)) hit[s] = 1
      } }
    END { for (k in hit) if (!index(ev, " " k " ")) print k }' "$p" | sort -n | tr '\n' ' ')
  [ -n "$ap" ] && ap="line $ap"
  echo "$p ($(state "$p"), $(age "$(stat -c %Y "$p")")) | status: ${st:-?} | approved: ${ap:-no} | steps: $steps | evidence: ${ev:-none}| first without evidence: $miss | captures without evidence: ${caps:-none}"
done
[ "$found" = 1 ] || echo "none"

echo "== gate sign-offs"
grep -n '^Sign-off:' "$impl" | tail -3
grep -n '^### .* gate, ' "$impl" | tail -3
