#!/usr/bin/env bash
# 把一张任务卡交给 Grok，在隔离工作区里无人值守开发。只有总控（Claude）运行。
# 用法：
#   tools/cards/dispatch.sh <卡号-短名>                  首次派发
#   tools/cards/dispatch.sh <卡号-短名> <修改意见文件>    审核退回后续作（沿用同一会话）
# 可选环境变量：MAX_TURNS（默认 200）、GROK_MODEL（默认 grok-4.7）、GROK_EFFORT（默认 high）
# 日志：/Users/fox/WorkSpace/_external_tools/fox-stars-lab-wt/_logs/
set -euo pipefail

ID="${1:?用法: dispatch.sh <卡号-短名> [修改意见文件]}"
FEEDBACK_SRC="${2:-}"
WT_ROOT="/Users/fox/WorkSpace/_external_tools/fox-stars-lab-wt"
WT="$WT_ROOT/$ID"
LOGDIR="$WT_ROOT/_logs"
GROK="$HOME/.grok/bin/grok"
CARD="$WT/docs/rebuild/任务卡/$ID.md"

[ -d "$WT" ] || { echo "没有工作区 $WT，先运行 tools/cards/new-card.sh $ID" >&2; exit 1; }
[ -f "$CARD" ] || { echo "工作区里没有任务卡 $CARD" >&2; exit 1; }
[ -f "$WT/AGENTS.md" ] || { echo "工作区里没有 AGENTS.md" >&2; exit 1; }
[ -x "$GROK" ] || { echo "找不到 grok：$GROK" >&2; exit 1; }
mkdir -p "$LOGDIR"

# 1) 允许改动的路径：取自任务卡头部「允许改动」清单。清单之外的写入会被权限层直接拒绝。
EDIT_GLOBS="$(python3 - "$CARD" <<'PY'
import re, sys
text = open(sys.argv[1], encoding="utf-8").read()
m = re.match(r"^---\n(.*?)\n---\n", text, re.S)
out, inblock = [], False
for line in (m.group(1).splitlines() if m else []):
    if line.startswith("允许改动"):
        inblock = True
        continue
    if inblock:
        mm = re.match(r"^\s+-\s+(\S.*?)\s*$", line)
        if mm:
            out.append(mm.group(1))
        elif line.strip():
            inblock = False
print("\n".join(out))
PY
)"

ARGS=()
if [ -n "$EDIT_GLOBS" ]; then
  while IFS= read -r g; do
    case "$g" in
      /*|*..*|'**'|'*') echo "任务卡里有不合规的允许路径：$g" >&2; exit 1 ;;
    esac
    ARGS+=(--allow "Edit($g)")
  done <<EOF
$EDIT_GLOBS
EOF
fi

# 2) 会话：首次派发新建；退回后续作则沿用同一个会话
SESSION_FILE="$LOGDIR/$ID.session"
STAMP="$(date +%m%d-%H%M%S)"
LOG="$LOGDIR/$ID-$STAMP.jsonl"

if [ -n "$FEEDBACK_SRC" ]; then
  [ -f "$SESSION_FILE" ] || { echo "没有可续作的会话记录：$SESSION_FILE" >&2; exit 1; }
  [ -f "$FEEDBACK_SRC" ] || { echo "找不到修改意见文件：$FEEDBACK_SRC" >&2; exit 1; }
  SESSION="$(cat "$SESSION_FILE")"
  cp "$FEEDBACK_SRC" "$WT/.card-feedback.md"
  PROMPT="总控审核后退回了这张卡（卡号 $ID）。修改意见在工作区根目录的 .card-feedback.md。请逐条处理；处理完更新 .card-report.md：保留原来的结论，新增一节「第 N 轮修改」，写清每条意见怎么处理的、做了什么验证。其余规则不变：只改任务卡允许的路径，只用 tools/cards/run.sh 验证，没做到的如实写。最后一条消息只写：一句话结论 + 报告在 .card-report.md。"
  SESSION_ARGS=(--resume "$SESSION")
else
  SESSION="$(uuidgen | tr 'A-Z' 'a-z')"
  echo "$SESSION" > "$SESSION_FILE"
  PROMPT="你是 Fox Stars Lab 2.0 的开发者。你的工作目录是任务卡 $ID 的隔离工作区。
1. 先完整阅读：AGENTS.md、docs/rebuild/命令契约.md、你的任务卡 docs/rebuild/任务卡/$ID.md；需要时再读卡里列出的其他文件。
2. 严格按任务卡执行：只改「允许改动」里的路径，满足「验收」的每一条，只用 tools/cards/run.sh 做构建和测试。
3. 完成后把交回报告写到工作区根目录的 .card-report.md，格式见命令契约 §6。没做到的如实写，不要美化。
4. 没人能回答你的问题：遇到卡里没写清的，选最保守、可回退的做法并记入报告；遇到必须 Fox 或总控决定的事，停手并写进报告。
最后一条消息只写：一句话结论 + 报告在 .card-report.md。"
  SESSION_ARGS=(-s "$SESSION")
fi

# 3) 固定的安全参数：沙盒 + 只准可信命令（dontAsk）+ 明确禁令
ARGS+=(
  -p "$PROMPT"
  "${SESSION_ARGS[@]}"
  --cwd "$WT"
  --sandbox fsl-dev
  --permission-mode dontAsk
  --disable-web-search
  --max-turns "${MAX_TURNS:-200}"
  -m "${GROK_MODEL:-grok-4.7}"
  --reasoning-effort "${GROK_EFFORT:-high}"
  --output-format streaming-json
  --rules "$(cat "$WT/AGENTS.md")"
  --allow 'Read'
  --allow 'Grep'
  --allow 'Bash(tools/cards/run.sh *)'
  --allow 'Edit(.card-report.md)'
  --deny 'Read(/Users/fox/Library/Application Support/com.foxwork.**)'
  --deny 'Edit(/Users/fox/Library/Application Support/com.foxwork.**)'
  --deny 'Bash(git add*)'
  --deny 'Bash(git commit*)'
  --deny 'Bash(git push*)'
  --deny 'Bash(git checkout*)'
  --deny 'Bash(git switch*)'
  --deny 'Bash(git reset*)'
  --deny 'Bash(git clean*)'
  --deny 'Bash(git stash*)'
  --deny 'Bash(git remote*)'
  --deny 'Bash(git worktree*)'
  --deny 'Bash(git tag*)'
  --deny 'Bash(security*)'
  --deny 'Bash(curl*)'
  --deny 'Bash(wget*)'
  --deny 'Bash(sudo*)'
  --deny 'Bash(open*)'
  --deny 'Bash(osascript*)'
  --deny 'Bash(pnpm install*)'
  --deny 'Bash(pnpm add*)'
  --deny 'Bash(npm*)'
  --deny 'Bash(cargo add*)'
  --deny 'Bash(cargo run*)'
  --deny 'Bash(rm -rf*)'
)

echo "派发 $ID | 会话 $SESSION | 日志 $LOG"
set +e
"$GROK" "${ARGS[@]}" > "$LOG" 2> "$LOG.err"
CODE=$?
set -e

echo "grok 退出码：$CODE"
echo "日志行数：$(wc -l < "$LOG" | tr -d ' ')"
echo "stderr（末 5 行）："
tail -n 5 "$LOG.err" 2>/dev/null || true
echo "报告：$([ -f "$WT/.card-report.md" ] && echo "$WT/.card-report.md（已生成）" || echo "未生成")"
exit "$CODE"
