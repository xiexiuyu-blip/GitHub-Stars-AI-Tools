---
编号: 43-idle-badge-wording
允许改动:
  - apps/desktop/src/lib/automation-view.ts
  - apps/desktop/src/lib/automation-view.test.ts
---

# 卡 43 · 自动整理页：状态徽章在「后台没排队、但还有收藏没整理好」时别再说「暂时没有要整理的」（前端）

> 状态：开发中（2026-10-05 派出）｜执行者：Codex｜只改前端，不动 Rust。**只改一句话的分支，别顺手改别的。**

## Fox 能看到的效果
打开「设置 · 自动整理与备份」，最上面的状态徽章不再和下面蓝框打架：现在是徽章写「已开启，暂时没有要整理的」、蓝框写「需要整理 495 个」，看着自相矛盾。改完以后，还有收藏没整理好（后台没排队）时，徽章写「已开启，后台暂时没有排队的」，下面一句话指向「现在整理」；真的一个都不剩时，徽章和说明一个字都不变。

## 为什么有这张卡
发布 #12 之后总控读了真实页面：状态徽章（`automationState` 的最后一支，`automation-view.ts` 约 396 行）写「已开启，暂时没有要整理的 / 有新收藏，或者你点了「加入整理」，就会自动开始。」，但同一页下面的蓝框写「需要整理 495 个」。徽章说的其实是**后台队列是空的**，不是「没有收藏要整理」，两句话挨着看就是矛盾。Fox 上一次就是被这种前后不一的说法绕晕的。

这一支只在这几件事同时成立时走到：开了自动整理、没暂停、不在「现在整理中」、每天额度有剩、`queued + running` 为 0。此时如果 `fullScanPending(status) > 0`（后端的 `awaiting`；旧后端退回 `needsOrganizing`，再退回 `queued + running`），就说明现有收藏里还有没整理好的——点「现在整理」之前，以及将来某次格式升级后、或某一轮整理有失败时，都会出现。一开始整理就走「现在整理中」那一支，不受影响。

## 背景与先读什么
- `apps/desktop/src/lib/automation-view.ts`：`automationState`（约 375–397 行）和 `fullScanPending`（约 168 行）。
- `apps/desktop/src/lib/automation-view.test.ts`：`describe('automationState', …)`（约 152–210 行），测试里的 `status()` 夹具（约 78 行，默认没有 `awaiting`、`needsOrganizing`，所以 `fullScanPending` 是 `queued + running`，即 0）。
- 页面上蓝框的按钮叫「现在整理全部 N 个」或「现在整理 N 个」，所以说明里写「点下面的「现在整理」」是对得上的（这一支只会在 AI 服务已配置时走到，那时蓝框一定有按钮）。

## 要做的事
只改 `automationState` 的**最后一个 `return`**：

```ts
return { label: '已开启，暂时没有要整理的', hue: 148, detail: '有新收藏，或者你点了「加入整理」，就会自动开始。' };
```

改成按 `fullScanPending(status)` 分两种（`hue` 两种都还是 148）：

- **`pending > 0`**：
  - `label`：`已开启，后台暂时没有排队的`
  - `detail`：`现有收藏里还有 ${pending} 个说明要整理，点下面的「现在整理」一次做完；新收藏、或你点了「加入整理」的，会自动开始。`
- **`pending <= 0`**（`fullScanPending` 对 NaN、负数、无穷大本来就返回 0）：**一个字不改**，还是原来的 `label` 和 `detail`。

前面所有分支（读取中、没配置 AI 服务、已暂停、现在整理中、未开启、每天额度用完了、整理中）的文案、顺序、`hue` **都不动**。

## 测试（每一条都要能「改坏就失败」）
1. 既有的「开了、有额度、没东西可整理：绿色待命」那条保留，并**补上 `detail` 的逐字断言**（现在只断言了 `label` 和 `hue`）：`有新收藏，或者你点了「加入整理」，就会自动开始。`
2. 新增：`pending > 0` 的三种来源各一条，逐字断言整个对象（`label`、`hue: 148`、`detail` 带真实个数）：
   - `awaiting: 495`；
   - 只有 `needsOrganizing: 7`（没有 `awaiting`，旧后端）；
   - `awaiting: 1`（边界，个数写 `1`）。
3. 新增：下面这些**仍是原来那句**（逐字断言 `label` 和 `detail`）：`awaiting: 0`（哪怕同时 `needsOrganizing: 5`——`awaiting` 是 0 就是 0，不回退）、`awaiting: Number.NaN`、`awaiting: -3`。
4. 新增：优先级不变——`awaiting: 495` 同时满足下面任意一种时，仍然返回各自原来的 `label`：`providerConfigured: false`（还没配置 AI 服务）、`paused: true`（已暂停）、`unlimited: true`（现在整理中）、`enabled: false`（未开启）、`budgetRemaining: 0`（每天额度用完了）、`queued: 3`（整理中）、`running: 1`（整理中）。每种一条断言即可。
5. 新增：新文案（`pending > 0` 的 `label` + `detail`）不含「今天」「明天」，也不含「暂时没有要整理的」。

## 不要做
- 不要改 Rust、命令、依赖、后端字段。
- 不要改 `fullScanPending` / `fullScanPanel` / `automationHint` / `organizeCounts` 等任何其他函数，不要改 `toolbox.tsx`、`toolbox.test.tsx`。
- 不要改前面各分支的文案、顺序和 `hue`，不要改 `AutomationState` 类型。
- 不要启动应用、不要碰数据目录和钥匙串。

## 验收
1. `tools/cards/run.sh web-build`、`web-test` 通过（改前 837 项，只增不减）。
2. `git diff --stat` 只出现「允许改动」里的两个文件；`automation-view.ts` 的生产代码改动只在 `automationState` 最后一支。
3. 报告写：条号 → 文件:行；改了哪些既有测试（只应该是补 `detail` 断言）；新增测试名；没把握的地方。

## 停止条件
- 需要改「允许改动」以外的文件，或需要后端新字段：停手，写进报告。
