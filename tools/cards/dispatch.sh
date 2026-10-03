#!/usr/bin/env bash
# 把一张任务卡交给 Grok，在隔离工作区里无人值守开发。只有总控（Claude）运行。
# 用法：
#   tools/cards/dispatch.sh <卡号-短名>                  首次派发
#   tools/cards/dispatch.sh <卡号-短名> <修改意见文件>    审核退回后续作（沿用同一会话）
# 可选环境变量：MAX_TURNS（默认 200）、GROK_MODEL（默认 grok-4.7）、GROK_EFFORT（默认 high）
# 日志：/Users/fox/WorkSpace/_external_tools/fox-stars-lab-wt/_logs/
#
# 安全模型（三层，缺一不可）：
#   1. 内核沙盒 fsl-dev（.grok/sandbox.toml）：只能写工作区；正式库、开发库、钥匙串、各类凭据读写都被禁。
#   2. 禁令清单：git 写命令、联网下载、杀进程、装依赖等会被拦截，只回给 Grok 一条错误，它会换办法继续。
#   3. 干净环境：用 env -i 启动，不继承总控会话里的任何令牌、套接字和其他变量。
# 为什么不用 dontAsk 白名单：实测 Grok 在白名单模式下，一次被拒就把整轮任务中止（stopReason=cancelled）。
# 改动范围（任务卡「允许改动」）由总控事后用 tools/cards/check-scope.sh 核对，越界的改动会被还原。
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

# 会话：首次派发新建；退回后续作则沿用同一个会话
SESSION_FILE="$LOGDIR/$ID.session"
STAMP="$(date +%m%d-%H%M%S)"
LOG="$LOGDIR/$ID-$STAMP.jsonl"

if [ -n "$FEEDBACK_SRC" ]; then
  [ -f "$SESSION_FILE" ] || { echo "没有可续作的会话记录：$SESSION_FILE" >&2; exit 1; }
  [ -f "$FEEDBACK_SRC" ] || { echo "找不到修改意见文件：$FEEDBACK_SRC" >&2; exit 1; }
  SESSION="$(cat "$SESSION_FILE")"
  cp "$FEEDBACK_SRC" "$WT/.card-feedback.md"
  PROMPT="总控审核后退回了这张卡（卡号 $ID）。修改意见在工作区根目录的 .card-feedback.md。请逐条处理；处理完更新 .card-report.md：保留原来的结论，新增一节「第 N 轮修改」，写清每条意见怎么处理的、做了什么验证。其余规则不变：只改任务卡允许的路径，构建和测试只用 tools/cards/run.sh，没做到的如实写。最后一条消息只写：一句话结论 + 报告在 .card-report.md。"
  SESSION_ARGS=(--resume "$SESSION")
else
  SESSION="$(uuidgen | tr 'A-Z' 'a-z')"
  echo "$SESSION" > "$SESSION_FILE"
  PROMPT="你是 Fox Stars Lab 2.0 的开发者。你的工作目录是任务卡 $ID 的隔离工作区。
1. 先完整阅读：AGENTS.md、docs/rebuild/命令契约.md、你的任务卡 docs/rebuild/任务卡/$ID.md；需要时再读卡里列出的其他文件。
2. 严格按任务卡执行：只改「允许改动」里的路径，满足「验收」的每一条；构建和测试只用 tools/cards/run.sh。
3. 完成后把交回报告写到工作区根目录的 .card-report.md，格式见命令契约 §6。没做到的如实写，不要美化。
4. 没人能回答你的问题：遇到卡里没写清的，选最保守、可回退的做法并记入报告；遇到必须 Fox 或总控决定的事，停手并写进报告。
5. 某个命令被拦截时，换一种办法完成；不要尝试绕过拦截。
最后一条消息只写：一句话结论 + 报告在 .card-report.md。"
  SESSION_ARGS=(-s "$SESSION")
fi

