---
编号: 40-organize-page-logic
允许改动:
  - apps/desktop/src/lib/automation-view.ts
  - apps/desktop/src/lib/automation-view.test.ts
  - apps/desktop/src/pages/v2/toolbox.tsx
  - apps/desktop/src/pages/v2/toolbox.test.tsx
  - apps/desktop/src/lib/item-format.ts
  - apps/desktop/src/lib/item-format.test.ts
  - apps/desktop/src/components/library/item-detail.test.tsx
  - apps/desktop/src/components/library/today-page.test.tsx
---

# 卡 40 · 「自动整理」页：讲清楚先后顺序和定义，额度用完也不锁死（前端）

## 为什么有这张卡
2026-10-05 Fox 看着开发版「设置 · 自动整理与备份」说：**这个界面逻辑不对**。他想要的是：「开始的时候会全部整理，然后后续应该是增量整理，或者自动整理；『自动整理』的定义要明确；而且不能额度用完就不能动了，可以有提示，但不能完全不给用。」

现在的页面问题（对着 Fox 的截图看）：
1. 页面顶上橙色徽章写「今天的额度用完了」+「明天会自动继续」，同一张卡里下面又写「这一轮不受每天额度限制」——读起来像「今天什么都点不了」。（后台这个「今天」其实是滚动的最近 24 小时，也没有「明天」；账的问题由卡 39 解决。）
2. 「一次性全部整理」写着「还有 500 个收藏没有中文说明」，下一行却写「上次整理了 461 个」，自相矛盾：那 461 个是旧格式的说明，要按新格式重写，并不是「没有说明」。
3. 「自动整理」「一次性全部整理」「每天自动整理」「集中补齐」「给还没有说明的收藏写说明」「重写旧的中文说明」——六个词、三条路，谁管谁、谁先谁后看不出来；「自动整理」到底指什么没有一句定义。
4. 额度用完时，今日 / 资料库顶上的提示、详情里的提示都说「明天会接着整理」，没有一个能现在就动手的入口；如果只剩 3 个排着队（README 有实质变化被重新排队的），页面还会说「全部收藏都整理好了」。

这张卡只改前端：**把顺序和定义说清楚（先「把现有收藏整理一遍」，之后是「每天自动整理新增和变化的」），额度用完只是「自动」那一步先歇着，手动的路永远开着。** 后端新字段由卡 39 提供，两张卡并行做，**字段名以下面为准**，且都要有「字段缺失」时的回退（旧后端不带字段，页面照常能用）。

## 要对 Fox 说清楚的定义（页面里要有）
- **自动整理** = 应用在后台，给「新收藏」和「README 有实质更新的收藏」写（或重写）中文说明。只处理这两类；没变化的不会重复整理、不会重复花钱。每天最多花他设的额度。
- **现在整理**（原「一次性全部整理」）= 他手动点、不受每天额度限制、一口气做完当前所有在等的。第一次用、或说明格式升级之后，用它把现有收藏整理一遍；之后偶尔不想等时也能用。**这条路不看额度，额度用完也能点。**

## 后端新字段（卡 39 提供；`AutomationStatus` 里全部加成可选，缺失时按回退）
| TS 字段 | 含义 | 缺失时的回退 |
|---|---|---|
| `activeRepos?: number \| null` | 有效收藏总数 | 没有就不显示分项那一行 |
| `needsOrganizingMissing?: number \| null` | 没有说明的个数 | 同上 |
| `needsOrganizingOutdated?: number \| null` | 旧版说明（要按新格式重写）的个数 | 同上 |
| `awaiting?: number \| null` | 现在在等整理的总数（没说明 + 旧版 + 已排队未做完，按收藏去重） | `needsOrganizing`，再缺就 `queued + running`（即现在的 `fullScanPending` 逻辑） |
| `dailyTokensUsed?: number \| null` | 每天额度口径的已用（不含「现在整理」那一段） | `tokens24h` |

`fullScanPending(status)` 改成按上面的回退顺序取 `awaiting`。

