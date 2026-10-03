# 交回报告 · 卡 11-ui-library

## 一句话结论

资料库页已按任务卡重写：筛选纯函数 19 个用例、三栏页面、`App.tsx` 只加了 `onOpenSolve`；`web-build` 与 `web-test` 都通过。没有启动 App，也没有用真数据点过页面。

## 验收逐条

| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| 1. `tools/cards/run.sh web-build`、`tools/cards/run.sh web-test` 通过；`library-filters.test.ts` 至少 14 个用例，覆盖每个纯函数和列出的边界 | 是 | `web-test`：9 files、95 tests 全过，其中 `library-filters.test.ts` 19 tests。`web-build`：`tsc --noEmit && vite build` 成功，2154 modules，562ms。用例行号见下方。 |
| 2. 逐项列出筛选栏三组、联动计数来源、防抖与过期丢弃、加载更多、三栏与两个断点、改状态撤销、键盘、空状态、详情三个操作的文件:行 | 是 | 见下方「验收 2」。页面行为没有浏览器点击证据，行号是源码位置。 |
| 3. 页面没有写死的十六进制颜色；只有一个主按钮，给出 `variant="primary"` 的位置 | 是 | 任务卡原文 `grep -nE` 不带 `-r`，对目录报 `grep: apps/desktop/src/components/library: Is a directory`（exit 2）。等价命令 `grep -nRE '#[0-9a-fA-F]{3,8}' apps/desktop/src/pages/v2/library.tsx apps/desktop/src/components/library` 无匹配（exit 1），卡 10 已有文件里也没有十六进制。唯一 `variant="primary"` 在 `apps/desktop/src/components/library/item-detail.tsx:202`（卡 10 原有「复制给 Codex」，本卡未改该文件）。本卡新增的按钮都是 `default` 或 `text`。 |
| 4. `git diff --stat` 只出现允许改动的路径；`App.tsx` 只有给资料库传 `onOpenSolve` 这一处 | 是 | `git diff --stat`：`App.tsx` 10 行、`library.tsx` 847 行。`git diff -- apps/desktop/src/App.tsx` 只把 `case 'library'` 从 `<V2LibraryPage initialKeyword={libraryKeyword} />` 改成传入 `onOpenSolve`（`App.tsx:307-314`）。三个新文件未跟踪，不进 `--stat`，路径都在允许改动里，见 `git status --short`。 |
| 5. 写清后端命令未合并时无法实际验证的逻辑，以及兜底 | 是 | 见「没做到的」和「给审核者的提示」。失败统一 `describeInvokeError`；命令缺失时文案是「这个功能还没接上。」复制给 Codex 在该文案且详情已加载时改用 `codexFallbackText`。 |

### 验收 1 的 19 个用例

`apps/desktop/src/lib/library-filters.test.ts`：

- 默认状态 `cardState: done`、`sort: starred`：27
- `toFilters` 空关键词 / 空白不传、`offset = page × pageSize`、非法页归零：38
- 场景、状态、形态都带上，关键词 trim：58
- `toggleScenario` 再点取消、点另一项替换：79
- `toggleReading` 同样：89
- `toggleForm` 同样：96
- `clearFilters` 后 `cardState` 回 `done`，`sort` 保留：103
- `hasActiveFilters`：122
- `withInitial`，空白当成没有：134
- `visibleScenarios` 用联动计数，缺键当 0，不用场景自带个数：148
- `formOptions` 个数降序、并列按 id 升序、中文名：170
- `cardStateOptions`，「全部」= 已整理 + 整理中，非有限数当 0：198
- `nextPageOffset` 满页且未取完时返回已加载条数：218
- 取完、超过总数、还没加载、页大小非法、这一页没取满：都是 null：224
- `shouldFallbackToAll` 已整理为 0 且还有待整理、没动过、仍是默认：235
- 动过、已不是默认、没有待整理、或已整理大于 0：为假：241
- `applyOptimisticReading` 返回新数组，不改原数组：254
- `removeIfFilteredOut` 筛了状态且对不上就移除，原数组不变：269
- 没有状态筛选，或新状态仍符合，就留下：280

