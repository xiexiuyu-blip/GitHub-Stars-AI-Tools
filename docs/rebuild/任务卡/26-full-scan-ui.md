---
编号: 26-full-scan-ui
允许改动:
  - apps/desktop/src/lib/automation-view.ts
  - apps/desktop/src/lib/automation-view.test.ts
  - apps/desktop/src/pages/v2/toolbox.tsx
  - apps/desktop/src/pages/v2/today.tsx
  - apps/desktop/src/pages/v2/library.tsx
  - apps/desktop/src/components/library/automation-banner.tsx
  - apps/desktop/src/components/library/item-detail.tsx
  - apps/desktop/src/hooks/use-regenerate.ts
  - apps/desktop/src/hooks/use-ai-automation.ts
---

# 卡 26 · 工具箱：「全量整理（不限额）」和「增量整理（日常）」的界面

## 为什么有这张卡
Fox（品牌营销，不是开发者）2026-10-04 上午试用后的原话：「限制每日的 token 这个很不好，我急用全量扫描的时候完全无法跑啊，最起码留一个全量不限制 token 的选项；同时后续可以有增量整理的功能，开始肯定要全量整理啊，后续可以增量节约 token。」

现在的情况：每天额度（默认 10 万 token）是硬上限，约 25 张理解卡 / 天；他有 388 个收藏排队等整理，按这个速度要 17 天。总控（Claude）**同时**在后台做两件事（你不要做，只按下面的「后台契约」写界面）：
1. **全量整理（不限额）**：Fox 点一下并确认之后，这一轮不受每天额度限制，一直整理到全部完成（或最长 48 小时、或他点「停止」），然后**自动回到日常模式**。
2. **增量整理（日常）**：就是现在已有的「每天额度」模式——只整理新收藏的、还没整理的、说明过期的，没变的不会重复花钱。界面上要把它说成「增量整理」，让 Fox 明白「先全量、以后增量省 token」。

## Fox 能看到的效果
- 工具箱「AI 自动整理」里，最显眼的位置是一块「全量整理 · 不限额」：写明还有多少个没整理、预计花多少 token；点「开始全量整理」，先出一个**必须确认**的提示（说清楚花钱、不限额、随时能停），确认后开始。
- 进行中显示进度条：「已整理 120 / 388 个 · 已用约 62 万 token」和「停止全量整理」按钮；整理完自动变回日常模式，并写一句「上次全量整理：……完成，用了约 X token」。
- 全量整理期间，今日 / 资料库顶上的提示、详情里的「还没有说明」都不能再说「今天的额度用完了」。
- 下面是「增量整理（日常）」：原来的额度档位和自定义额度，加一句说明。

## 后台契约（总控实现，你按这个写，不要改 Rust）
`get_ai_automation` 返回的 `AutomationStatus` **新增**这些字段（驼峰；**后台还没合并时字段可能缺失，界面必须容错**：缺失当作 `unlimited=false`、`fullScan=null`、`lastFullScan=null`、`needsOrganizing` 缺失时用 `queued + running` 代替）：

```ts
export type FullScan = {
  startedAt: string;      // ISO 时间（UTC，带 Z）
  until: string;          // 兜底截止（开始后 48 小时），到点自动结束
  tokensUsed: number;     // 开始以来已用 token（含在途的预留）
  total: number;          // 开始时要整理的个数
  remaining: number;      // 现在还剩（排队 + 进行中）
};
export type LastFullScan = {
  startedAt: string;
  finishedAt: string;
  tokensUsed: number;
  reason: 'completed' | 'stopped' | 'expired' | 'ceiling';  // 整理完 / 他点了停止 / 到 48 小时 / 用量远超预计被自动停下
  total: number;
  remaining: number;      // 结束时还剩几个没整理
};
// AutomationStatus 里新增：
unlimited: boolean;       // 全量整理正在进行（= fullScan 不为 null）
fullScan: FullScan | null;
lastFullScan: LastFullScan | null;
needsOrganizing: number;  // 现在还没整理好（没有理解卡 + 说明过期）的收藏数，用来估计全量整理要多少
```
- 全量整理中 `budgetRemaining` 仍然是「**日常额度**还剩多少」，**不代表队列会不会停**；界面凡是用到 `budgetRemaining <= 0` 的地方，都要**先看 `unlimited`**。
- `enabled` 在全量整理期间为 `true`（即使日常额度是 0）。
- 命令（`invoke`）：
  - `start_full_scan`，参数 `{ confirmed: true }` → 返回最新的 `AutomationStatus`。失败会抛出人话（比如「还没配置 AI 服务」「没有需要整理的仓库了」），用 `describeInvokeError` 显示。
  - `stop_full_scan`，无参数 → 返回最新的 `AutomationStatus`。
  - 命令还没合并时调用会报「这个功能还没接上」（`describeInvokeError` 已处理），照常用 toast 显示。