## 背景与先读什么
- `apps/desktop/src/lib/automation-view.ts`：`AutomationStatus`（约 38 行）、`BUDGET_PRESETS`（约 91 行）、`fullScanPending`（约 147 行）、`fullScanPanel`（约 274 行）、`automationState`（约 313 行）、`automationHint`（约 349 行）、`itemPendingNote`（约 421 行）、`regenerateNote`（约 447 行）、`fullScanConfirmLines`（约 499 行）、`recentUsageMetric`（约 567 行）。
- `apps/desktop/src/pages/v2/toolbox.tsx`：`FullScanPanelView`（约 128–235 行）、`ToolboxPage`（约 236 行起；`askFullScan` / `startFullScan` / `enqueueCards`、`confirmingFullScan`、「每天自动整理」那一段约 497–584 行）。
- 页面上所有判断和文案都要放在 `automation-view.ts` 的纯函数里（项目没有 testing-library / jsdom，页面不能做渲染测试），`toolbox.tsx` 只负责按函数结果画，方便单测。
- 今日 / 资料库的提示条（`components/library/automation-banner.tsx`）不论按钮写什么字，点了都只是跳到这个页面，所以改按钮文字不影响跳转。
- 只改前端：不改 Rust、不改命令名和参数（`start_full_scan`、`stop_full_scan`、`set_ai_automation`、`enqueue_cards_now`、`pause_ai_queue`、`resume_ai_queue` 都照旧）。

## 要做的事

### 1. 页面结构（自动整理那一块，从上到下）
1. 区块标题「自动整理」不变，**区块说明换成定义**（`AUTO_ORGANIZE_DEFINITION`，原文）：
   「自动整理：应用在后台给「新收藏」和「README 有实质更新的收藏」写（或重写）中文说明——它能做什么、适合解决什么、怎么试。没变化的不会重复整理，也不会重复花钱。整理好的会出现在「今日」和「资料库」里。用的是你自己配置的 AI 服务，具体花多少钱以服务商的账单为准。」
2. 右上状态徽章 + 下面一行 `detail`（`automationState`）：**额度用完时**（已开启、不在「现在整理」中、`budgetRemaining <= 0`）：
   - label：「每天额度用完了」（保持黄色色相 36）；
   - detail：「每天自动整理的额度用完了（按最近 24 小时累计），过一阵额度恢复会自动继续{；现在有 N 个在排队}。想现在就整理，点下面的「现在整理」，不受额度限制；也可以在下面调高每天额度。」（`queued > 0` 才带「；现在有 N 个在排队」）。**不能再出现「今天」「明天」。**
   - 未开启（`!enabled`）的 detail 改成：「还没开启每天自动整理时，应用不会自动调用 AI，也不会花钱；你手动点「现在整理」时才会花钱（会先让你确认）。」
   - 其余状态（没配服务、暂停、整理中、在做、没有要整理的）文案不动。
3. 一行分项（`organizeCountsLine`，只在 `activeRepos`、`needsOrganizingMissing`、`needsOrganizingOutdated` 三个都有、且 `activeRepos > 0` 时画）：
   「现有 {total} 个收藏：新版说明 {organized} 个 · 旧版说明 {outdated} 个 · 还没有说明 {missing} 个。」其中 `organized = total − outdated − missing`（小于 0 按 0）；`outdated > 0` 时句末再加一句「旧版说明是用旧格式写的，还能看，但建议按新格式重写，「找方案」和资料库筛选才更准。」
4. **「整理现有收藏」面板**（原「一次性全部整理」，`fullScanPanel` + `FullScanPanelView`）。面板标题、说明、按钮都由 `fullScanPanel` 返回，按下表：

