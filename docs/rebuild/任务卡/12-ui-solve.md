---
编号: 12-ui-solve
允许改动:
  - apps/desktop/src/pages/v2/solve.tsx
  - apps/desktop/src/components/solve/**
  - apps/desktop/src/lib/solve-view.ts
  - apps/desktop/src/lib/solve-view.test.ts
  - apps/desktop/src/App.tsx
---

# 卡 12 · 「找方案」页：说一句想做的事，勾选，复制给 Codex

## Fox 能看到的效果
点「找方案」：一个大输入框，下面是几个示例（做小红书封面、给公众号文章配图、短视频加字幕、做 PPT 和文档、做电商详情页图片）。输入「做小红书封面」回车：左边出现「你已经有的」——从 Fox 收藏里找出的项目，每条有一句「为什么相关」，**所有意图都对上了的**带蓝色「都对上了」标签并默认勾选（最多 3 个），只对上一部分的显示「只对上「小红书」」；如果没有同时对上全部意图的，页面顶部一行提示「你的收藏里没有同时对上「小红书」和「封面」的工具，下面是部分匹配」，**一个都不默认勾选，不硬凑**，并给一个「去「发现」看看」的入口。右边是**参考包**：选了几个就显示几个，可以切 Markdown / AGENTS.md / JSON 三种格式，预览框里是真的、能直接粘给 Codex 的内容；唯一的蓝色主按钮「复制给 Codex」把它复制好。这就是 Fox 说的「从提问到拿到可直接粘给 Codex 的参考包」。

## 背景与先读什么
- **视觉与行为依据：原型** `docs/rebuild/原型/template.html` 的 `pageSolve`（第 611 行起）、`runSolve`、`packText`，以及样式表里的 `.solvegrid`、`.hit`、`.circ`（勾选圆圈）、`.nm`、`.why`、`.crossnote`、`.gapcard`、`.packpre`、`.pk-h`、`.chip`。
- **卡 10 做好的积木（已在基线里，先读它们的 props 再用）**：`apps/desktop/src/components/kit/**`（`Card`/`CardHeader`、`Button`、`Tag`、`Segmented`、`EmptyState`、`Spinner`、`useToast`）、`apps/desktop/src/lib/api.ts`（`solveV2`、`buildReferencePack`、`describeInvokeError`、`listScenarios`）、`apps/desktop/src/lib/item-format.ts`（`formatStars`）。
- **后端契约（只读，不许改）**：`apps/desktop/src/lib/contracts.ts`：`SolveHit`、`SolveResult`（`intents`、`extras`、`hits`、`preselected`、`fullMatchCount`、`gapIntents`、`suggestedScenarioId`、`suggestedScenarioCount`、`pendingMatches`）。参考包的类型在 `lib/api.ts` 里：`PackRequest`、`PackOutput`（`content`、`itemCount`、`missing`、`warnings`）和 `buildReferencePack(request, format, generatedOn?)`。`format` 的取值是 `'markdown' | 'agents' | 'json'`（这是后端 `PackFormat` 的序列化值）。**这些命令的后端由别的卡实现，可能还没合并**；调用失败要有友好提示，不能崩。你没法用真数据看效果，总控会用假后端验收；你要保证类型检查、测试、构建通过，逻辑照契约写对。
- 现状：`apps/desktop/src/pages/v2/solve.tsx`（旧的骨架页，约 42 行，整个替换）；`App.tsx` 里怎么挂载 `SolvePage`（旧 props 是 `initialQuestion`、`onCreatePack`；**新 props**：`{ initialQuestion?: string; onOpenDiscover?: (scenarioId: string | null) => void }`）。旧的「参考包」子页（`pages/v2/packs.tsx`）仍保留，不要动。
- 测试环境：vitest 的 node 环境，没有 jsdom，也不许加依赖。**勾选、参考包请求组装、提示文案的逻辑必须写成不依赖 React 的纯函数并测试**。

## 要做的事

### 1. 纯函数 `lib/solve-view.ts`（带测试，至少 14 个用例）
- `initialPicks(result)`：直接用 `result.preselected`（但只保留确实在 `hits` 里的 id）。
- `togglePick(picked, id)`：返回新数组（不改原数组），保持勾选顺序；`orderedPicks(hits, picked)`：按勾选顺序返回命中项。
- `gapNote(result)`：返回给用户的提示文案或 `null`。规则：`hits` 非空且 `fullMatchCount === 0` 且 `intents.length > 1` 时，是「你的收藏里没有同时对上「A」和「B」的工具，下面是部分匹配。」（意图词用「、」不用，格式是 `「A」和「B」`，三个以上写成 `「A」「B」和「C」`）；`hits` 非空、`fullMatchCount === 0`、只有 1 个意图词时返回 `null`；`hits` 为空时返回 `null`（空状态另有文案）。
- `matchBadge(hit, intents)`：返回 `{ tone: 'blue' | 'gray', text }`：`fullMatch` 且 `intents.length > 1` 为蓝色「都对上了」；`!fullMatch` 且 `matched.length > 0` 为灰色「只对上「X」「Y」」；其他情况返回 `null`。
- `crossNote(result, scenarios)`：返回「你在「{场景名}」方向收藏了 {N} 个。想看更多同类的新项目？」或 `null`（`suggestedScenarioId` 为空时）；`pendingNote(result)`：`pendingMatches > 0` 时返回「另有 {N} 个还在整理，整理完可能对得上。」否则 `null`。
- `buildPackRequest(query, picked hits)`：`title = "参考包：" + query`、`problem = query`、`items` 按勾选顺序，第一个 `role: 'main'`、其余 `'alternative'`，每个带 `why = hit.why`，`localCapabilities: []`；`todayLocalDate(now)`：`YYYY-MM-DD`（本地时区）。
- `packPlaceholder(format)`：没选中任何项目时预览框里显示的占位文字：「（还没有选中的工具）」。
- `formatOptions`：三个格式的 `value`/`label`（`markdown`→Markdown、`agents`→AGENTS.md、`json`→JSON）。

### 2. 页面 `pages/v2/solve.tsx`（整个重写）
- **布局**（桌面）：`PageHeader`（「找方案」，副标题「说一句你想做的事，从你收藏的里面挑出能用的，再整理成给 Codex 的参考包。」）→ 一张卡：大输入框（高 40px）+ 按钮「找」（**没有结果时用主按钮样式，有结果后改成次要样式**，保证页面只有一个蓝色主按钮；有结果时唯一的主按钮是参考包里的「复制给 Codex」）+ 下面一排示例 `chip`（点一个就把它填进输入框并立刻搜索）→ 下面一个 `grid`：左边结果卡、右边参考包卡（左 1fr、右 340–460px；窄屏小于 900px 上下排列）。
- **搜索**：回车或点按钮调用 `solveV2(query)`；输入为空时按钮禁用；搜索中显示转圈；失败用 `describeInvokeError` 显示一行错误和「重试」。新请求覆盖旧请求，旧请求晚到的结果丢掉。`initialQuestion` 有值时，页面一出现就自动搜一次，并把它填进输入框。
- **结果卡「你已经有的」**：`CardHeader`（标题、命中个数、右侧灰字「勾选的会进右边的参考包」）；`gapNote` 非空时在卡片顶部显示一行浅蓝底提示，右边带文字按钮「去「发现」看看」（调用 `onOpenDiscover(result.suggestedScenarioId)`）；每一条命中：左边**勾选圆圈**（`button`，`aria-pressed`，勾选后蓝底白对勾）、中间一块（第一行：仓库名按钮（点击在 GitHub 打开）、`★星数`、形态标签、`matchBadge`；第二行：`oneLiner`；第三行「**为什么相关** {why}」，`why` 前的粗体字不加蓝色）；默认勾选 `initialPicks`；底部 `crossNote` 和 `pendingNote`（有就显示，`crossNote` 右边带「去「发现」看看」文字按钮）。
- **没有任何命中**：用 `EmptyState`：「你的收藏里没有直接对得上的 / 去「发现」按这个方向找找看」，下面一个次要按钮「去发现」；若 `pendingNote` 非空把它显示在说明里。
- **参考包卡**：标题「参考包」、「已选 N 个」、右侧 `Segmented` 切格式；预览框（等宽小字、可滚动、`whitespace-pre-wrap`）显示 `buildReferencePack` 返回的 `content`（**每次勾选或格式变化 250 毫秒防抖后重新取**，同样丢弃过期结果）；没选任何项目时显示 `packPlaceholder`，**不调用后端**；`PackOutput.warnings` 非空时在预览框上方显示灰色小字；`missing` 非空时显示「有 N 个项目已不在库里，已跳过」；底栏**唯一的主按钮**「复制给 Codex」（预览有内容才可点），点击后 `navigator.clipboard.writeText(content)`，成功弹 toast「已复制，去 Codex 里粘贴」+「{N} 个参考项目」；剪贴板失败弹「浏览器不允许自动复制」+「请手动选中后按 ⌘C」。**不做「保存到文件夹」**（以后的卡再做）。
- 调用 `buildReferencePack` 时 `generatedOn` 传 `todayLocalDate(new Date())`。
- 键盘：输入框里回车搜索；其余不加快捷键。

### 3. `App.tsx`
- 只做小改：给 `SolvePage` 传新 props：`initialQuestion={solveQuestion}`、`onOpenDiscover={(scenarioId) => { …跳到 discover… }}`（`setCurrentPage('discover')`，场景 id 暂时用不上，先忽略）。旧的 `onCreatePack` 不再传。**不要改 `App.tsx` 里别的东西。**

## 不要做
- 不改 `lib/contracts.ts`、不改后端、不改 `pages/v2/packs.tsx` 和别的页面、不改 `components/kit/**`、`components/library/**` 和卡 10 已有的文件（只允许在 `components/solve/` 里**新增**文件）；不新增依赖；不写死十六进制颜色（用令牌）；不引入 lucide。
- 不要启动 App，不要读写任何数据目录。

## 验收
1. `tools/cards/run.sh web-build`、`tools/cards/run.sh web-test` 通过；`solve-view.test.ts` **至少 14 个用例**，覆盖上面每个纯函数，含边界：`gapNote` 的三种情形（多意图无全匹配、单意图、无命中）和意图词个数 2/3 的格式；`matchBadge` 的各分支；`togglePick` 不改原数组且保持顺序；`buildPackRequest` 的角色分配和 `why`；`todayLocalDate` 在跨日边界（本地 0 点前后）的行为；`crossNote` 和 `pendingNote` 的有无。
2. 逐项列出：搜索与过期结果丢弃、示例 chip、勾选圆圈与默认勾选、`gapNote` 提示条、`matchBadge`、`crossNote`/`pendingNote`、空状态、参考包的格式切换与防抖、占位文字不调后端、`warnings`/`missing` 提示、复制给 Codex（含剪贴板失败兜底）各自在哪个文件哪一行（文件:行）。
3. 页面没有写死的十六进制颜色（给出 `grep -nE '#[0-9a-fA-F]{3,8}' apps/desktop/src/pages/v2/solve.tsx apps/desktop/src/components/solve` 的输出，应为空）；整个页面在任何时刻只有一个蓝色主按钮（说明「找」按钮在有结果后如何改成次要样式，并给出 `variant="primary"` 的位置）。
4. `git diff --stat` 里只出现「允许改动」里的路径；`App.tsx` 的改动只有上面说的那一处。
5. 报告里写清：哪些逻辑因为后端命令还没合并而无法实际验证，以及你做了什么兜底。

## 停止条件
- 要改契约或后端、要新依赖、卡 10 的积木不够用且必须改它们、需要改卡外文件——停手并写进报告。
