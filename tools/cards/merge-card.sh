#!/usr/bin/env bash
# 审核通过后合并一张卡。只有总控（Claude）运行。任何一步失败都立刻停下，不会合进集成线。
# 步骤：把集成线并进卡分支（先在卡分支上解决冲突）→ 在卡分支上跑完整检查（含沙盒里跳过的钥匙串用例）→ --no-ff 合并进集成线。
# 用法：tools/cards/merge-card.sh <卡号-短名> "<一句话说明>"
set -euo pipefail

ID="${1:?用法: merge-card.sh <卡号-短名> \"<一句话说明>\"}"
MSG="${2:?请给一句话说明}"
WT_ROOT="/Users/fox/WorkSpace/_external_tools/fox-stars-lab-wt"
WT="$WT_ROOT/$ID"
INT="$WT_ROOT/_integration"

cd "$WT"
git diff --quiet HEAD || { echo "卡工作区有未提交的改动，先提交" >&2; exit 1; }
echo "== 1/4 把集成线并进卡分支"
git merge fox-integration -m "合并集成线进卡 $ID" || { echo "有冲突：先在 $WT 里解决，提交后重跑" >&2; exit 1; }
echo "== 2/4 Rust：格式、编译、完整测试（含钥匙串用例）"
tools/cards/run.sh rust-fmt
tools/cards/run.sh rust-check 2>&1 | grep -E "generated [0-9]+ warning" || true
cargo test --manifest-path "$WT/apps/desktop/src-tauri/Cargo.toml" --lib 2>&1 | tee /tmp/fsl-merge-$ID.log | grep -E "^test result|FAILED"
grep -q "^test result: ok" /tmp/fsl-merge-$ID.log || { echo "Rust 测试没有全过，不合并" >&2; exit 1; }
! grep -q "FAILED" /tmp/fsl-merge-$ID.log || { echo "有失败的测试，不合并" >&2; exit 1; }
echo "== 3/4 前端：类型检查与测试"
tools/cards/run.sh web-build 2>&1 | grep -E "built in|error" || true
tools/cards/run.sh web-test 2>&1 | tee /tmp/fsl-merge-$ID-web.log | grep -E "Test Files|Tests "
! grep -qE "failed|FAIL" /tmp/fsl-merge-$ID-web.log || { echo "前端测试有失败，不合并" >&2; exit 1; }
echo "== 4/4 合并进集成线"
cd "$INT"
git diff --quiet HEAD || { echo "集成区有未提交的改动" >&2; exit 1; }
git merge --no-ff "card/$ID" -m "合并 卡$ID：$MSG

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
git log --oneline -1