| 状态 | 标题 | 说明（灰字） | 正文 | 按钮 |
|---|---|---|---|---|
| `ready`、第一步（`organized === 0`，分项缺失时看 `lastFullScan == null`） | 第一步：把现有收藏整理一遍 | 只在第一次用、或说明的格式升级之后做一次：一口气把现有收藏整理完，不受每天额度限制。整理完，以后就只剩下面的「每天自动整理」。 | 需要整理 {N} 个，预计约 {X} token（具体花多少钱以你服务商的账单为准）。 | 开始整理全部 {N} 个 |
| `ready`、之后（其余情况） | 现在就整理 {N} 个 | 不想等每天自动整理的时候用：这一轮不受每天额度限制，整理完自动回到每天自动整理。 | 有 {N} 个收藏在等整理，预计约 {X} token（具体花多少钱以你服务商的账单为准）。 | 现在整理 {N} 个 |
| `nothing` | 现有收藏都整理好了 | 以后新收藏、README 有实质更新的，会交给下面的「每天自动整理」。 | （「上次…」那一行照旧） | 无 |
| `running` | 正在整理（不受每天额度限制） | 照旧 | 进度、停止按钮、暂停提示全部照旧 | 照旧 |
| `unavailable` / `loading` | 整理现有收藏 | — | 照旧 | 无 |

   - {N} = `fullScanPending(status)`（即 `awaiting`），{X} = 现有的 `roundedEstimate(estimateFullScanTokens(N))`。
   - **面板的状态只能由「配没配服务」「有没有在跑」「有没有在等」决定，不能看额度。** 也就是额度用完（`budgetRemaining === 0`）、`awaiting > 0` 时必须是 `ready`，按钮可点、点了照样出确认框。
   - 确认框 `fullScanConfirmLines(pending, estimate)` 三句：第一句改成「需要整理 {N} 个，预计约 {X} token（具体花多少钱以你服务商的账单为准）。」，第二、三句**一个字不改**。
   - **点按钮之后确认块要一眼看见**：第一次点按钮后，确认块出现时用 `scrollIntoView({ block: 'nearest' })` 把它滚进可视区，并把焦点放到「确认开始」上（Fox 反馈过「没有你说的确认开始的按钮」）。确认块里「确认开始」「取消」两个按钮文字不改。点按钮本身仍然只展开确认、不调用命令、不花钱（`askFullScan` 的语义不变）。
5. 「README 更新检查」那一行（`readmeLine`）位置不变。
6. **「每天自动整理」那一段**：
   - 小标题：「每天自动整理（后台自动做，只处理新增和变化的）」；说明改成：「第一步做完以后，平时不用管：每次同步到新收藏、或某个收藏的 README 有实质更新，应用会在后台自动整理它。每天最多花你设定的额度；用完这一天的自动整理就先歇着，不影响你随时手动「现在整理」。」
   - `BUDGET_PRESETS` 的 `tokens` 一个都不改（保存的值和测试靠它），文案改成：关闭「不自动整理，一分钱不花（你仍然可以手动整理）」；试跑「每天最多约 16 个，先看看效果」；日常「每天最多约 33 个」；**「集中补齐」改名「多整理些」**，说明「每天最多约 100 个，新收藏很多时用」。
   - 「现在整理」进行中的那句提示改成：「整理正在进行，这一轮不受每天额度限制；整理完才按下面的额度。」
   - 额度确认框、自定义额度、保存逻辑不动。
7. 三块数字（`Metric`）：第二块改成 label「自动整理额度还剩」；value：在「现在整理」中显示「不受每天额度限制」，已开启显示「{budgetRemaining} token」，没开启显示「—」；sub：「现在整理」中显示「正在整理中」，已开启显示「约还能整理 {approxRepos} 个 · 已用 {dailyTokensUsed ?? tokens24h} / 额度 {每天额度}（整理现有收藏用的不算）」（数字用 `formatTokens`），没开启显示「没有开启每天自动整理」。第一块 `recentUsageMetric`、第三块「等待整理」不动。
8. 底部那两个只排队不立刻做的按钮（`enqueueCards('missing'|'outdated')`）收进一个**默认收起的折叠区**，标题「只想慢慢整理（可选）」，里面先一句说明「这两个只是把它们排进队列，按每天额度慢慢做，不会马上花钱；想马上做完，用上面的「现在整理」。」按钮改名「把还没有说明的排进队列」「把旧版说明排进队列重写」；点完的提示改成：「已排进队列 {N} 个，按每天额度慢慢整理；想马上做完，点上面的「现在整理」。」（「现在整理」进行中：「已排进队列 {N} 个，整理正在进行，会一起处理。」；0 个：「没有还要整理的了。」）。「暂停」按钮留在原处。

