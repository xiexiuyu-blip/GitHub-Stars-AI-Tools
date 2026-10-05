---
编号: 38-readiness-rows
允许改动:
  - apps/desktop/src/pages/settings.tsx
  - apps/desktop/src/types.ts
  - apps/desktop/src/lib/automation-view.ts
  - apps/desktop/src/lib/automation-view.test.ts
  - apps/desktop/src/pages/settings.test.tsx
---

# 卡 38 · 运行检查：去掉两行「已停止检查」占位（前端）

## 为什么有这张卡
卡 37 去掉了「运行检查」里「相似推荐」「标签网络」两项 AI 探测（对应的功能页面在新界面里已经没有入口，每点一次检查都白花 token）。但设置页还按这两个键各画一行，并把它们计进小结里的「没检查 N 项」。卡 37 不允许改设置页，只好让这两行显示成两行同名的「已停止检查」。这张卡把它们从界面里拿掉。**只改前端显示。**

## Fox 能看到的效果
- 设置里点「运行检查」，结果只有六行：资料库文件、应用设置、GitHub 令牌、读取我的收藏、下载项目说明、AI 服务；
- 「检查结果小结」里的「通过 / 没通过 / 没检查」只算这六项，不再永远多出「没检查 2 项」。

## 背景与先读什么
- `apps/desktop/src/pages/settings.tsx`：约 783–790 行（八行 `RuntimeCheckRow`，最后两行是 `tagNetwork`、`recommendation`）；约 815–830 行 `summarizeRuntimeCheck`（`items` 数组里也有这两项）。
- `apps/desktop/src/types.ts` 约 311–320 行：`RuntimeReadinessCheckResult` 里的 `tagNetwork`、`recommendation`。
- `apps/desktop/src/lib/automation-view.ts` 约 725–733 行：`RUNTIME_CHECK_TITLES`，最后两个键的值都是「已停止检查」。
- `apps/desktop/src/lib/automation-view.test.ts` 约 1055–1080 行：断言了这个对象的全部键。
- 后端（`lib.rs` 的 `RuntimeReadinessCheckResult`）仍会回传 `tagNetwork` / `recommendation` 两个键（固定「没检查」）。**不改 Rust**：前端不再读它们就行，多出来的键无害。
- 「读页面源码原文」的守卫测试有先例：`components/library/map-page.test.tsx`、`today-page.test.tsx` 里用 `import SOURCE from '@/pages/v2/xxx.tsx?raw'`，照这个写法给设置页写一个（不用 node 的 `fs`）。

## 要做的事
1. `settings.tsx`：删掉 `tagNetwork`、`recommendation` 那两行 `RuntimeCheckRow`；`summarizeRuntimeCheck` 的 `items` 去掉这两项。
2. `types.ts`：`RuntimeReadinessCheckResult` 去掉这两个键。
3. `automation-view.ts`：`RUNTIME_CHECK_TITLES` 去掉这两个键（以及只为它们写的注释）；其余六项名字、`RUNTIME_CHECK_COST_NOTE` 都不变。
4. `automation-view.test.ts`：把对这个对象的断言改成只有六个键、六个名字；保留「不再出现旧叫法 `AI 打标签` / `AI 找相似工具`」那条断言。
5. 新增 `apps/desktop/src/pages/settings.test.tsx`（如果已经存在同名文件就往里加）：用 `?raw` 读 `settings.tsx` 的源码，断言 ① 里面不再出现 `tagNetwork`、`result.recommendation`、`runtimeCheck.recommendation`、`已停止检查`；② 六个 `RUNTIME_CHECK_TITLES.xxx` 的行都还在。注意 `settings.tsx` 里有与本卡无关的 `tagNetworkBatchSize`、`tagNetworkPromptVersion`（设置项），不要把它们当成要删的东西，断言要写得不误伤它们。
6. 在 `apps/desktop/src` 下 `grep` 一遍 `tagNetwork`、`recommendation`（用户看得见的地方）：允许范围内的处理掉，范围外的（比如 `pages/tag-network.tsx`、设置项）只在报告里列出来，不改。

## 不要做
- 不改 Rust、后端字段、命令名、提示词；不新增依赖；不碰卡 36 已经带上的「导出」说明（它已经写了「用 Excel 打开中文不会乱码」）。
- 不放宽、不删现有断言（期望改成新状态是对的）。

## 验收
1. `tools/cards/run.sh web-build`、`web-test` 通过（`web-test` 在沙盒里跑不了时，说明原因，总控在沙盒外跑）。
2. `git diff --stat` 只出现「允许改动」里的文件。
3. 报告里写：条号 → 文件:行；第 6 条 `grep` 的输出里范围外的残留清单；没把握的地方。

## 停止条件
- 要改 Rust 或「允许改动」以外的页面才能做完——停手并写进报告。
