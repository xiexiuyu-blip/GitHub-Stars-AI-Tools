#!/usr/bin/env bash
# 核对一张卡的改动是否都在任务卡「允许改动」范围内。只有总控（Claude）运行。
# 用法：tools/cards/check-scope.sh <卡号-短名>
# 退出码：0 全部合规；1 有越界文件（逐行列出，由总控决定还原还是认可）。
set -euo pipefail

ID="${1:?用法: check-scope.sh <卡号-短名>}"
WT="/Users/fox/WorkSpace/_external_tools/fox-stars-lab-wt/$ID"
CARD="$WT/docs/rebuild/任务卡/$ID.md"
[ -d "$WT" ] || { echo "没有工作区 $WT" >&2; exit 2; }
[ -f "$CARD" ] || { echo "没有任务卡 $CARD" >&2; exit 2; }

cd "$WT"
git -c core.quotepath=false status --porcelain=v1 -uall -z | python3 -c '
import re, sys

card = sys.argv[1]
text = open(card, encoding="utf-8").read()
m = re.match(r"^---\n(.*?)\n---\n", text, re.S)
globs, inblock = [], False
for line in (m.group(1).splitlines() if m else []):
    if line.startswith("允许改动"):
        inblock = True
        continue
    if inblock:
        mm = re.match(r"^\s+-\s+(\S.*?)\s*$", line)
        if mm:
            globs.append(mm.group(1))
        elif line.strip():
            inblock = False

def to_regex(g):
    out, i = "", 0
    while i < len(g):
        c = g[i]
        if g[i:i+3] == "**/":
            out += "(?:.*/)?"; i += 3; continue
        if g[i:i+2] == "**":
            out += ".*"; i += 2; continue
        if c == "*":
            out += "[^/]*"
        elif c == "?":
            out += "[^/]"
        else:
            out += re.escape(c)
        i += 1
    return re.compile("^" + out + "$")

rules = [to_regex(g) for g in globs]
changed = []
for rec in sys.stdin.buffer.read().split(b"\0"):
    if not rec:
        continue
    status, path = rec[:2].decode(), rec[3:].decode("utf-8")
    changed.append((status.strip(), path))

bad = [(s, p) for s, p in changed if not any(r.match(p) for r in rules)]
print("改动文件 %d 个，允许范围 %d 条规则" % (len(changed), len(globs)))
for s, p in changed:
    print("  %s %s%s" % ("✗" if (s, p) in bad else "✓", s.ljust(2), p))
if bad:
    print("越界文件 %d 个" % len(bad))
    sys.exit(1)
print("全部在允许范围内")
' "$CARD"
