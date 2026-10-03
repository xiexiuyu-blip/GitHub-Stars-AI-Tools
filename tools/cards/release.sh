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
# 钱的闸：开发版的每日 AI 预算必须是 0，除非 Fox 明确批准过。否则新版后台一启动就会自己开始花 token。
DEV_QUEUE="$HOME/Library/Application Support/com.foxwork.fox-stars-lab.dev/fox-stars-lab.queue.json"
if [ -f "$DEV_QUEUE" ]; then
  BUDGET="$(python3 -c 'import json,sys; print(int(json.load(open(sys.argv[1])).get("dailyBudgetTokens", 0)))' "$DEV_QUEUE" 2>/dev/null || echo 0)"
  if [ "$BUDGET" -gt 0 ]; then
    if [ "${RESET_DEV_BUDGET:-}" = "1" ]; then
      python3 - "$DEV_QUEUE" <<'PY'
import json, sys
p = sys.argv[1]
d = json.load(open(p))
d["dailyBudgetTokens"] = 0
json.dump(d, open(p, "w"))
PY
      echo "!! 已把开发版每日 AI 预算从 $BUDGET 重置为 0（Fox 还没批准花额度）"
    elif [ "${KEEP_DEV_BUDGET:-}" = "1" ]; then
      echo "!! 开发版每日 AI 预算是 $BUDGET，按 Fox 明确批准的保留"
    else
      echo "开发版的每日 AI 预算是 $BUDGET（大于 0）：发布后后台会自己开始花 token。Fox 没批准前必须是 0。" >&2
      echo "要重置就加 RESET_DEV_BUDGET=1 再运行；Fox 明确批准过就加 KEEP_DEV_BUDGET=1。" >&2
      exit 1
    fi
  fi
fi
echo "== 3/3 快进合并 fox-rebuild ← fox-integration"
git merge --ff-only fox-integration
git log --oneline -1
