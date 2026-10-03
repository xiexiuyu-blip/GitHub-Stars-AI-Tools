#!/usr/bin/env bash
# 为一张任务卡建隔离工作区：新分支（基于集成分支）+ 独立目录 + 装好依赖 + 预热编译缓存。
# 只有总控（Claude）运行。用法：tools/cards/new-card.sh <卡号-短名> [基线分支，默认 fox-integration]
#
# 两条线：
#   fox-rebuild     = Fox 的开发版在跑的分支（主工作区）。只在总控确认过的时间点更新。
#   fox-integration = 总控的集成线。每张卡审核通过后先合并到这里（在 ../fox-stars-lab-wt/_integration 里做）。
set -euo pipefail

ID="${1:?用法: new-card.sh <卡号-短名> [基线分支]}"
BASE="${2:-fox-integration}"
REPO="/Users/fox/WorkSpace/_external_tools/fox-stars-lab"
WT_ROOT="/Users/fox/WorkSpace/_external_tools/fox-stars-lab-wt"
WT="$WT_ROOT/$ID"
BRANCH="card/$ID"

cd "$REPO"
git rev-parse --verify -q "$BASE" >/dev/null || { echo "没有基线分支 $BASE" >&2; exit 1; }
git cat-file -e "$BASE:docs/rebuild/任务卡/$ID.md" 2>/dev/null || { echo "基线分支 $BASE 上没有任务卡 docs/rebuild/任务卡/$ID.md（先在集成区写好并提交）" >&2; exit 1; }
[ ! -e "$WT" ] || { echo "工作区已存在：$WT" >&2; exit 1; }

mkdir -p "$WT_ROOT/_logs"
git worktree add -b "$BRANCH" "$WT" "$BASE"
"$REPO/tools/cards/warm.sh" "$WT"

echo "工作区就绪：$WT（分支 $BRANCH，基于 $BASE 的 $(git rev-parse --short "$BASE")）"