### 验收 2 的文件:行

筛选栏三组（`apps/desktop/src/components/library/filter-panel.tsx`，新文件；`index.ts` 未改）：

- 场景：27。名称、`thin` 时 `Tag`「偏少」（42）、右侧个数。
- 我的状态：47。顺序 `unseen / want / tried / in_use / dropped`（8），标签用 `readingGroupLabel`。
- 形态：59。行数据来自 `formOptions`。
- 选中项 `bg-accent text-accent-foreground`：99。组标题 12px（`text-xs`）`text-on-surface-variant`：76。
- 页面挂上筛选栏：`library.tsx:615-631`。宽屏放左栏 564-567；不够宽时收进「筛选」按钮 538-557。

联动计数来源（不在前端自己数条目）：

- 场景个数：`library-filters.ts:119-129` 的 `visibleScenarios`，用 `counts.scenarios[id]`，缺键 0。页面 `library.tsx:490`。
- 形态：`library-filters.ts:133-145` 的 `formOptions`。页面 `library.tsx:491`。
- 我的状态：`library.tsx:707-714` 的 `readingCountsOf`，读 `counts.reading`。
- 已整理 / 整理中 / 全部：`library-filters.ts:149-158` 的 `cardStateOptions`。页面 `library.tsx:493-499`，`Segmented` 在 559。
- 右侧「{n} 个」：`library.tsx:561`，用本次 `listLibraryView` 的 `totalCount`（筛选后的总数，不是已加载条数）。副标题的收藏总数用 `counts.total`：500-503。
- 计数没到时筛选项显示「…」：`filter-panel.tsx:105`。

搜索防抖与过期结果丢弃（`library.tsx`）：

- 防抖 200ms：常量 39，写入 `ui.keyword` 的 effect 126-131。占位「搜工具、说明、关键词」：532。
- 筛选键 `uiKey`：721-729。条件一变就重取第 0 页：effect 152-193，每页 80（38）。
- 世代号 `listGeneration`：89。每次请求 `++`：153。晚到的结果在 170 和 186 丢掉（cleanup 同时把 `cancelled` 设为真）。
- `listScenarios` 只在挂载取一次，失败可重试：133-150，依赖 `scenarioReloadKey`。失败时筛选栏显示 `describeInvokeError` 加「重试」（`filter-panel.tsx:29-32`），页面不崩。

加载更多：

- 是否显示：`library-filters.ts:166-174` 的 `nextPageOffset`；页面 506-507、按钮 577-579。
- 点击追加：`loadMore` 436-470。offset 用 `serverItems.length`，页码 = offset / 80。追加按 id 去重。返回 0 条新条目就不再显示按钮。筛选一变把 `noMore` 清掉：162-163。请求世代对不上就丢掉：446、463。

三栏和两个断点（`library.tsx`）：

- `<1100px`：`useMaxWidth(1099)` 102。`<900px`：`useMaxWidth(899)` 103。`matchMedia` 实现：732-745。
- 宽屏三栏 `200px / minmax(0,1fr) / minmax(340px,440px)`：515-516。
- 中屏两栏（列表 + 详情）加「筛选」浮层：517-518、538-557。点浮层外关闭：208-218。回到宽屏关掉浮层：204-206。
- 窄屏只留一栏：519。点一行进详情：485-487。返回只在窄屏传给 `ItemDetail`：598。列表/详情互斥：513-514。

改状态加撤销（`library.tsx`）：

- 乐观更新不改 `serverItems`，记在 `overrides` 里，展示时走 `projectItems`：106、677-691。先 `applyOptimisticReading`，当前筛了状态组再 `removeIfFilteredOut`。撤销只清掉 override，行会回到列表。
- `changeReading`：337-363。toast 标题 `已标为「${readingGroupLabel(group)}」`，带「撤销」（346-349），和今日页同一句式。
- 成功后只 `refreshCounts`（327-335）：再调一次 `listLibraryView` 取 `counts` 和 `totalCount`，不替换已加载条目。
- 写库失败把 override 和详情退回，并 toast 错误：356-361。撤销本身：365-379。