## 背景与先读什么
- `apps/desktop/src/lib/automation-view.ts`：纯函数（`automationState`、`automationHint`、`itemPendingNote`、`regenerateNote`、`formatTokens`、`approxRepos`……）和类型；`automation-view.test.ts` 是它的单测。**逻辑尽量放在这里（纯函数 + 单测），页面只负责画。**
- `apps/desktop/src/pages/v2/toolbox.tsx`：「AI 自动整理」那一块（约第 280–400 行）。沿用页面里已有的 `Section`、`Card`、`Button`、`Notice`、`Metric`、`useToast` 和 Tailwind 写法；确认面板照现有的 `pendingBudget` 那块（蓝边、`bg-accent`）。
- 用到 `AutomationStatus` 的其它地方：`pages/v2/today.tsx`、`pages/v2/library.tsx`（`automationHint`）、`hooks/use-regenerate.ts`（`regenerateNote`）、`components/library/item-detail.tsx`（`itemPendingNote`）、`components/library/automation-banner.tsx`。`grep -rn "budgetRemaining\|automationHint\|automationState\|itemPendingNote\|regenerateNote" apps/desktop/src` 把所有用到的地方过一遍。
- 视觉：Fox 嫌过「一堆文字没有视觉重点」。全量整理那一块要**一眼看出这是主动作**（有标题、一句话说明、一个主按钮、进行中有进度条），别写成一大段字。

## 要做的事

### 1. `automation-view.ts`
1. 在 `AutomationStatus` 里加上面的新字段（`unlimited`、`fullScan`、`lastFullScan`、`needsOrganizing` 都写成**可选**：`unlimited?: boolean` 等，读的时候用 `status.unlimited === true`），并导出 `FullScan`、`LastFullScan` 类型。
2. 常量与纯函数（都要单测）：
   - `FULL_SCAN_TOKENS_PER_REPO = 5200`（实测一张理解卡约 5 千 token，含思考）。
   - `estimateFullScanTokens(count: number): number`：`count * 5200`，向最接近的整数取整；`count` 不是有限数或 ≤ 0 返回 0。
   - `roundedEstimate(tokens: number): string`：给人看的「约」数——≥ 100 万按 10 万取整，否则按 1 万取整（至少 1 万），再用 `formatTokens` 格式化。例：`2_017_600 → '200 万'`，`2_065_000 → '210 万'`，`35_000 → '4 万'`（不用特意处理半个万，沿用 `Math.round`），`0 → '0'`。
   - `fullScanPending(status): number`：`status.needsOrganizing ?? (status.queued + status.running)`，不是有限数或 < 0 当 0。
   - `fullScanProgress(scan: FullScan): { done: number; total: number; percent: number }`：`done = max(0, min(total, total - remaining))`；`total` ≤ 0 时 `percent = 0`；`percent` 是 0–100 的整数（`Math.floor`）；`remaining` 为 0 时一定是 100。
   - `fullScanPanel(status: AutomationStatus | null): FullScanPanel`，类型：
     ```ts
     export type FullScanPanel =
       | { kind: 'loading' }
       | { kind: 'unavailable'; reason: string }
       | { kind: 'nothing'; last: string | null }
       | { kind: 'ready'; pending: number; estimate: string; last: string | null }
       | { kind: 'running'; done: number; total: number; percent: number; tokensUsed: number; startedAt: string; until: string; paused: boolean; pausedReason: string | null };
     ```
     规则（**按这个顺序**判断）：`status` 为 null → `loading`；`status.fullScan` 不为 null 且 `unlimited !== false` → `running`（`paused` 取 `status.paused`，`pausedReason` 取 `status.pauseReason?.message ?? null`）；`!providerConfigured` → `unavailable`，`reason` 是 `'先到「设置 → 引擎」里选好服务商和模型、保存密钥，才能整理。'`；`fullScanPending(status) <= 0` → `nothing`；否则 `ready`（`estimate = roundedEstimate(estimateFullScanTokens(pending))`）。`last` 是 `lastFullScanText(status.lastFullScan)`（见下）。
   - `lastFullScanText(last: LastFullScan | null | undefined): string | null`，用 `formatMinute(finishedAt)` 和 `formatTokens`：
     - `completed`：`上次全量整理：2026-10-04 12:30 完成，整理了 388 个，用了约 201.8 万 token。`（整理了 `total - remaining` 个）
     - `stopped`：`上次全量整理：2026-10-04 12:30 被停止，已整理 120 / 388 个，用了约 62 万 token。`
     - `expired`：`上次全量整理：2026-10-04 12:30 到时间自动结束，已整理 120 / 388 个，用了约 62 万 token。`
     - `ceiling`：`上次全量整理：2026-10-04 12:30 因为用量远超预计，已自动停下（已整理 120 / 388 个，用了约 62 万 token）。要继续可以再开一次。`
     - 没有 / 认不出的 `reason` → `null`。
