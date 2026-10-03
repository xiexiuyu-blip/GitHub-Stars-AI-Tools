#!/usr/bin/env bash
# 给一个新建的 git 工作区装好依赖、克隆 Rust 编译缓存并预热构建。只有总控（Claude）运行。
# 用法：tools/cards/warm.sh <工作区绝对路径>
set -euo pipefail

WT="${1:?用法: warm.sh <工作区绝对路径>}"
REPO="/Users/fox/WorkSpace/_external_tools/fox-stars-lab"
[ -d "$WT" ] || { echo "没有这个目录：$WT" >&2; exit 1; }

# 1) Rust 编译缓存：克隆（改动互不影响；本地的 app crate 会重编，第三方依赖复用）
if [ -d "$REPO/apps/desktop/src-tauri/target" ]; then
  mkdir -p "$WT/apps/desktop/src-tauri"
  cp -c -R "$REPO/apps/desktop/src-tauri/target" "$WT/apps/desktop/src-tauri/target"
fi

# 2) 前端依赖：按锁文件原样安装（--frozen-lockfile，版本与主工作区一致；仓库里已有的包走硬链接；CI=true 避免要终端确认）
( cd "$WT" && CI=true pnpm install --frozen-lockfile )

# 3) 预热：前端构建（生成 dist，Tauri 编译需要它）+ Rust 检查与测试编译
( cd "$WT" && tools/cards/run.sh web-build )
( cd "$WT" && tools/cards/run.sh rust-check )
( cd "$WT" && cargo test --manifest-path "$WT/apps/desktop/src-tauri/Cargo.toml" --lib --no-run )