# 固定参数
ARGS=(
  -p "$PROMPT"
  "${SESSION_ARGS[@]}"
  --cwd "$WT"
  --sandbox fsl-dev
  --always-approve
  --disable-web-search
  --max-turns "${MAX_TURNS:-200}"
  -m "${GROK_MODEL:-grok-4.7}"
  --reasoning-effort "${GROK_EFFORT:-high}"
  --output-format streaming-json
  --rules "$(cat "$WT/AGENTS.md")"
  # 数据与凭据（内核沙盒之外再加一层规则）
  --deny 'Read(/Users/fox/Library/Application Support/com.foxwork.**)'
  --deny 'Edit(/Users/fox/Library/Application Support/com.foxwork.**)'
  # git：只读命令放行，写命令一律拦截（提交与合并由总控做）
  --deny 'Bash(git add *)'
  --deny 'Bash(git commit *)'
  --deny 'Bash(git push *)'
  --deny 'Bash(git pull *)'
  --deny 'Bash(git fetch *)'
  --deny 'Bash(git checkout *)'
  --deny 'Bash(git switch *)'
  --deny 'Bash(git restore *)'
  --deny 'Bash(git reset *)'
  --deny 'Bash(git clean *)'
  --deny 'Bash(git stash *)'
  --deny 'Bash(git remote *)'
  --deny 'Bash(git worktree *)'
  --deny 'Bash(git tag *)'
  --deny 'Bash(git branch -*)'
  --deny 'Bash(git merge *)'
  --deny 'Bash(git rebase *)'
  --deny 'Bash(git cherry-pick *)'
  --deny 'Bash(git revert *)'
  --deny 'Bash(git config *)'
  --deny 'Bash(git rm *)'
  --deny 'Bash(git mv *)'
  --deny 'Bash(git apply *)'
  --deny 'Bash(gh *)'
  # 联网、远程、提权、系统改动、杀进程
  --deny 'Bash(curl *)'
  --deny 'Bash(wget *)'
  --deny 'Bash(nc *)'
  --deny 'Bash(ncat *)'
  --deny 'Bash(ssh *)'
  --deny 'Bash(scp *)'
  --deny 'Bash(sftp *)'
  --deny 'Bash(rsync *)'
  --deny 'Bash(telnet *)'
  --deny 'Bash(security *)'
  --deny 'Bash(sudo *)'
  --deny 'Bash(su *)'
  --deny 'Bash(open *)'
  --deny 'Bash(osascript *)'
  --deny 'Bash(launchctl *)'
  --deny 'Bash(defaults *)'
  --deny 'Bash(kill *)'
  --deny 'Bash(pkill *)'
  --deny 'Bash(killall *)'
  --deny 'Bash(chmod *)'
  --deny 'Bash(chown *)'
  # 装依赖、启动 App、跑开发服务
  --deny 'Bash(pnpm install *)'
  --deny 'Bash(pnpm add *)'
  --deny 'Bash(pnpm remove *)'
  --deny 'Bash(pnpm update *)'
  --deny 'Bash(pnpm dlx *)'
  --deny 'Bash(pnpm dev*)'
  --deny 'Bash(pnpm tauri*)'
  --deny 'Bash(pnpm package*)'
  --deny 'Bash(npm *)'
  --deny 'Bash(npx *)'
  --deny 'Bash(yarn *)'
  --deny 'Bash(pip *)'
  --deny 'Bash(pip3 *)'
  --deny 'Bash(brew *)'
  --deny 'Bash(cargo add *)'
  --deny 'Bash(cargo install *)'
  --deny 'Bash(cargo update*)'
  --deny 'Bash(cargo run*)'
  --deny 'Bash(cargo fetch*)'
  --deny 'Bash(cargo publish*)'
  --deny 'Bash(rustup *)'
  # 递归删除
  --deny 'Bash(rm -r*)'
  --deny 'Bash(rm -fr*)'
  --deny 'Bash(rm -R*)'
  --deny 'Bash(rm -Rf*)'
)

# 干净环境：只给最少的变量（代理设置若有则带上，否则 Grok 连不上网）
CLEAN_ENV=(
  "HOME=$HOME"
  "USER=$(id -un)"
  "LOGNAME=$(id -un)"
  "SHELL=/bin/zsh"
  "LANG=en_US.UTF-8"
  "TERM=dumb"
  "PATH=$HOME/.grok/bin:$HOME/.cargo/bin:$HOME/.local/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
)
for v in HTTP_PROXY HTTPS_PROXY ALL_PROXY NO_PROXY http_proxy https_proxy all_proxy no_proxy; do
  if [ -n "${!v:-}" ]; then CLEAN_ENV+=("$v=${!v}"); fi
done

echo "派发 $ID | 会话 $SESSION | 日志 $LOG"
set +e
env -i "${CLEAN_ENV[@]}" "$GROK" "${ARGS[@]}" > "$LOG" 2> "$LOG.err"
CODE=$?
set -e

# 汇总：看最后一个 end 事件（停止原因、轮数、用量、花费）
python3 - "$LOG" "$LOGDIR/$ID.usage.jsonl" "$ID" <<'PY'
import json, sys, time
log, usage_file, card = sys.argv[1:4]
end = None
n = 0
for line in open(log, encoding="utf-8", errors="replace"):
    line = line.strip()
    if not line:
        continue
    n += 1
    try:
        o = json.loads(line)
    except Exception:
        continue
    if o.get("type") == "end":
        end = o
print("日志行数：%d" % n)
if end is None:
    print("没有找到 end 事件：Grok 可能异常退出，看 .err 文件")
else:
    u = end.get("usage") or {}
    print("停止原因：%s | 轮数：%s | 总 token：%s | 花费：$%.3f" % (
        end.get("stopReason"), end.get("num_turns"), u.get("total_tokens"), end.get("total_cost_usd") or 0))
    with open(usage_file, "a", encoding="utf-8") as fh:
        fh.write(json.dumps({"time": time.strftime("%Y-%m-%d %H:%M:%S"), "card": card, "log": log,
                             "stopReason": end.get("stopReason"), "turns": end.get("num_turns"),
                             "total_tokens": u.get("total_tokens"), "cost_usd": end.get("total_cost_usd")},
                            ensure_ascii=False) + "\n")
PY
echo "grok 退出码：$CODE"
echo "stderr（末 5 行）："
tail -n 5 "$LOG.err" 2>/dev/null || true
echo "报告：$([ -f "$WT/.card-report.md" ] && echo "$WT/.card-report.md（已生成）" || echo "未生成")"
exit "$CODE"
