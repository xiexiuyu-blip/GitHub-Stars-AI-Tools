---
编号: 11-ui-library
允许改动:
  - apps/desktop/src/pages/v2/library.tsx
  - apps/desktop/src/components/library/**
  - apps/desktop/src/lib/library-filters.ts
  - apps/desktop/src/lib/library-filters.test.ts
  - apps/desktop/src/App.tsx
---

# 卡 11 · 「资料库」页：按业务场景找

## Fox 能看到的效果
点「资料库」：顶部是搜索框，旁边「已整理 N / 整理中 N / 全部」切换（**默认只看已整理的**）、「清除筛选」和总数；下面三栏：**左边筛选**（业务场景——每个场景旁边是个数，库里偏少的方向（电商、SEO·增长、数据分析）带「偏少」小标签；我的状态五档；形态），**中间项目列表**，**右边详情**（先 AI 的中文结论，再能做什么、怎么试，唯一的蓝色主按钮是「复制给 Codex」）。点一个场景，列表只剩这个场景下的项目，其他筛选项旁边的数字随之联动；再点一次取消。搜索、筛选、改状态都不用等，改了状态弹「已标为想试」并带「撤销」。键盘：`/` 去搜索框、J/K 上下选、T 想试、U 在用、X 不要了、C 复制给 Codex。这是 Fox 要的三个新功能之一「业务场景视图」的界面。

## 背景与先读什么
- **视觉与行为依据：原型** `docs/rebuild/原型/template.html` 的 `pageLib`（第 574 行起）、`libList`，以及样式表里的 `.libhead`、`.search`、`.libsplit`（三栏 `200px / 1fr / 340–440px`）、`.fcard`/`.fh`/`.fi`（筛选栏）、`.cnt`。行和详情的样式沿用卡 10 做好的 `ItemRow`、`ItemDetail`。
- **卡 10 做好的积木（已在基线里，先读它们的 props 再用）**：`apps/desktop/src/components/kit/**`（`Card`、`Button`、`Tag`、`Segmented`、`EmptyState`、`Spinner`、`Kbd`、`ToastProvider`/`useToast`）、`apps/desktop/src/components/library/**`（`ItemRow`、`ItemDetail`、`ReadingSegmented`）、`apps/desktop/src/lib/api.ts`（`listScenarios`、`listLibraryView`、`getLibraryDetail`、`setLibraryReading`、`copyItemForCodex`、`confirmLibraryCard`、`regenerateLibraryCard`、`describeInvokeError`）、`apps/desktop/src/lib/item-format.ts`、`apps/desktop/src/hooks/use-list-keys.ts`。**卡 10 的 `pages/v2/today.tsx` 是同一套积木的完整用法示例，照着它的写法来**（选中、改状态加撤销、复制给 Codex、说得对吗、错误提示）。
- **后端契约（只读，不许改）**：`apps/desktop/src/lib/contracts.ts`：`LibraryFilters`、`LibraryPage`、`LibraryCounts`、`LibraryItem`、`LibraryDetail`、`ScenarioSummary`、`ReadingGroup`。这些命令的后端由别的卡实现，可能还没合并；调用失败要有友好提示，不能崩。你没法用真数据看效果，总控会用假后端验收；你要保证类型检查、测试、构建通过，逻辑照契约写对。
- 现状：`apps/desktop/src/pages/v2/library.tsx`（旧的骨架页，约 116 行，整个替换）；`App.tsx` 里怎么挂载 `V2LibraryPage`（它传了 `initialKeyword`）。新页的 props：`{ initialKeyword?: string; initialScenario?: string | null; onOpenSolve?: (query: string) => void }`。
- 测试环境：vitest 的 node 环境，没有 jsdom，也不许加依赖。**筛选状态的逻辑必须写成不依赖 React 的纯函数并测试**。

## 要做的事

### 1. 纯函数 `lib/library-filters.ts`（带测试，至少 14 个用例）
- 页面筛选状态类型 `LibraryUiState`：`{ scenario: string | null; reading: ReadingGroup | null; form: string | null; cardState: 'done' | 'pending' | 'all'; keyword: string; sort: 'starred' | 'stars' | 'updated' }`，默认值 `cardState: 'done'`、`sort: 'starred'`，其余空。
- `toFilters(state, page, pageSize)` → 契约里的 `LibraryFilters`（空串关键词不传；`offset = page × pageSize`）；`toggleScenario/toggleReading/toggleForm(state, value)`：再点同一项就取消，点另一项就替换；`clearFilters(state)`（场景、状态、形态、关键词清空，`cardState` 回到默认 `done`，`sort` 保留）；`hasActiveFilters(state)`（决定是否显示「清除筛选」，`cardState` 不是默认值也算）；`withInitial(initialKeyword, initialScenario)`。
- `visibleScenarios(scenarios, counts)`：把 `ScenarioSummary[]` 和 `LibraryCounts.scenarios` 合成筛选栏要显示的行（`{ id, nameZh, count, thin }`，**count 用联动后的 `counts.scenarios[id]`，没有键当 0**，保持原有排序）；`formOptions(counts.forms)`：形态按个数从多到少，个数相同按字母；形态的中文显示名映射（`skill`→技能、`mcp`→MCP、`cli`→命令行、`app`→应用、`library`→代码库、`api`→接口、`service`→服务、`collection`→合集、`other`→其他，未知值原样显示）；`cardStateOptions(counts.cardState)`：三个选项「已整理 N」「整理中 N」「全部 N」。
- `nextPageOffset(loaded, total, pageSize)`：是否还有更多（用于「加载更多」）。
- `shouldFallbackToAll(counts, state, touched)`：首次加载后，如果 `counts.cardState.done === 0` 且 `counts.cardState.pending > 0`，而用户还没有自己动过「已整理 / 整理中 / 全部」三选项（`touched` 为假）且当前 `cardState` 是默认的 `done`，返回真——页面据此自动改成「全部」，并在列表上方显示一行浅色提示「AI 还没整理出理解卡，先显示全部收藏。」（用户点任何一个三选项后提示消失）。这是为了刚装好、还没有生成过理解卡时，资料库不是一片空白。
- `applyOptimisticReading(items, id, group)`：返回把某个条目的 `readingGroup` 改掉的新数组（不改原数组）；`removeIfFilteredOut(items, id, reading)`：如果当前筛选里有状态组且该条目新状态不再满足，则把它从列表里移除。

### 2. 页面 `pages/v2/library.tsx`（整个重写）
- **布局**（桌面）：`PageHeader`（标题「资料库」，副标题「你收藏的 {total} 个工具，按你能拿它做什么来找。已整理的有中文说明，其余在排队。」）→ 搜索条（搜索框占满剩余宽度、`Segmented` 三选项、有筛选时显示文字按钮「清除筛选」、右侧「{n} 个」）→ 三栏 `grid`（左 200px 筛选卡，中间列表卡，右边详情卡；三栏都撑满剩余高度各自滚动）。宽度小于 1100px：左边筛选栏收成一个「筛选」按钮（点击展开成浮层）；小于 900px：只显示列表，点一项进入详情并有「返回」。
- **筛选栏**（`fcard`）：分三组，组标题 12px 灰字：「场景」（每项：名称、`偏少` 小标签（若 `thin`）、右侧个数）、「我的状态」（没看/想试/试过/在用/不要了 + 个数，来自 `counts.reading`）、「形态」（来自 `counts.forms`，用 `formOptions`）。选中项浅蓝底蓝字。点击切换。**个数全部用后端返回的联动计数**，不要在前端自己数。
- **数据**：`listScenarios()` 只在页面挂载时取一次；`listLibraryView(toFilters(...))` 在筛选条件变化时重新取（关键词输入 200 毫秒防抖，用新请求覆盖旧请求，**旧请求晚到的结果要丢掉**，不能让过期结果盖掉新结果）；每页 80 条，列表底部「加载更多」按钮（`nextPageOffset` 决定显示与否），点击追加下一页。第一次拿到 `counts` 后用 `shouldFallbackToAll` 判断是否自动改成「全部」（只判断一次，之后不再自动切换）。
- **列表**：用 `ItemRow`（资料库的列表行**不传 `why`**，只有今日页的「今天值得先看」才有第三行）；选中项默认是列表第一项；筛选变化后，如果原选中项不在新列表里就改选第一项。没结果显示空状态「没有找到 / 换个说法试试，或去「找方案」说说你想做什么」，并给一个次要按钮「去找方案」（调用 `onOpenSolve(当前关键词)`，没有关键词就传空串）。列表底部一行 `lnote` 快捷键提示。
- **详情**：选中后 `getLibraryDetail(id)`；右栏用 `ItemDetail`；**改状态、复制给 Codex、说得对吗**的行为和卡 10 的「今日」页完全一致（乐观更新、撤销 toast、错误 toast）。改了状态之后：如果当前筛选带着状态组，该条目会从列表里消失（用 `removeIfFilteredOut`），并刷新计数（重新取一次 `listLibraryView` 的 `counts` 即可，不必整页重载）。
- **键盘**：`/` 聚焦搜索框（焦点已经在输入框里时不触发）；J/K、T、X、C 用卡 10 的 `use-list-keys`；再加 `U`（设成「在用」）。
- 搜索框占位文字：「搜工具、说明、关键词」。**不要**在这里做「按意图找」（那是找方案页的事）。
- 页面里**只有一个主按钮**：详情底栏的「复制给 Codex」。

### 3. `App.tsx`
- 只做两处小改：给 `V2LibraryPage` 传 `onOpenSolve`（点击后 `setSolveQuestion(query)` 并 `setCurrentPage('solve')`，`setSolveQuestion` 和 `solveQuestion` 在 `App.tsx` 里已有）；如果需要从别处带场景进来（`initialScenario`），预留 props 即可，现在不必有调用方。**不要改 `App.tsx` 里别的东西。**

## 不要做
- 不改 `lib/contracts.ts`、不改后端、不改 `pages/` 下别的页面、不改 `components/kit/**` 和卡 10 已有的文件（只允许在 `components/library/` 里**新增**文件，比如 `filter-panel.tsx`；如果发现卡 10 的组件缺 props，写进报告，不要改）；不新增依赖；不写死十六进制颜色（用令牌）；不引入 lucide。
- 不要启动 App，不要读写任何数据目录。

## 验收
1. `tools/cards/run.sh web-build`、`tools/cards/run.sh web-test` 通过；`library-filters.test.ts` **至少 14 个用例**，覆盖上面的每个纯函数，含边界（空关键词、再点取消、清除筛选后 `cardState` 回默认、联动计数缺键当 0、形态排序并列规则、`nextPageOffset` 的边界、`shouldFallbackToAll` 的各种组合（done 为 0 且有待整理才为真；用户动过就为假；已经不是默认值就为假）、乐观更新不改原数组、`removeIfFilteredOut` 的两种情形）。
2. 逐项列出：筛选栏三组、联动计数的来源、搜索防抖与过期结果丢弃、加载更多、三栏布局和两个窄屏断点、改状态加撤销、键盘 `/` `J` `K` `T` `U` `X` `C`、空状态、详情的三个操作各自在哪个文件哪一行（文件:行）。
3. 页面没有写死的十六进制颜色（给出 `grep -nE '#[0-9a-fA-F]{3,8}' apps/desktop/src/pages/v2/library.tsx apps/desktop/src/components/library` 的输出，应为空，不含卡 10 已有文件里的）；页面只有一个主按钮（给出 `variant="primary"` 的位置）。
4. `git diff --stat` 里只出现「允许改动」里的路径；`App.tsx` 的改动只有上面说的那一处。
5. 报告里写清：哪些逻辑因为后端命令还没合并而无法实际验证，以及你做了什么兜底。

## 停止条件
- 要改契约或后端、要新依赖、卡 10 的积木不够用且必须改它们、需要改卡外文件——停手并写进报告。
