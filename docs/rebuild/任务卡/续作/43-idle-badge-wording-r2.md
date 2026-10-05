# 卡 43 · 第 2 轮交接（只补 2 条测试，生产代码一个字不动）

第 1 轮的实现和测试我（总控）核对过：生产 diff 和任务卡一字不差，851 项全过。我另外对 `automationState` 做了 26 种「改坏实现」检查：**24 种被测试抓住，2 种没抓住**——

- **等价改动（不用管）**：把 `fullScanPending(status)` 换成「少一级回退」的写法，结果不变，因为走到这一支时 `queued + running` 一定是 0。
- **真缺口（这一轮要补）**：把新分支的条件从 `if (pending > 0) {` 改成 `if (pending > 0 && status.lastFullScan == null) {`，**851 项测试仍然全过**。

这个场景是真实会发生的：现有收藏整理完一轮后，有几个整理失败、还剩几个没整理好，后台又没排队——这时 `lastFullScan` 是有值的，徽章仍然要说「已开启，后台暂时没有排队的」并指向「现在整理」，不能退回「暂时没有要整理的」；反过来，没有剩余时即使有上次整理记录，也要保持原来的待命文案。第 1 轮新增的测试里 `lastFullScan` 全都没给，守不住这两点。

## 要做的事
只在 `apps/desktop/src/lib/automation-view.test.ts` 的 `describe('automationState', …)` 里**再补 2 条测试**（紧挨着第 1 轮新增的那几条，用文件里已有的 `lastScan()` 夹具，约 395 行；`reason` 用 `'completed'`，`failed` 可以给 3）：

1. 有上次整理记录、`awaiting: 3`、后台没排队：逐字断言整个对象——`label` 是 `已开启，后台暂时没有排队的`，`hue` 是 148，`detail` 是 `现有收藏里还有 3 个说明要整理，点下面的「现在整理」一次做完；新收藏、或你点了「加入整理」的，会自动开始。`
2. 有上次整理记录、`awaiting: 0`、后台没排队：`label` 和 `detail` 逐字断言还是原来那句待命文案（`已开启，暂时没有要整理的` / `有新收藏，或者你点了「加入整理」，就会自动开始。`）。

## 不要做
- 不要改 `automation-view.ts`（第 1 轮那 8 行生产代码一个字不动）、不要改第 1 轮已有的任何测试、不要动「允许改动」以外的文件。
- 不要启动应用、不要碰数据目录和钥匙串。

## 验收
1. `tools/cards/run.sh web-build`、`web-test` 通过（改前 851 项，只增不减，应该是 853）。
2. `git diff --stat` 仍然只有 `automation-view.ts` 和 `automation-view.test.ts`；`automation-view.ts` 的 diff 和第 1 轮完全一样（+8 行）。
3. 你自己做一次「改坏验证」写进报告：临时把 `if (pending > 0) {` 改成 `if (pending > 0 && status.lastFullScan == null) {`，应该有测试失败；再临时改成 `if (pending > 0 || status.lastFullScan != null) {`，也应该有测试失败；验完恢复原样，`git diff` 里不能留下这类改动。
4. 报告在第 1 轮报告基础上补一节「第 2 轮」：新增的 2 条测试名和 文件:行，以及上面两次改坏验证的结果。