### 2. 其余地方「额度用完」的说法（同一套话，都在 `automation-view.ts`）
- `automationHint`（今日 / 资料库顶上的提示）额度用完：title「每天自动整理的额度用完了」；detail「还有 {pending} 个没整理，过一阵额度恢复会自动接着做；想现在就整理，到「设置 → 自动整理与备份」点「现在整理」。」；按钮文字「去整理」。
- `itemPendingNote` 额度用完：「每天自动整理的额度用完了，过一阵会接着整理；也可以到「设置 → 自动整理与备份」点「现在整理」。先看 GitHub 上的原文：」，按钮文字「去整理」。
- `regenerateNote` 额度用完：sub「每天自动整理的额度用完了，过一阵会接着整理，也可以到「设置 → 自动整理与备份」点「现在整理」」。
- 以上凡是原来提到「一次性全部整理」的句子（`itemPendingNote` / `regenerateNote` 在一次性整理进行中的那几句、`automationHint` 的 `unlimited` 分支），把名字统一成「现在整理」；`lastFullScanText`（「上次一次性全部整理：…」）保持原样，因为它记录的就是那一类运行。
- 全仓库 `grep` 一遍「明天」「额度用完」「一次性全部整理」「集中补齐」：用户看得见的地方在允许范围内处理掉；范围外的列在报告里，不改。

## 测试（vitest）
1. 额度用完 + `awaiting > 0` + 没在跑 → `fullScanPanel` 是 `ready`（按钮文字、标题按表）；额度用完 + 没有要整理的 → `nothing`。
2. 第一步 / 之后 两种模式：有分项时按 `organized === 0` 判断；没有分项时按 `lastFullScan == null`；各自的标题、按钮文字、正文逐字断言。
3. 回退：旧后端（没有新字段）时，`fullScanPending` 依次回退 `needsOrganizing`、`queued + running`；分项那一行不出现；页面其余照常。
4. `organizeCountsLine`：三个数齐全；`organized` 夹在 0 以上；`outdated > 0` 才带后面那句；任何一个缺失返回 null。
5. `automationState`：额度用完的 label / detail 逐字断言，且**不含「今天」「明天」**，含「现在整理」；带 / 不带「现在有 N 个在排队」两种；未开启的新 detail。
6. `automationHint`、`itemPendingNote`、`regenerateNote`：额度用完的新文案逐字断言、不含「明天」；`unlimited` 分支改用「现在整理」的名字。
7. `fullScanConfirmLines`：新的第一句逐字断言，第二、三句和原来逐字相同。
8. `BUDGET_PRESETS`：`tokens` 列表不变（`[0, 100000, 200000, 600000]`），名字含「多整理些」、不含「集中补齐」。
9. 新增 `apps/desktop/src/pages/v2/toolbox.test.tsx`（用 `import SOURCE from '@/pages/v2/toolbox.tsx?raw'`，写法照 `components/library/map-page.test.tsx`）：源码里仍有「确认开始」「取消」两个按钮、`scrollIntoView`、`只想慢慢整理（可选）`；不再出现「集中补齐」「明天会」；`FullScanPanelView` 的状态来自 `scanPanel`（不在页面里再自己判断额度）。
10. 已有测试里因为文案改了而必须改的期望，改成新状态；**不放宽、不删**；改了哪几条在报告里点名。

## 不要做
- 不改 Rust、不改命令名和参数、不新增依赖、不改 `BUDGET_PRESETS` 的 token 数值、不改 `lastFullScanText`、不改额度确认框和自定义额度的逻辑。
- 不改今日 / 资料库页面的布局，只改它们读到的文案；不动备份、微信工作台那两个区块。
- 不在页面里调用任何会花钱的命令来「测试」；点按钮只展开确认这件事不能变。

## 验收
1. `tools/cards/run.sh web-build`、`web-test` 通过（`web-test` 在沙盒里跑不了时，说明原因，总控在沙盒外跑）。
2. `git diff --stat` 只出现「允许改动」里的文件。
3. 报告里写：条号 → 文件:行；改了哪些已有测试的期望；第 2 部分 `grep` 的输出里范围外的残留清单；没把握的地方。

## 停止条件
- 要改 Rust 或「允许改动」以外的页面才能做完——停手并写进报告。
