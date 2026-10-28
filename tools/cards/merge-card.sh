#!/usr/bin/env bash
# 审核通过后合并一张卡。只有总控运行。任何一步失败都立刻停下，不会合进集成线。
# 步骤：把集成线并进卡分支（先在卡分支上解决冲突）→ 跑检查（按 run.sh 跳过钥匙串用例）→ --no-ff 合并进集成线。
# 用法：tools/cards/merge-card.sh <卡号-短名> "<一句话说明>"
set -euo pipefail

ID="${1:?用法: merge-card.sh <卡号-短名> \"<一句话说明>\"}"
MSG="${2:?请给一句话说明}"
WT_ROOT="/Users/fox/WorkSpace/_infra/fox-stars-lab-wt"
WT="$WT_ROOT/$ID"
INT="$WT_ROOT/_integration"

cd "$WT"
git diff --quiet HEAD || { echo "卡工作区有未提交的改动，先提交" >&2; exit 1; }
echo "== 1/4 把集成线并进卡分支"
if ! git merge fox-integration -m "合并集成线进卡 $ID"; then
  CONFLICTED="$(git diff --name-only --diff-filter=U)"
  ACL="apps/desktop/src-tauri/gen/schemas/acl-manifests.json"
  if [ "$CONFLICTED" = "$ACL" ]; then
    # 唯一的冲突是生成文件：取集成线的版本，再让 cargo 按合并后的权限清单重新生成
    echo "只有生成文件 acl-manifests.json 冲突：按合并后的权限清单重新生成"
    git checkout --theirs "$ACL"
    tools/cards/run.sh rust-check >/dev/null 2>&1 || { echo "重新生成时编译失败" >&2; exit 1; }
    git add "$ACL"
    git commit -q -m "合并集成线进卡 ${ID}：acl-manifests.json 冲突按合并后的权限清单重新生成"
  else
    echo "有冲突（$CONFLICTED）：先在 $WT 里解决，提交后重跑" >&2
    exit 1
  fi
fi
echo "== 2/4 Rust：格式、编译、测试（按 run.sh 排除钥匙串用例）"
tools/cards/run.sh rust-fmt
tools/cards/run.sh rust-check
tools/cards/run.sh rust-test 2>&1 | tee /tmp/fsl-merge-$ID.log
grep -q "^test result: ok" /tmp/fsl-merge-$ID.log || { echo "Rust 测试没有全过，不合并" >&2; exit 1; }
! grep -q "FAILED" /tmp/fsl-merge-$ID.log || { echo "有失败的测试，不合并" >&2; exit 1; }
echo "== 3/4 前端：类型检查与测试"
tools/cards/run.sh web-build 2>&1 | tee /tmp/fsl-merge-$ID-build.log
tools/cards/run.sh web-test 2>&1 | tee /tmp/fsl-merge-$ID-web.log
! grep -qE "failed|FAIL" /tmp/fsl-merge-$ID-web.log || { echo "前端测试有失败，不合并" >&2; exit 1; }
echo "== 4/4 合并进集成线"
cd "$INT"
git diff --quiet HEAD || { echo "集成区有未提交的改动" >&2; exit 1; }
git merge --no-ff "card/$ID" -m "合并 卡${ID}：$MSG

Co-Authored-By: Codex <noreply@openai.com>"
git log --oneline -1