键盘：

- `/` 聚焦搜索；已在输入框、正在输入、或带修饰键时不触发：250-266，复用 `shouldIgnoreKey`。
- `U` 设为 `in_use`：268-272。`use-list-keys` 没有 U，也没有改那个 hook。
- `J` / `K` / `T` / `X` / `C` 交给 `useListKeys`：278-288。`T` → `want`，`X` → `dropped`，`C` → `copyForCodex`。按键本身在未改的 `hooks/use-list-keys.ts:66-82`。
- 底部快捷键提示：583-589。

空状态：

- `library.tsx:640-652`。标题「没有找到」，说明「换个说法试试，或去「找方案」说说你想做什么」，次要按钮「去找方案」`variant="default"`，调用 `onOpenSolve(ui.keyword.trim())`，没有关键词就是空串。
- 列表用 `ItemRow` 且不传 `why`：661-669。选中不在新列表里就改选第一项：195-198。

详情三个操作（`library.tsx` 把回调传给卡 10 的 `ItemDetail`，593-609）：

- 改状态：600 → `changeReading` 337。
- 说得对吗：确认 601 → `confirmCard` 411（本地把 `cardReview` 标成 `confirmed`）；重新整理 602 → `regenerateCard` 424（成功 toast「已排队，整理好会更新」）。
- 复制给 Codex：603 → `copyForCodex` 393。唯一主按钮仍是 `item-detail.tsx:202`。
- 详情数据：`getLibraryDetail` 220-248，选中变化或点重试才重取。命令失败显示 `describeInvokeError`，不崩。

首次空白回退（只判断一次）：

- `shouldFallbackToAll`：`library-filters.ts:181-190`。页面在第一次拿到非空 `counts` 时判断：172-178。为真就改成「全部」并记下提示，这次不写入「已整理」的空列表。
- 提示文案「AI 还没整理出理解卡，先显示全部收藏。」：570-573。显示条件 504：提示还在、用户没点过三选项、当前是「全部」。
- 点任一选项：`chooseCardState` 473-477，`touched` 置真，提示清掉。

`App.tsx`：

- `onOpenSolve`：307-314。`setSolveQuestion(query)` 然后 `setCurrentPage('solve')`。空串也原样传，不回退到旧的 `solveQuestion`。
- `initialScenario` 只在页面 props 预留（`library.tsx:41-45`、`withInitial` `library-filters.ts:54`）。没有调用方。

## 改了哪些文件

- `apps/desktop/src/lib/library-filters.ts`：新文件。筛选状态、联动计数、分页、空白回退、乐观更新，都不依赖 React。
- `apps/desktop/src/lib/library-filters.test.ts`：新文件。19 个用例。
- `apps/desktop/src/components/library/filter-panel.tsx`：新文件。筛选栏三组。未改 `index.ts`、`item-row.tsx`、`item-detail.tsx`、`reading-segmented.tsx`。
- `apps/desktop/src/pages/v2/library.tsx`：整页替换旧骨架。
- `apps/desktop/src/App.tsx`：只在 `library` 分支传入 `onOpenSolve`。

## 验证

- `tools/cards/run.sh web-test` → 通过。9 files、95 tests，`src/lib/library-filters.test.ts` 19 tests，约 206ms。有 `NO_COLOR` / `FORCE_COLOR` 的 Node 警告，没有失败。
- `tools/cards/run.sh web-build` → 通过。`tsc -b`、`tsc --noEmit && vite build`，2154 modules，562ms。
- `git diff --stat` → 只有 `apps/desktop/src/App.tsx`（10 行）和 `apps/desktop/src/pages/v2/library.tsx`（847 行）。
- `git status --short` → 上面两个已修改；未跟踪的是 `filter-panel.tsx`、`library-filters.ts`、`library-filters.test.ts`。
- `grep -nE '#[0-9a-fA-F]{3,8}' apps/desktop/src/pages/v2/library.tsx apps/desktop/src/components/library` → 目录被当成文件，`Is a directory`，exit 2。
- `grep -nRE` 同一范围 → 无匹配，exit 1。
- `variant="primary"`：只在 `item-detail.tsx:202`。`library.tsx` 里没有。

