# 交回报告 · 卡 26-full-scan-ui

## 一句话结论
做完了：工具箱里「全量整理 · 不限额」要先确认才开始，进行中能看进度和停止；全量整理期间今日、资料库、详情和重新整理都不再说「额度用完」。

## 验收逐条
| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| `tools/cards/run.sh web-test` 全部通过，`automation-view.test.ts` 新增测试**至少 16 个**，覆盖：`estimateFullScanTokens`、`roundedEstimate`（上面三个例子 + 0）、`fullScanPending`（有 / 缺 `needsOrganizing`）、`fullScanProgress`（正常、`remaining` 大于 `total`、`total` 为 0、`remaining` 为 0 → 100）。 | 是 | `tools/cards/run.sh web-test`：16 个文件、323 项全过（`automation-view.test.ts` 61 项）。原有 30 项未改，新增 31 项（`automation-view.test.ts:270` 起）。三个取整例子和 0 在 `:286`；`needsOrganizing` 有 / 缺 / null 在 `:302`、`:307`；进度四种在 `:316`–`:330`。 |
| `fullScanPanel` 的五种 `kind`，以及**顺序**：`fullScan` 不为 null 时即使 `providerConfigured` 为 false 也是 `running`；`unlimited === false` 但 `fullScan` 不为 null 时不是 `running`（后台说没在跑就是没在跑）。 | 是 | 五种 kind：`automation-view.test.ts:336`（loading）、`:340`（unavailable）、`:347`（nothing）、`:355`（ready）、`:365`（running）。服务没配好但仍 running：`:385`。`unlimited === false` 落到 ready：`:394`；再叠加没配服务落到 unavailable：`:411`。 |
| `lastFullScanText` 四种 `reason` + `null` + 认不出的 `reason`。 | 是 | `automation-view.test.ts:422` 四句与任务卡示例一致（完成 388 个 / 201.8 万；停止、到时间、用量超了都是 120 / 388、62 万）。`:437` 覆盖 `null`、`undefined` 和认不出的 `other`。 |
| `automationState` / `automationHint` / `itemPendingNote` / `regenerateNote` 在 `unlimited: true` 且 `budgetRemaining: 0` 时**都不出现「额度用完」**；`unlimited` 缺失（旧后台）时行为和以前完全一样（保留原有测试不改）。 | 是 | 四处都不出现「额度用完」：`automation-view.test.ts:455`–`:514`（含队列为 0）。原有 30 项一个字没改，仍全过。缺失字段仍走「额度用完」、排队文案仍含「整理队列」：`:537`。 |
| 暂停优先于 `unlimited`（`paused: true, unlimited: true` → `已暂停`）。 | 是 | `automation-view.test.ts:514`：`automationState` 标签是「已暂停」、色相 4；提示、详情、重新整理也都先说暂停。代码在 `automation-view.ts:231` 先判断 `paused`，`:234` 才判断 `unlimited`。 |
| `tools/cards/run.sh web-build` 通过（类型检查）。 | 是 | 退出码 0。`tsc --noEmit` 通过，随后 `vite build` 成功（2183 个模块，约 1.14 秒）。 |
| `git diff --stat` 只出现「允许改动」里的文件。 | 是 | 只有这 3 个：`automation-view.ts`（+200）、`automation-view.test.ts`（+309）、`toolbox.tsx`（+199）。`today.tsx`、`library.tsx`、`item-detail.tsx`、`automation-banner.tsx`、`use-regenerate.ts` 没改。 |
| 报告里给出：新增 / 改动的函数在 `automation-view.ts` 的行号；`toolbox.tsx` 里全量整理面板的起止行；你对「进行中」「暂停」两种状态怎么画的一句话说明；任何你觉得文案还不够清楚的地方（直接写出你的建议文案）。 | 是 | 行号和画法见下方「给审核者的提示」。 |

## 改了哪些文件
- `apps/desktop/src/lib/automation-view.ts`：补上全量整理的类型和纯函数；全量整理进行时，状态、顶上提示、详情、重新整理都不再走「额度用完」。
- `apps/desktop/src/lib/automation-view.test.ts`：新增 31 个测试，原有断言没改。
- `apps/desktop/src/pages/v2/toolbox.tsx`：在「AI 自动整理」里加上全量整理面板和「增量整理（日常）」；只有点「确认开始全量整理」才调用 `start_full_scan`。

