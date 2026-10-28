#!/usr/bin/env bash
# 把集成线推到 Fox 的开发版（fox-rebuild，主工作区）。只有总控运行。
# 先在集成区跑完整检查，全绿才快进合并；之后 Fox 开着的开发版会自动更新。
# 如果这次带了后台（Rust）改动，开发版会重新编译，macOS 会弹一次钥匙串授权，只有 Fox 能点——
# 所以发布前要先告诉 Fox。
# 用法：tools/cards/release.sh
set -euo pipefail

REPO="/Users/fox/WorkSpace/_infra/fox-stars-lab"
INT="/Users/fox/WorkSpace/_infra/fox-stars-lab-wt/_integration"

cd "$INT"
git diff --quiet HEAD || { echo "集成区有未提交的改动" >&2; exit 1; }
echo "== 1/3 集成区完整检查"
tools/cards/run.sh rust-fmt
tools/cards/run.sh rust-test 2>&1 | tee /tmp/fsl-release-rust.log
grep -q "^test result: ok" /tmp/fsl-release-rust.log && ! grep -q "FAILED" /tmp/fsl-release-rust.log || { echo "Rust 测试没有全过，不发布" >&2; exit 1; }
tools/cards/run.sh web-build 2>&1 | tee /tmp/fsl-release-build.log
tools/cards/run.sh web-test 2>&1 | tee /tmp/fsl-release-web.log
! grep -qE "failed|FAIL" /tmp/fsl-release-web.log || { echo "前端测试有失败，不发布" >&2; exit 1; }

cd "$REPO"
git diff --quiet HEAD || { echo "主工作区有未提交的改动，不发布" >&2; exit 1; }
BACK_CHANGED="$(git diff --name-only HEAD fox-integration -- apps/desktop/src-tauri | grep -vE '(gen/schemas|Cargo.lock)' | head -1 || true)"
echo "== 2/3 将发布的改动："
# 用 -n 限制条数，不要接 `| head`：提交多于 30 条时 head 提前关管道，git 收到 SIGPIPE，pipefail 会让整个脚本在这里悄悄退出
git log --oneline -n 30 HEAD..fox-integration
echo "（共 $(git rev-list --count HEAD..fox-integration) 个提交，最多列出 30 个）"
if [ -n "$BACK_CHANGED" ]; then
  echo "!! 本次包含后台（Rust）改动：开发版会重新编译，之后会弹一次钥匙串授权（只有 Fox 能点）。"
fi
# 钱的闸：开发版的每日 AI 预算必须是 0，除非 Fox 明确批准过。否则新版后台一启动就会自己开始花 token。
DEV_QUEUE="$HOME/Library/Application Support/com.foxwork.fox-stars-lab.dev/fox-stars-lab.queue.json"
if ! BUDGET="$(python3 -c '
import json, sys
with open(sys.argv[1]) as handle:
    value = json.load(handle)["dailyBudgetTokens"]
if type(value) is not int or value < 0:
    raise ValueError("invalid dailyBudgetTokens")
print(value)
' "$DEV_QUEUE" 2>/dev/null)"; then
  echo "无法确认开发版 AI 额度，停止发布。请先检查控制文件；不会把读取失败当成额度为 0。" >&2
  exit 1
fi
# 在 Python 校验后比较字符串，避免合法大整数超出 Bash 范围而漏过门禁。
if [ "$BUDGET" != "0" ]; then
  if [ "${RESET_DEV_BUDGET:-}" = "1" ]; then
    python3 - "$DEV_QUEUE" <<'PYRESET'
import json, sys
p = sys.argv[1]
d = json.load(open(p))
d["dailyBudgetTokens"] = 0
json.dump(d, open(p, "w"))
PYRESET
    echo "!! 已把开发版每日 AI 预算从 $BUDGET 重置为 0（Fox 还没批准花额度）"
  elif [ "${KEEP_DEV_BUDGET:-}" = "1" ]; then
    echo "!! 开发版每日 AI 预算是 $BUDGET，按 Fox 明确批准的保留"
  else
    echo "开发版的每日 AI 预算是 ${BUDGET}（大于 0）：发布后后台会自己开始花 token。Fox 没批准前必须是 0。" >&2
    echo "要重置就加 RESET_DEV_BUDGET=1 再运行；Fox 明确批准过就加 KEEP_DEV_BUDGET=1。" >&2
    exit 1
  fi
fi
echo "== 3/3 快进合并 fox-rebuild ← fox-integration"
git merge --ff-only fox-integration
git log --oneline -1