## 没做到的 / 偏离任务卡的地方

- 没有启动 App，没有做浏览器点击。红线禁止 `pnpm dev:desktop`、`tauri dev`、`cargo run`、`open`，也不占用 8765、39217。断点、浮层、键盘、toast、改状态这些交互只靠类型检查、构建和纯函数测试，没有在页面上点过。
- 后端 `list_library_view`、`list_scenarios`、`get_library_detail`、`set_library_reading`、复制 / 确认 / 重新整理若还没合并，对应操作会走 `describeInvokeError`（命令缺失时是「这个功能还没接上。」），页面不崩。没有真库，也没有假后端，这些路径没有跑过。
- 复制给 Codex 的兜底：错误文案含「这个功能还没接上」且当前详情已加载时，用 `codexFallbackText` 再尝试写入剪贴板（`library.tsx:399-405`）。剪贴板不可用时 toast「浏览器不允许自动复制」。
- 列表或场景读取失败：列表区或筛选栏显示错误加「重试」，不整页崩溃（`library.tsx:637-638`、`filter-panel.tsx:29-32`）。
- 任务卡里的 `grep` 不带 `-r`，对目录会报错。内容检查用了 `grep -nRE`，结果为空。

## 需要总控或 Fox 决定的事

无。没有花钱、删数据、动正式库、加依赖、改契约或改卡 10 文件。下面是卡里没写死时已经落地的保守做法，总控若要换，可以改，不必先问 Fox。

## 给审核者的提示

- `git diff --stat` 看不到三个未跟踪新文件。核对时用 `git status --short`。它们都在允许改动里。
- `nextPageOffset`（`library-filters.ts:166-174`）：已加载条数不是 `pageSize` 的正整数倍时返回 null，「加载更多」不出现。这样不会把不满的一页再当成第 0 页重取，也不会跳条目。若后端在还有更多时就返回不满的一页，按钮会提前消失。
- 「全部 N」= `done + pending`（`library-filters.ts:157`）。`cardStateOptions` 拿不到 `counts.total`。非有限数当 0。副标题用的是 `counts.total`，两边口径不同。
- 自动改成「全部」之后，`hasActiveFilters` 为真（`cardState !== 'done'`），「清除筛选」会出现。点清除会把整理状态设回「已整理」。`fallbackChecked` 已经为真，不会再自动切回「全部」。提示在用户点过三选项，或当前不再是「全部」时消失。
- 右侧「n 个」是筛选后的 `totalCount`。筛选刚切换时不把 `counts` 清掉，副标题和筛选项数字会短暂留着上一轮，新响应到了再换，避免闪成「…」。
- 改状态成功后不重载列表。条目留在 `serverItems`，展示层用 override；筛了状态组时 `projectItems` 把它拿掉。撤销只改 override。计数靠 `refreshCounts`。刷新计数失败时状态已经写上，toast 会说数字可能还没刷新。
- 排序没有界面。默认 `starred`。`clearFilters` 保留 `sort`。
- `initialScenario` 没有调用方。命令面板仍只传 `initialKeyword`，本卡没改那一处。
- 断点用 `matchMedia`（1099 / 899），不用两套会打架的 Tailwind 显隐。原型写的是 1240，任务卡写的是 1100，按任务卡。
- `ItemRow` 没有「在用」回调。`U` 只走键盘。没有改卡 10 的行组件。
- 空状态和「加载更多」「筛选」都是 `variant="default"`。唯一主按钮仍是详情底栏「复制给 Codex」。
- `onOpenSolve` 传的是当前关键词 trim 后的值，可以是空串。没有照命令面板写成 `query || solveQuestion`。找方案页对空问题的原有行为没有改。
- 本卡没改 `contracts.ts`、后端、kit、其他页面、`components/library/index.ts` 和卡 10 已有组件，也没有新依赖、lucide、十六进制色。
