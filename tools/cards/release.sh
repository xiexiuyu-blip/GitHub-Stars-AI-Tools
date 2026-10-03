#!/usr/bin/env bash
# 把集成线推到 Fox 的开发版（fox-rebuild，主工作区）。只有总控（Claude）运行。
# 先在集成区跑完整检查，全绿才快进合并；之后 Fox 开着的开发版会自动更新。
# 如果这次带了后台（Rust）改动，开发版会重新编译，macOS 会弹一次钥匙串授权，只有 Fox 能点——
# 所以发布前要先告诉 Fox。
# 用法：tools/cards/release.sh
set -euo pipefail

REPO="/Users/fox/WorkSpace/_external_tools/fox-stars-lab"
INT="/Users/fox/WorkSpace/_external_tools/fox-stars-lab-wt/_integration"

cd "$INT"
git diff --quiet HEAD || { echo "集成区有未提交的改动" >&2; exit 1; }
echo "== 1/3 集成区完整检查"
tools/cards/run.sh rust-fmt
cargo test --manifest-path "$INT/apps/desktop/src-tauri/Cargo.toml" --lib 2>&1 | tee /tmp/fsl-release-rust.log | grep -E "^test result|FAILED"
grep -q "^test result: ok" /tmp/fsl-release-rust.log && ! grep -q "FAILED" /tmp/fsl-release-rust.log || { echo "Rust 测试没有全过，不发布" >&2; exit 1; }
tools/cards/run.sh web-build 2>&1 | grep -E "built in|error" || true
tools/cards/run.sh web-test 2>&1 | tee /tmp/fsl-release-web.log | grep -E "Test Files|Tests "
! grep -qE "failed|FAIL" /tmp/fsl-release-web.log || { echo "前端测试有失败，不发布" >&2; exit 1; }

cd "$REPO"
git diff --quiet HEAD || { echo "主工作区有未提交的改动，不发布" >&2; exit 1; }
BACK_CHANGED="$(git diff --name-only HEAD fox-integration -- apps/desktop/src-tauri | grep -vE '(gen/schemas|Cargo.lock)' | head -1 || true)"
echo "== 2/3 将发布的改动："
git log --oneline HEAD..fox-integration | head -30
if [ -n "$BACK_CHANGED" ]; then
  echo "!! 本次包含后台（Rust）改动：开发版会重新编译，之后会弹一次钥匙串授权（只有 Fox 能点）。"
fi
echo "== 3/3 快进合并 fox-rebuild ← fox-integration"
git merge --ff-only fox-integration
git log --oneline -1
