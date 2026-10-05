---
编号: 42-organize-page-polish
允许改动:
  - apps/desktop/src/lib/automation-view.ts
  - apps/desktop/src/lib/automation-view.test.ts
  - apps/desktop/src/pages/v2/toolbox.test.tsx
---

# 卡 42 · 自动整理页三处收尾：「第一步」的判断、24 小时用量的说明、整理花费的估算（前端）

> 状态：开发中（2026-10-05 派出）｜执行者：Codex｜只改前端，不动 Rust。

## 为什么有这张卡
发布 #11（卡 39 + 40 + 41）之后，总控用真数据读了一遍页面（开发版刚重启，后台把之前卡住的 5 个任务做完了），发现三处小问题。Fox 要的逻辑是：**开始的时候先把现有收藏整理一遍，之后才是增量 / 每天自动整理**，所以下面第 1 条尤其要紧。

1. **「第一步」的判断太脆。** 现在是「新版说明一个都没有才算第一步」（`counts.organized === 0`）。后台那 5 个任务一做完，新版说明变成 5 个，页面立刻从「第一步：把现有收藏整理一遍 / 现在整理全部 496 个」变成「现在就整理 496 个 / 现在整理 496 个」，「第一步」的说法凭空消失了。真实情况是 500 个收藏里 496 个还是旧版说明——这明明还是第一步。
2. **24 小时用量旁边没有说明。** 页面写「24 小时内整理了 465 个 · 已用约 266.0 万 token，含失败重试」，而每天额度只有 60 万，看起来像超了。后端现在有 `dailyTokensUsed`（卡 39：每天额度已用，不含「现在整理」那几轮），两个数的差就是整理现有收藏用的，可以如实说「其中约 261.8 万是整理现有收藏用的，不占每天额度」。现在只有「每天额度为 0 且上一轮整理在 24 小时内结束」才会说这句（`spentByEarlierFullScan`），额度开着时一句也不说。
3. **「预计约 320 万 token」估低了。** 前端按每个 6500 估；真实的 v3 说明（LongCat 实际输出，第一批 5 张）每张 6.4–8.9 千，平均约 7.3 千；后台刹车线用的是每张 7500（Rust `FULL_SCAN_TOKENS_PER_CARD`）。Fox 点「确认开始」是按这个数字决定花不花钱的，要和实际接近。

## 要做的事

### 1. 估算改 7500
`automation-view.ts`：`FULL_SCAN_TOKENS_PER_REPO` 从 `6500` 改成 `7500`，注释改成：「真实记录：v3 说明第一批 5 张平均约 7.3 千（含思考）；后台刹车线 `FULL_SCAN_TOKENS_PER_CARD` 也是 7500，两处保持一致。只用于这一轮的预计花费，不用于每天额度折算。」
`TOKENS_PER_REPO`（6000，每天额度「约还能整理 N 个」、额度档位的提示用）**不动**，`BUDGET_PRESETS` 也不动。

### 2. 「第一步」的判断：新版说明不到现有收藏的一半，就还是第一步
`fullScanPanel` 里现在是：

```ts
const firstStep = counts ? counts.organized === 0 : status.lastFullScan == null;
```

改成：分项齐全（`counts` 不是 null）时，**新版说明的个数不到现有收藏总数的一半**就算第一步——`counts.organized * 2 < counts.total`；分项不齐（旧后台）时仍然用 `status.lastFullScan == null`。第一步和「之后」的标题、说明、正文、按钮文案**都不改**。

边界要写进测试（`total` = 现有收藏总数，`organized = total − 旧版 − 没有`，不小于 0）：

| 现有收藏 | 没有说明 | 旧版说明 | 新版说明 | 结果 |
|---|---|---|---|---|
| 500 | 0 | 496 | 4 | 第一步（这就是真实情况） |
| 12 | 0 | 7 | 5 | 第一步（5×2=10 < 12） |
| 12 | 0 | 6 | 6 | 之后（6×2=12，不小于 12） |
| 520 | 20 | 0 | 500 | 之后（同步来了 20 个新收藏） |
| 12 | 10 | 10 | 0（合计超过总数，按 0 算） | 第一步 |

分项不齐时：没有 `lastFullScan`（`null` 或缺失）→ 第一步，有 → 之后（和现在一样，已有测试守着）。

