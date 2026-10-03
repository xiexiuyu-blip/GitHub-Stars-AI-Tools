# 交回报告 · 卡 12-ui-solve

## 一句话结论
做完了。「找方案」页已换成：输入、示例、勾选命中、缺口提示，以及可切换格式并复制给 Codex 的参考包。纯函数 30 个用例、`web-test`、`web-build` 都通过。真后端和浏览器里的点击没有验证。

## 验收逐条
| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| 1. `tools/cards/run.sh web-build`、`tools/cards/run.sh web-test` 通过；`solve-view.test.ts` 至少 14 个用例，覆盖每个纯函数，含 `gapNote` 三种情形和意图词 2/3 的格式、`matchBadge` 各分支、`togglePick` 不改原数组且保持顺序、`buildPackRequest` 的角色和 `why`、`todayLocalDate` 本地 0 点前后、`crossNote` 和 `pendingNote` 的有无。 | 是 | `web-test`：9 个文件、106 项通过，其中 `solve-view.test.ts` 30 项。`web-build`：`tsc --noEmit` 后 vite 构建成功（最终一次在改完「打开 GitHub」之后）。用例见 `apps/desktop/src/lib/solve-view.test.ts`：`gapNote` 116、125、141、149 行；`matchBadge` 167–195 行；`togglePick` 90、98 行；`buildPackRequest` 240 行；`todayLocalDate` 268 行；`crossNote` 204、211 行；`pendingNote` 229、233 行。 |
| 2. 逐项列出搜索与过期结果丢弃、示例 chip、勾选圆圈与默认勾选、`gapNote` 提示条、`matchBadge`、`crossNote`/`pendingNote`、空状态、参考包格式切换与防抖、占位文字不调后端、`warnings`/`missing`、复制给 Codex（含剪贴板失败）各自在哪个文件哪一行。 | 是 | 见下方清单。 |
| 3. 页面没有写死的十六进制颜色；任何时刻只有一个蓝色主按钮。说明「找」在有结果后如何改成次要样式，并给出 `variant="primary"` 的位置。 | 是 | 对具体文件 `grep -nE '#[0-9a-fA-F]{3,8}' apps/desktop/src/pages/v2/solve.tsx apps/desktop/src/components/solve/*.tsx apps/desktop/src/lib/solve-view.ts` 无匹配（exit 1）。任务卡那条把目录 `components/solve` 直接交给 grep，macOS grep 在 stderr 写 `Is a directory`（exit 2），stdout 仍为空。`variant="primary"` 只在 `solve.tsx:157`（还没有结果时的「找」）和 `solve.tsx:161`（已有结果时的「复制给 Codex」）。两者由 `hasResult`（`solve.tsx:100`，即 `result !== null`）互斥渲染，同一次渲染只出现一个。有结果后「找」走 `solve.tsx:155` 的 `variant="default"`。勾选圆圈是 `bg-primary` 的圆形按钮（`solve-hit-row.tsx:28`），不是第二个主按钮。 |
| 4. `git diff --stat` 里只出现「允许改动」里的路径；`App.tsx` 只有那一处。 | 是 | `git diff --stat`：`App.tsx`（2 行）、`pages/v2/solve.tsx`。新文件未跟踪，都在允许范围内：`lib/solve-view.ts`、`lib/solve-view.test.ts`、`components/solve/solve-hit-row.tsx`。`App.tsx` 只改了第 311 行：去掉 `onCreatePack`，改为 `onOpenDiscover` 里 `setCurrentPage('discover')`，`scenarioId` 未使用。 |
| 5. 写清哪些逻辑因为后端命令还没合并而无法实际验证，以及做了什么兜底。 | 是 | 见「没做到的」和下面兜底。 |

验收 2 的位置：

