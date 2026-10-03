#!/usr/bin/env bash
# 为一张任务卡建隔离工作区：新分支 + 独立目录 + 装好依赖 + 预热编译缓存。
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

# 1) Rust 编译缓存：克隆（改动互不影响；本地的 app crate 会重编，第三方依赖复用）
if [ -d "$REPO/apps/desktop/src-tauri/target" ]; then
  mkdir -p "$WT/apps/desktop/src-tauri"
  cp -c -R "$REPO/apps/desktop/src-tauri/target" "$WT/apps/desktop/src-tauri/target"
fi

# 2) 前端依赖：从 pnpm 全局仓库离线安装（硬链接，不联网；CI=true 避免要终端确认）
( cd "$WT" && CI=true pnpm install --offline --frozen-lockfile )

# 3) 预热：前端构建（生成 dist，Tauri 编译需要它）+ Rust 检查与测试编译
( cd "$WT" && tools/cards/run.sh web-build )
( cd "$WT" && tools/cards/run.sh rust-check )
( cd "$WT" && cargo test --manifest-path "$WT/apps/desktop/src-tauri/Cargo.toml" --lib --no-run )

echo "工作区就绪：$WT（分支 $BRANCH，基于 $(git rev-parse --short fox-rebuild)）"