### 3. 24 小时用量那一句：有后端证据就说清楚「其中多少不占每天额度」
- `RecentUsage` 类型（`Pick<AutomationStatus, …>`）加上 `'dailyTokensUsed'`。
- 新增并导出一个小函数（名字自己起，建议 `fullScanSpendIn24h`）：**`tokens24h − dailyTokensUsed`**，只有同时满足才返回差值，否则返回 0：`tokens24h` 和 `dailyTokensUsed` 都是有限数、`dailyTokensUsed ≥ 0`、差值 ≥ 10,000（不到 1 万的差不说，避免噪声）。`dailyTokensUsed` 缺失 / null / NaN / 负数 / 比 `tokens24h` 还大，一律返回 0（旧后台不会冒出这句）。**不要**因为 `unlimited === true` 就放弃这句：整理进行中，后端的 `dailyTokensUsed` 同样不含这一轮，差值照样是真数。
- `recentUsageLine` 和 `recentUsageMetric` 里选小字的顺序：先看原来的 `spentByEarlierFullScan`（成立就用原来的 `EARLIER_FULL_SCAN_NOTE`，**原文一个字不改**，已有测试全部保持原样）；不成立时，差值 > 0 就用新句：`其中约 ${formatTokens(差值)} 是整理现有收藏用的，不占每天额度`；都不成立就和现在一样，不加小字。
- 新句放的位置和旧句一样：`recentUsageLine` 有个数时 `（已用约 X token，含失败重试；其中约 Y 是整理现有收藏用的，不占每天额度）。`，没有个数时 `最近 24 小时已用约 X token（其中约 Y 是整理现有收藏用的，不占每天额度）。`；`recentUsageMetric` 有个数时 `sub` 是 `已用约 X token，含失败重试；其中约 Y 是整理现有收藏用的，不占每天额度`，没有个数时 `sub` 就是 `其中约 Y 是整理现有收藏用的，不占每天额度`。
- `spentByEarlierFullScan` 函数本身、`EARLIER_FULL_SCAN_NOTE` 都不改。

### 4. 既有测试里按旧数字断言的，换成新数字（只换期望值，不放宽、不删、不跳过）
- `estimateFullScanTokens` 那组（`automation-view.test.ts` 约 333 行）：`FULL_SCAN_TOKENS_PER_REPO` 是 7500，388 个 = 2,910,000，1 个 = 7500，1.2 个 = 9000；测试名里的 6500 说明改成 7500 和上面的新依据。
- `fullScanPanel` 里用 12 个算出「预计约 8 万」的两处（约 530 行、551 行）：12 × 7500 = 90,000 → 「9 万」（`body` 和 `estimate` 都要改）；3 个的那处仍是「2 万」。
- 约 546 行「有新版说明，或分项缺失但有上次记录，显示后续手动整理」那组：第一条 `{ activeRepos: 12, needsOrganizingMissing: 2, needsOrganizingOutdated: 9, lastFullScan: null }` 的新版说明只有 1 个（1×2 < 12），按新规则是第一步，会失败。把这一条换成一个新版说明过半的例子（例如 `{ activeRepos: 12, needsOrganizingMissing: 2, needsOrganizingOutdated: 3, lastFullScan: null }`，新版 7 个），并在报告里点名这是**有意改的期望**；把上面表格里的五种边界另写成新的 `it.each`。
- 其他因为 12 × 7500 或估算变化而失败的断言，同样只换数字，报告里逐条点名。

### 5. 新增测试（每一条都要能「改坏就失败」）
- 第 2 条表格里的五种边界，各自断言 `title` 和 `buttonLabel`；旧后台两种（没有 `lastFullScan` / 有 `lastFullScan`）保留。
- 第 3 条：
  - 差值刚好 10,000 → 有小字；9,999 → 没有；
  - `dailyTokensUsed` 缺失、null、NaN、-1、比 `tokens24h` 大 → 没有；
  - 额度开着（`dailyBudgetTokens: 600000`）、`tokens24h: 2_660_000`、`dailyTokensUsed: 42_000`、`succeeded24h: 465`：`recentUsageLine` 和 `recentUsageMetric` 的整句 / 整个对象逐字断言（数字格式沿用 `formatTokens`，例如 261.8 万那一句，自己按 `formatTokens(2_618_000)` 核对写出来）；没有个数时也各断言一条；
  - `unlimited: true` 且差值足够 → 仍然有小字；
  - 原来的「额度为 0 且上一轮整理 24 小时内结束」那些测试保持原样、全部仍然通过（旧句优先）。
- 页面源码守卫（`toolbox.test.tsx`）如果有地方按「预计约」「第一步」逐字守着，核对一下没被你这次改动弄坏；不需要新增守卫。

## 不要做
- 不要改 Rust、命令、依赖、后端字段名。
- 不要改 `TOKENS_PER_REPO`、`BUDGET_PRESETS`、`MIN_DAILY_BUDGET`、「第一步」和「之后」的标题 / 说明 / 正文 / 按钮文案、确认三句话、额度用完的文案、`spentByEarlierFullScan` 和 `EARLIER_FULL_SCAN_NOTE`。
- 不要改 `organizeCounts` / `fullScanPending` / `fullScanProgress` 的判断（`organizeCounts` 里 `organized` 怎么算保持原样，只在 `fullScanPanel` 里用它）。
- 不要启动应用、不要碰数据目录和钥匙串。

## 验收
1. `tools/cards/run.sh web-build`、`web-test` 通过。
2. `git diff --stat` 只出现「允许改动」里的文件。
3. 报告写：条号 → 文件:行；改了哪些既有测试的期望（逐条点名，说明为什么）；新增测试名；没把握的地方。

## 停止条件
- 需要改「允许改动」以外的文件，或需要后端新字段：停手，写进报告。