- 搜索、空输入不发请求：`solve.tsx:49-51`（`runSearch` 先 trim，空则返回）、`solve.tsx:179`（回车走表单提交）、`solve.tsx:155-157`（空输入禁用「找」）。
- 过期结果丢弃：`solve.tsx:52` 递增序号，`solve.tsx:59` 和 `solve.tsx:65` 对不上就丢掉；卸载时 `solve.tsx:73` 再加一号。参考包用 `solve.tsx:109-122` 的 `cancelled` 丢掉过期响应。
- 示例 chip：`solve.tsx:21` 五个例子，`solve.tsx:191-198` 点一下填进输入框并立刻搜索。
- `initialQuestion` 自动搜一次：`solve.tsx:77-82`。
- 搜索中转圈：`solve.tsx:320-324`。失败一行错误加「重试」：`solve.tsx:66`、`solve.tsx:327-331`。
- 默认勾选：`solve.tsx:62` 用 `initialPicks`。勾选圆圈：`solve-hit-row.tsx:22-35`（`button`、`aria-pressed`，勾选后 `bg-primary text-on-primary`）。切换：`solve.tsx:222` 的 `togglePick`。
- `gapNote` 浅蓝条和「去「发现」看看」：`solve.tsx:278-285`。
- `matchBadge`：`solve-hit-row.tsx:16` 计算，`solve-hit-row.tsx:49` 用 `Tag` 显示。仓库名打开 GitHub：`solve-hit-row.tsx:40-45`、`62-68`（`open_external_url`，失败则 `window.open`）。「为什么相关」粗体用的是正文色，不是蓝色：`solve-hit-row.tsx:52-53`。
- `crossNote` / `pendingNote`：有命中时在 `solve.tsx:294-306`；没有命中时 `pendingNote` 放进空状态说明 `solve.tsx:338-350`。
- 空状态：`solve.tsx:345-351`，`EmptyState` 标题「你的收藏里没有直接对得上的」，次要按钮「去发现」。
- 格式切换：`solve.tsx:236-241`。防抖 250 毫秒后调用 `buildReferencePack`，`generatedOn` 用 `todayLocalDate(new Date())`：`solve.tsx:110-112`。
- 没选中不调后端：`solve.tsx:106` 直接 return；预览用 `packPlaceholder`：`solve.tsx:253-254`。
- `warnings`：`solve.tsx:243-247`。`missing`：`solve.tsx:250-251`。
- 复制给 Codex：`solve.tsx:135-145`。成功 toast「已复制，去 Codex 里粘贴」加「{N} 个参考项目」（N 用 `PackOutput.itemCount`）。剪贴板不可用或写入失败：`solve.tsx:143-144`，toast「浏览器不允许自动复制」/「请手动选中后按 ⌘C」。没内容时按钮禁用：`solve.tsx:101`、`161`。

## 改了哪些文件
- `apps/desktop/src/lib/solve-view.ts`：勾选、提示文案、参考包请求、本地日期、格式选项。纯函数。
- `apps/desktop/src/lib/solve-view.test.ts`：30 个用例。
- `apps/desktop/src/pages/v2/solve.tsx`：整页重写。
- `apps/desktop/src/components/solve/solve-hit-row.tsx`：一条命中（勾选圆圈、仓库名、星数、形态、匹配标签、为什么相关）。
- `apps/desktop/src/App.tsx`：只改 `SolvePage` 的 props 那一行。

## 验证
- `tools/cards/run.sh web-test` → 9 files, 106 tests passed（含 `solve-view.test.ts` 30）。
- `tools/cards/run.sh web-build` → `tsc --noEmit` 通过，vite 构建成功。最后一次是在改完 GitHub 打开方式之后跑的。
- `git diff --stat` → 只有 `App.tsx` 和 `pages/v2/solve.tsx`。`git status --short` 里新增的三个路径都在允许改动内。
- 色值：对上述 ts/tsx 文件跑 `grep -nE '#[0-9a-fA-F]{3,8}'` 无匹配。

## 没做到的 / 偏离任务卡的地方
- 没有在浏览器里点过页面。任务卡禁止启动 App；vitest 是 node 环境，没有 jsdom，页面交互没有组件测试。布局和按钮互斥只靠阅读代码和类型检查。
- `solve_v2`、`build_reference_pack_v2`、`list_scenarios` 没有用真数据打过。命令不存在或返回不完整时的兜底：搜索和参考包失败都走 `describeInvokeError`，页面不抛到白屏；结果缺 `hits` / `intents` / `preselected`，或参考包缺 `content`，会变成一句中文错误加「重试」（`solve.tsx:374-376`、`399-401`）。`list_scenarios` 失败就当成没有场景（`solve.tsx:84-90`），`crossNote` 因此返回 null，不编造场景名。
- 新的一次搜索会先清掉上一次的结果。失败后看不到旧命中，避免旧结果配上新问题；点「重试」或再点「找」会再搜当前输入。
- 输入框里改过的句子不会写回 `App` 的 `solveQuestion`。任务卡要求不要改 `App.tsx` 里别的东西，所以离开再进入会按原来的 `initialQuestion` 再搜一次。
- 参考包顶栏没有用 `CardHeader`。340px 列宽下放不下三段格式切换，固定高度会被裁切。现在的顶栏类名与 `CardHeader` 相同，但允许换行（`solve.tsx:232-242`）。
- 任务卡原文的 grep 把目录当文件，会在 stderr 提示 `Is a directory`。文件本身没有十六进制颜色。

## 需要总控或 Fox 决定的事
无

