#!/usr/bin/env bash
# 为一张任务卡建隔离工作区：新分支 + 独立目录 + 复用编译缓存（APFS 克隆，不额外占空间）。
# 只有总控（Claude）运行。用法：tools/cards/new-card.sh <卡号-短名>   例：tools/cards/new-card.sh 01-shell
set -euo pipefail

ID="${1:?用法: new-card.sh <卡号-短名>}"
REPO="/Users/fox/WorkSpace/_external_tools/fox-stars-lab"
WT_ROOT="/Users/fox/WorkSpace/_external_tools/fox-stars-lab-wt"
WT="$WT_ROOT/$ID"
BRANCH="card/$ID"

cd "$REPO"
[ "$(git rev-parse --abbrev-ref HEAD)" = "fox-rebuild" ] || { echo "主工作区必须在 fox-rebuild 分支上" >&2; exit 1; }
git diff --quiet HEAD || { echo "主工作区有未提交的改动，工作区会拿不到它们。先提交或处理。" >&2; exit 1; }
[ -f "docs/rebuild/任务卡/$ID.md" ] || { echo "主线上没有任务卡 docs/rebuild/任务卡/$ID.md（先写好并提交）" >&2; exit 1; }
[ ! -e "$WT" ] || { echo "工作区已存在：$WT" >&2; exit 1; }

mkdir -p "$WT_ROOT/_logs"
git worktree add -b "$BRANCH" "$WT" fox-rebuild

# 复用依赖与编译缓存（克隆，改动互不影响）
clone() { [ -e "$REPO/$1" ] && { mkdir -p "$(dirname "$WT/$1")"; cp -c -R "$REPO/$1" "$WT/$1"; }; return 0; }
clone node_modules
clone apps/desktop/node_modules
clone apps/desktop/src-tauri/target
for d in "$REPO"/packages/*/; do
  n="$(basename "$d")"
  clone "packages/$n/node_modules"
  clone "packages/$n/dist"
done

echo "工作区就绪：$WT（分支 $BRANCH，基于 $(git rev-parse --short fox-rebuild)）"