## 验证
- `tools/cards/run.sh web-test` → 16 个文件、323 项全过，退出码 0。其中 `automation-view.test.ts` 61 项（原 30 + 新 31）。
- `tools/cards/run.sh web-build` → `tsc --noEmit` 与 `vite build` 都通过，退出码 0。
- `git diff --stat` → 只列出上面 3 个允许改动的文件。
- 没有启动 App（红线禁止 `pnpm dev:desktop` / `tauri dev`）。界面没有在运行中的窗口里点过，点按钮、确认、停止这条路径靠代码阅读，没有浏览器点击记录。

## 没做到的 / 偏离任务卡的地方
- 没有在运行中的界面里亲手点「开始全量整理 → 确认 → 停止」。原因：红线不许启动 App，也不许占 8765 / 39217。类型和单测盖住了文案和分支，盖不住像素和点击手感。
- `itemPendingNote` 的新句子「正在全量整理，整理好会自动出现在这里。……」只在 `unlimited === true` 时使用。日常排队（`unlimited` 缺失或 false，但队列里有货）仍是原来的「已经在整理队列里了」。任务卡括号写「queued + running > 0 或 unlimited 都用这条」；若日常排队也改成「正在全量整理」，原有测试 `toContain('整理队列')` 会失败，也和「旧后台行为完全一样」冲突。所以按更严的那条做。
- 日常额度的确认框开着时，「开始全量整理」暂时用普通按钮，不用主按钮。确认框一关，它仍是主按钮。原因：这页的按钮约定是同时只放一个主按钮；两个确认框也不会叠在一起（开一个会关掉另一个）。

## 需要总控或 Fox 决定的事
无

## 给审核者的提示
- 函数行号（`apps/desktop/src/lib/automation-view.ts`）：`FullScan` / `LastFullScan` 6–28 行；`AutomationStatus` 新字段 31–54 行；`FULL_SCAN_TOKENS_PER_REPO` 60 行；`estimateFullScanTokens` 99–104 行；`roundedEstimate` 110–116 行；`fullScanPending` 119–123 行；`fullScanProgress` 126–133 行；`lastFullScanText` 143–162 行；`FullScanPanel` 与 `fullScanPanel` 164–216 行。`automationState` 的全量分支 234–236 行（暂停仍在 231 行，更优先）。`automationHint` 的全量分支 277–286 行，额度用完改写成「去工具箱」在 296–303 行。`itemPendingNote` 327–333 行。`regenerateNote` 349–355 行。
- 全量整理面板：`toolbox.tsx` 的 `FullScanPanelView` 是 127–235 行，挂到页面上是 473–482 行。「增量整理（日常）」标题在 484 行。
- 进行中：标题右侧绿色小徽章「进行中」（色相 148），下面一条 `role="progressbar"` 的进度条（宽度等于百分比），一行「已整理 A / B 个 · 已用约 X token · 开始于 …」，一行小字写最长到何时，再一个次要按钮「停止全量整理」（点一下就停，没有第二次确认）。暂停：同一块面板仍保持进度和「进行中」徽章，额外加一行红字「现在是暂停状态（原因），点上面的「继续整理」才会接着做」；页顶的状态徽章这时是「已暂停」，「继续整理」按钮在面板上方。
- 今日 / 资料库的 `AutomationBanner`、详情里的 `PendingNote`、`useRegenerate` 的 toast，都是「只要有 `actionLabel` 就调用 `onOpenToolbox`」，不按旧文案过滤。所以新的「查看进度」「去工具箱」点下去仍然进工具箱，这四个文件不用改。
- `unlimited` 缺失但 `fullScan` 还在时：面板按任务卡写成 running（只有明确 `unlimited === false` 才不当在跑，见测试 `:403`）；顶上徽章仍要 `unlimited === true` 才写「全量整理中」。正常后台这两个字段会一起给。若要收紧成「缺了 `unlimited` 就绝不显示进行中」，把 `fullScanPanel` 里的 `unlimited !== false` 改成 `unlimited === true` 即可。
- 觉得还不够清楚的文案，建议（没改代码）：
  1. 全量整理进行中又被暂停时，小徽章仍写「进行中」，和顶上的「已暂停」叠在一起。建议小徽章改成「已暂停」，红字可以留着。
  2. 全量整理已开、但队列暂时是 0 时，今日 / 资料库顶上会落到「它们还没进整理队列」，像是没在整理。建议改成：「正在全量整理（不限额）。队列里这一批已经排完，整理完会自动回到日常整理。」按钮仍是「查看进度」。