## 给审核者的提示
- 「找」和「复制给 Codex」的 `variant="primary"` 写了两处，但由 `result !== null` 二选一。没结果时（含第一次搜索中、搜索失败）主按钮是「找」；一旦有 `SolveResult`（包括 0 条命中）主按钮就是「复制给 Codex」，没预览内容时它禁用。文字按钮和勾选圆圈不是实底主按钮。
- `initialPicks` 只过滤「id 确实在 hits 里」，并去掉重复，不自己再截成 3 个。最多 3 个默认勾选靠后端的 `preselected`。
- `gapNote` 用的是 `intents`，不是 `gapIntents`。两个词是「A」和「B」，三个及以上是「A」「B」和「C」。
- `crossNote` 的数量用 `suggestedScenarioCount`，这个字段为 null 时写成 0。场景 id 在 `list_scenarios` 里对不上中文名时返回 null。
- 复制成功的「N 个参考项目」用的是 `PackOutput.itemCount`，不是界面上的已选个数。已跳过的项目会少在这个数字里，同时预览框上方有「有 N 个项目已不在库里，已跳过」。
- 参考包请求的 `problem` / `title` 优先用 `result.query`，空的话用这次提交的输入。
- 没有做「保存到文件夹」。

## 第 2 轮修改

上面的「一句话结论」和验收表仍是第 1 轮交回时的记录。这一节只记总控退回后的 4 条意见。第 1 轮里写的行号是改之前的；下面用的是改完后的行号。`variant="primary"` 现在在 `solve.tsx:158`（还没有结果时的「找」）和 `solve.tsx:162`（已有结果时的「复制给 Codex」），仍然二选一。

### 1. 「为什么相关」重复时不显示

处理了。`whyLine` 在 `apps/desktop/src/lib/solve-view.ts:86-92`：`why` 去掉首尾空白后为空，或和去掉首尾空白的 `oneLiner` 完全相同，返回 `null`；否则返回去掉空白的 `why`。`oneLiner` 为 `null` 或缺失时按空字符串比较，所以非空且不同的 `why` 仍会显示。

`SolveHitRow` 在 `apps/desktop/src/components/solve/solve-hit-row.tsx:18` 取值，`53-58` 行只在非 `null` 时渲染整行（含「为什么相关」）。显示的是去掉空白后的句子。

新增 5 个用例（`solve-view.test.ts`）：

- `why 为空或只有空白时不显示`（291 行）
- `oneLiner 为 null 或 undefined 时，非空 why 仍然显示`（296 行）
- `why 和 oneLiner 完全相同时不显示`（303 行）
- `why 和 oneLiner 只差首尾空白时不显示`（307 行）
- `why 和 oneLiner 不同时返回去掉空白的 why`（312 行）

### 2. 没有认出任务词时给一行说明

处理了。`noIntentNote` 在 `solve-view.ts:95-102`：`intents` 为空且 `hits` 至少一条时，返回「没有认出具体的任务词，下面是文字上相近的收藏，请自己勾选你觉得对的。」，否则 `null`。

页面在命中列表上方渲染，`solve.tsx:218` 调用，`solve.tsx:280-288` 的 `NoIntentBanner`。样式与 `gapNote` 的浅蓝条相同（`bg-accent`、灯泡图标），没有右侧按钮。`gapNote` 在意图词少于等于 1 个时本来就是 `null`，两条不会同时出现。

新增 3 个用例：

- `有命中但没有意图词时给出说明`（318 行）
- `有意图词时不说明`（324 行）
- `没有命中时不说明`（328 行）

### 3. 进页面自动聚焦输入框

处理了。`solve.tsx:187` 的输入框 `autoFocus={!openingQuery}`。`openingQuery` 是 `initialQuestion` 去掉空白后的句子（`solve.tsx:37`）。没有这句话，或只有空白时，页面挂载即聚焦；有非空 `initialQuestion` 时不抢焦点。离开「找方案」会卸载这个页面（`App.tsx` 按当前页切换），再进来会重新按当时的 `initialQuestion` 决定要不要聚焦。

### 4. `App.tsx` 不再预填旧句子

先读了 `pages/v2/packs.tsx`（未改）。空的 `question` 不会在渲染时出错：第 22 行 `props.question || '还没从找方案带过来'`，空字符串会显示这句兜底；「生成」只在点击时才把 `question` 传给命令。

因此改了 `App.tsx:46`：`useState('我想整理群聊里的工具链接')` 改为 `useState('')`。这是第 1 轮那一处（现在的 `App.tsx:311`，`onOpenDiscover`）之外唯一多改的地方。`git diff -- apps/desktop/src/App.tsx` 只有这两行。第一次进入「找方案」不再自动搜索那句旧话。输入框里改过的句子仍然不会写回 `solveQuestion`。

### 验证

- `tools/cards/run.sh web-test`：9 个文件、114 项通过。其中 `solve-view.test.ts` 从 30 项增到 38 项（上面 8 个新用例，原有用例都还在）。
- `tools/cards/run.sh web-build`：`tsc --noEmit` 通过，vite 构建成功（2154 modules，约 935ms）。
- `git diff --stat`：仍只有 `App.tsx`、`pages/v2/solve.tsx`。未跟踪的新文件仍是 `lib/solve-view.ts`、`lib/solve-view.test.ts`、`components/solve/`。
- 没有在浏览器里点过自动聚焦和这条新提示。任务卡禁止启动 App；vitest 没有 jsdom，这两处的界面行为只靠阅读代码和类型检查。纯函数的分支由上面 8 个用例覆盖。