3. 改现有函数（**全量整理期间不能再说额度用完**）：
   - `automationState`：`providerConfigured` 之后、`paused` 之前不变（暂停仍然优先）；在 `paused` 之后、`!enabled` 之前加：`unlimited === true` → `{ label: '全量整理中（不限额）', hue: 148, detail: '在后台连续整理，不受每天额度限制；整理完会自动回到日常的增量整理。' }`。
   - `automationHint(status, pending)`：`unlimited === true` 且 `queued + running > 0` → `{ hue: 148, icon: 'autorenew', title: '正在全量整理（不限额），还剩 N 个', detail: '不受每天额度限制，已用约 X token。整理完会自动回到日常整理。', actionLabel: '查看进度' }`（N 用传入的 `pending`，X 用 `formatTokens(status.fullScan?.tokensUsed ?? status.tokens24h)`）；放在 `paused` 判断之后、`!enabled` 之前。原来「今天的整理额度用完了」那条（`budgetRemaining <= 0`，且**不是** unlimited）改成：`title: '今天的整理额度用完了'`，`detail: '还有 N 个没整理，明天会自动接着做；想现在就整理完，可以在工具箱里开启「全量整理」（不限额）。'`，`actionLabel: '去工具箱'`。
   - `itemPendingNote`、`regenerateNote`：`unlimited === true` 时不能走「额度用完」分支；`itemPendingNote` 给 `{ text: '正在全量整理，整理好会自动出现在这里。先看 GitHub 上的原文：', actionLabel: null }`（`queued + running > 0` 或 unlimited 都用这条）；`regenerateNote` 给 `{ title: '已加入整理队列', sub: '正在全量整理，会在后台重新整理，好了会自动更新', actionLabel: null }`。「今天的额度用完了」那两条的文案里顺手加一句「也可以在工具箱里开启全量整理」。
4. `BUDGET_PRESETS` 的 `hint` 不用改。

### 2. `toolbox.tsx`（「AI 自动整理」一块）
结构从上到下：
1. 标题区、状态徽章、状态说明、「继续整理」按钮：**保持**。
2. **新增「全量整理 · 不限额」面板**（`Card` 或带边框的块，标题 `全量整理 · 不限额`，副标题一句：`第一次用、想一口气把全部收藏整理完，就用它。这一轮不受每天额度限制，整理完自动回到日常的增量整理。`）。内容按 `fullScanPanel(automation)` 的 `kind` 画：
   - `loading`：Spinner + 「读取中」。
   - `unavailable`：灰字 `reason`，**没有按钮**。
   - `nothing`：`全部收藏都整理好了。以后新收藏会在日常的增量整理里自动处理。` + 有 `last` 就在下面写一行小字。
   - `ready`：一句 `还有 {pending} 个收藏没整理，预计要用 约 {estimate} token（按每个约 5 千估算，实际会上下浮动）。`；主按钮 `开始全量整理`；有 `last` 就写小字。点按钮**不直接开始**，先展开确认面板（`pendingFullScan` 状态；**默认关闭**），文案（可微调措辞，但这几点必须有）：
     > 这会让应用在后台**连续调用你配置的 AI 服务**，一直整理到这 {pending} 个收藏全部完成，**不受每天额度限制，按服务商的价格计费**。预计用 **约 {estimate} token**（按每个约 5 千估算，实际会上下浮动）。
     > 这期间旧版的「增量 AI 解析」「生成标签网络」「AI 搜索」等功能也不受每天额度限制。最长运行 48 小时，随时可以点「停止全量整理」；连续失败时会自动暂停。整理完会自动回到日常的增量整理。

     按钮 `确认开始全量整理`（primary）和 `取消`。确认后 `invoke('start_full_scan', { confirmed: true })`，成功用返回值 `applyAutomation`、toast `全量整理已开始`；失败 toast `errorText(reason)`；调用期间按钮 `loading` 且禁用，防止重复点。
   - `running`：标题行右侧小徽章 `进行中`（绿，hue 148）；一条进度条（有 `role="progressbar"`、`aria-valuenow` 等；宽度 = percent）；一行 `已整理 {done} / {total} 个 · 已用约 {formatTokens(tokensUsed)} token · 开始于 {formatMinute(startedAt)}`；一行小字 `最长到 {formatMinute(until)} 自动结束；随时可以停止。`；`paused` 为 true 时再加一行红字 `现在是暂停状态{pausedReason ? `（${pausedReason}）` : ''}，点上面的「继续整理」才会接着做。`；按钮 `停止全量整理`（次要按钮；点一下就停，**不需要二次确认**——停止是安全的），`invoke('stop_full_scan')` → `applyAutomation` → toast `已停止全量整理，剩下的会按每天额度接着整理`。
