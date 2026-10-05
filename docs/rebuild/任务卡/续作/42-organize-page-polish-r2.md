# 卡 42 续作（第 2 轮）：新句子加上单位、补 1 处测试漏洞（总控，2026-10-05）

背景：你第 1 轮的结果，总控已核对：范围检查通过；完整前端测试总控自己跑了一遍，**835 项全过**；然后对 `automation-view.ts` 做了 38 种「改坏实现」检查（每种把一处实现故意改坏、再跑你写的测试，看会不会发现）：**35 种被测试抓住**；另有 3 种改坏之后测试仍然全绿——其中 2 种（去掉 `dailyTokensUsed == null ||`、去掉 `!Number.isFinite(dailyTokensUsed) ||`）是「改了结果也不变」的等价改动（`Number.isFinite(null / undefined / NaN)` 本来就是 false，`±Infinity` 会被后面的负数检查和差值门槛挡住），**不处理**；还有 **1 处是真漏洞**（下面第 2 条）。另外有 1 处措辞要改（第 1 条）。

这一轮只做这两件事，**其余全部保留**：你第 1 轮的实现、测试、数字、测试名一个字都不要重做、不要改写（除了下面明说要改的）。

允许改动：`apps/desktop/src/lib/automation-view.ts`、`apps/desktop/src/lib/automation-view.test.ts`（和任务卡一致，不多不少）。

## 1. 新句子加上单位「token」

现在的新句（`automation-view.ts` 里 `recentUsageNote`）：

```
其中约 ${formatTokens(spent)} 是整理现有收藏用的，不占每天额度
```

实际显示成「其中约 261.8 万 是整理现有收藏用的，不占每天额度」——「万」和「是」之间那个空格是总控写任务卡时模板里带的，读起来别扭，也没说 261.8 万是什么。改成：

```
其中约 ${formatTokens(spent)} token 是整理现有收藏用的，不占每天额度
```

即显示成「其中约 261.8 万 token 是整理现有收藏用的，不占每天额度」（和页面上其他地方「约 266 万 token」同一种写法）。

- 只改 `recentUsageNote` 里这一句的模板。旧句 `EARLIER_FULL_SCAN_NOTE`（「这是之前整理现有收藏用的，不占每天额度」）、`spentByEarlierFullScan`、选句的先后顺序都**不动**。
- 测试里所有逐字写出新句的地方同步改成新句：`recentUsageLine` / `recentUsageMetric` 的整句、整个对象的断言、`toContain(note)` 里的 `note` 常量，以及「差值刚好 1 万」那组里的两条字符串（「其中约 1 万 token 是整理现有收藏用的，不占每天额度」）。测试名里如果写了旧措辞也一并改。报告里列出你改了哪几处断言（行号）。

## 2. 补测试：每天用量远远大于总用量时也不说

**改坏版本**：`fullScanSpendIn24h` 里把 `return spent >= 10_000 ? spent : 0;` 改成 `return Number.isNaN(spent) ? 0 : Math.abs(spent) >= 10_000 ? Math.abs(spent) : 0;`（差值是很大的负数时，把它当成正的）。现在所有测试仍然全绿。

**为什么没抓住**：现有「每天用量比总用量大」的用例只有 `dailyTokensUsed: 40_001`（总用量 40,000），差值是 −1，绝对值不到 1 万，两种写法都返回 0。真实数据里两个数不是同一时刻读的，每天用量比总用量大很多也可能出现，必须是 0。

**要做**（只追加用例，不改已有的）：

- `fullScanSpendIn24h` 那组「每天用量缺失或不可用时不推测整理花费」的 `it.each` 列表里追加一条 `{ dailyTokensUsed: 100_000 }`（`tokens24h` 是 40,000，差值 −60,000，期望 0）。
- `recentUsageLine / recentUsageMetric` 那组「没有可靠的每天用量时，整句和指标均不加新小字」的 `it.each` 列表里同样追加 `{ dailyTokensUsed: 100_000 }`。

## 验收
1. `tools/cards/run.sh web-build`、`web-test` 通过；用例总数应该比第 1 轮多 2 项（835 → 837）。
2. `git diff --stat` 只出现上面两个文件。
3. 报告（`.card-report.md`，覆盖写）保留第 1 轮报告的结构，补一节「第 2 轮」：改了哪几处断言（行号）；新增了哪两条用例；没把握的地方。