3. **「增量整理（日常）」**：把原来的「每天最多整理多少」那一小节加上标题 `增量整理（日常）`（`b` 加粗，和上面的面板同一层级的小标题）和一句说明：`全量整理完以后用它：只整理新收藏的、还没整理的和说明过期的，没变的不会重复花钱。每天最多用你设定的额度，用完第二天自动继续。`；额度档位、确认面板、自定义额度**照旧**。`unlimited` 为 true 时在档位上方加一行小字 `全量整理进行中，这一轮不受每天额度限制；整理完才按下面的额度。`，档位按钮仍可点。
4. 三个指标（Metric）：`24 小时内还剩` 在 `unlimited` 时显示值 `不限额`、副文案 `全量整理中`；`排队中` 不变。
5. 最下面一排按钮：`给还没有说明的仓库排队` 改名 `整理新增和还没整理的仓库`；`把旧版说明排队重做` 不变；排队后的提示文案 `已排队 N 个。是否开始整理，取决于上面的每天额度。` 在 `unlimited` 时改成 `已排队 N 个，全量整理正在处理。`。

### 3. 其它页面
`today.tsx`、`library.tsx`、`item-detail.tsx`、`use-regenerate.ts`、`automation-banner.tsx` 只在需要时改（比如点提示按钮的跳转文案）；**它们用到的纯函数已经在第 1 步处理了**，通常不用动。如果 `automationHint` 的 `actionLabel` 有新值（`'查看进度'`、`'去工具箱'`），确认点击后仍然是跳到工具箱（读一下现有 `onClick` 怎么处理 actionLabel，别写成只认旧的几个文案）。

## 不要做
- 不改 Rust、不改迁移、不改 `lib/api.ts`（工具箱里直接 `invoke`，和现有写法一致）、不改契约文档。
- 不新增依赖、不加动画库；进度条用普通 div。
- **不要在界面里自动开始全量整理**：只有用户点了「确认开始全量整理」才调用 `start_full_scan`。
- 不改 `BUDGET_PRESETS` 的数值、不改 `MIN_DAILY_BUDGET`、不放宽 `parseDailyBudget`。
- 不在页面里写复杂逻辑：判断都放进 `automation-view.ts` 的纯函数，页面只按 `kind` 画。

## 验收
1. `tools/cards/run.sh web-test` 全部通过，`automation-view.test.ts` 新增测试**至少 16 个**，覆盖：
   - `estimateFullScanTokens`、`roundedEstimate`（上面三个例子 + 0）、`fullScanPending`（有 / 缺 `needsOrganizing`）、`fullScanProgress`（正常、`remaining` 大于 `total`、`total` 为 0、`remaining` 为 0 → 100）。
   - `fullScanPanel` 的五种 `kind`，以及**顺序**：`fullScan` 不为 null 时即使 `providerConfigured` 为 false 也是 `running`；`unlimited === false` 但 `fullScan` 不为 null 时不是 `running`（后台说没在跑就是没在跑）。
   - `lastFullScanText` 四种 `reason` + `null` + 认不出的 `reason`。
   - `automationState` / `automationHint` / `itemPendingNote` / `regenerateNote` 在 `unlimited: true` 且 `budgetRemaining: 0` 时**都不出现「额度用完」**；`unlimited` 缺失（旧后台）时行为和以前完全一样（保留原有测试不改）。
   - 暂停优先于 `unlimited`（`paused: true, unlimited: true` → `已暂停`）。
2. `tools/cards/run.sh web-build` 通过（类型检查）。
3. `git diff --stat` 只出现「允许改动」里的文件。
4. 报告里给出：新增 / 改动的函数在 `automation-view.ts` 的行号；`toolbox.tsx` 里全量整理面板的起止行；你对「进行中」「暂停」两种状态怎么画的一句话说明；任何你觉得文案还不够清楚的地方（直接写出你的建议文案）。

## 停止条件
- 需要改 Rust、`lib/api.ts` 或允许范围外的文件；或发现契约里某个字段无法满足验收——停手并写进报告，不要绕。
