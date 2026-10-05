# 交回报告 · 卡 36-copy-cleanup

## 一句话结论
做完了：第 1、2、3、5、7（1–15）、8 条全部做完并各有测试；`web-build`、`web-test`（766 项，`TZ=Asia/Shanghai`、`TZ=America/Los_Angeles` 和另外 5 个时区都过）、`rust-fmt`、`rust-check`（6 条既有警告，没增加）、`rust-test`（1164 项通过、0 失败）都通过；有几处我按「最保守」做了取舍，见「没做到的 / 偏离」和「需要总控决定」。没有启动 App，界面没目测过。

## 验收逐条
| 验收项（照抄任务卡） | 做到了吗 | 证据 |
|---|---|---|
| 1. `web-build`、`web-test` 通过（有 Rust 测试读前端文件的，如 `packs.rs` 里的 `include_str!`，也要跑 `rust-test` 确认没红），新增 / 改写的测试覆盖上面各条（第 2 条的测试要在 `TZ=Asia/Shanghai` 和 `TZ=America/Los_Angeles` 下都能过——用本机时间构造，不依赖固定时区） | 是 | `web-build`：`tsc --noEmit` + `vite build` 通过。`web-test`：32 个文件 766 项全过（起点 27 个文件 647 项，净增 119 项）。`TZ=Asia/Shanghai`、`TZ=America/Los_Angeles`、`UTC`、`Pacific/Kiritimati`（+14）、`Pacific/Pago_Pago`（−11）、`Asia/Kolkata`（+5:30）、`Australia/Lord_Howe`（+10:30 / DST 半小时）下 `tools/cards/run.sh web-test` 都是 766 项全过（最终状态上又整组重跑了一遍）。Rust 里唯一读前端文件的是 `packs.rs:2047` 的 `include_str!("../../src/lib/codex-prompts.ts")`，这个文件我没碰；`rust-test` 仍跑了一遍（见「验证」）。各条对应的测试见下面「条号 → 文件:行」。**反向验证**：把 `monthDay` 临时改回「直接取串里的 UTC 日期」，`Asia/Shanghai` 下 5 个用例变红、`America/Los_Angeles` 下 9 个用例变红，还原后全绿；把「工具箱」塞进 `app-layout.tsx` 的一个 `title`，`old-terms.test.ts` 变红并指出 `app-layout.tsx:151`，还原后全绿 |
| 2. `git diff --stat` 只出现 `apps/desktop/src/` 下的文件 | 是 | `git diff --stat`：39 个已有文件，全在 `apps/desktop/src/` 下；`git status` 里另有 6 个新文件（`lib/task-progress-view.ts`、`components/app-layout.test.tsx`、`components/capture/inbox-card.test.tsx`、`components/library/today-page.test.tsx`、`lib/task-progress-view.test.ts`、`lib/old-terms.test.ts`），也都在 `src/` 下；`.tmp-card/` 已删 |
| 3. 报告里写：条号 → 文件:行；第 5 条 `grep` 的输出里范围外的残留清单；没把握的地方 | 是 | 见下面三节 |

## 条号 → 文件:行（行号都是 `apps/desktop/src/` 下，按最终状态）

### 1. 页签名
- `lib/nav.ts:41`：`toolbox` 页签 `label` 改「自动整理与备份」，`key` 不变。
- `App.tsx:386-387`：「覆盖显示」那一段删掉，直接 `const tabs = subTabsFor(currentPage);`（「按类型看」那一支也一起删了，`nav.ts:35` 本来就是它）。
- 测试：`lib/nav.test.ts:133`（设置那一组的页签名）、`:144`（**渲染出来的页签文字**：用 `SubTabs` 把 15 个页面的页签都渲染一遍 + 主导航，没有「工具箱」「地图」）、`:165`（`?raw` 读 `App.tsx`：没有再覆盖页签名）；`lib/automation-view.test.ts:1274`（卡 33 的守卫原来断言 `App.tsx` 里有 `label: '自动整理与备份'`，换成断言 `nav.ts` 里有、`App.tsx` 里没有）。

### 2. 「收藏于」按本机时区
- `lib/item-format.ts:40-97`：`monthDay` 重写。带 `Z` / `+08:00` 的串先按它自己的时区换算出那一刻，再取**本机**的年 / 月 / 日；不带时区的（`2026-10-03`、`2026-10-03 18:17:00`）按本机时间理解；认不出、不存在的日期（13 月、2 月 31 日）返回「—」；「今年」也按本机日历。`item-detail.tsx:156` 仍传 `now`，没动。
- 测试（全用 `new Date(年, 月 - 1, 日, 时, 分)` 构造，不靠固定 UTC 串）：`lib/item-format.test.ts:128-216`（本机凌晨 / 深夜、`+14:00` / `−12:00` / `+05:30` 等同一刻的多种写法、跨年、和 JS 自己按时区解析的结果逐个对照、不存在的日期）；`components/library/item-detail.test.tsx:102`（详情页渲染出来的「收藏于」）。
- 改过的既有断言：`item-format.test.ts` 的 `monthDay` 里原来写死的 `'2026-01-09T00:00:00.000Z'`、`'2026-10-03T18:17:00Z'`、`'…+08:00'`、`'2025-10-03T18:17:00Z'` 和 `item-detail.test.tsx` 里的 `…T18:17:00Z`，在别的时区会变红，所以**输入**换成本机时间构造的串，**期望值（「10 月 3 日」「2025 年 10 月 3 日」等）一个字没变**；「不含 18:17」收紧成「不含 `:`」。

### 3. 「少于 14 个」→「14 个以内」
- `lib/item-format.ts:321`（`THIN_TAG_HINT`，资料库左栏 `filter-panel.tsx` 和发现页 `scenario-row.tsx` 的悬停提示、`discover-view.ts` 的那句「偏少（14 个以内）」都读它）；`components/library/thin-tag.tsx:6` 注释。
- 测试期望字符串换成新文案：`components/library/filter-panel.test.tsx:49-97`、`components/discover/scenario-row.test.tsx:33-82`、`lib/discover-view.test.ts:85`。

### 5. 其它残留（前端文字）
- 任务条（审读 47）：新文件 `lib/task-progress-view.ts`（`taskDoingText` / `taskPercentText`），`components/app-layout.tsx:518,576` 和 `components/welcome-flow.tsx:469,513` 用它。「阶段：请求数据」→「正在做：读取收藏…（第 3 步，共 12 步）」；「已完成 3/12 批 · 25%」→ 进度条右下角只写「25%」。测试：`lib/task-progress-view.test.ts:19`、`components/app-layout.test.tsx:82,129`。
- `components/library/detail-panels.tsx`：原来「设置 → 工具箱」「抓取 README」已被卡 32 / 33 改成「设置 → 自动整理与备份」；本卡第 7.2 条又把这句换成了真实的入口（见 7.2）。
- 全仓库 `grep`：见下面「第 5 条 grep 的输出」。新界面里剩下的用户看得见的旧叫法只有两处，都已改：`lib/nav.ts:41`（工具箱）、`lib/repo-assets.ts:361`（Zread 链接悬停提示「用 AI 通读仓库后的解读文档」→「用 AI 通读这个工具的代码后写的解读文档」，`repo-assets.test.ts:219` 跟着换）。新增守卫 `lib/old-terms.test.ts`：用 `import.meta.glob` 读全部 `.ts/.tsx` 源码、去掉注释，查八个词在新界面里不再出现（下面列的范围外文件除外）。

### 7. 真数据界面清扫发现的问题
1. 顶栏「同步」悬停：`components/app-layout.tsx:140`「从 GitHub 同步你最新收藏的工具」。测试 `components/app-layout.test.tsx:69`（整页渲染，没有「拉取」，没连接时仍是「请先连接 GitHub 账号」）。
2. 项目说明还没下载：
   - 先核对了两个入口：顶栏「同步」→ `use-stars-workspace.ts` 的 `handleSyncStars` → 后端 `sync_github_stars_worker` 里的 `refresh_stale_readmes`（`lib.rs:4315`，`readme_follow::refresh_targets` 把「没有缓存的」排在最前面，`readme_follow.rs:122,165`）；「设置 → GitHub」页的「下载项目说明」按钮（`pages/settings.tsx:583-616`）→ `handleFetchReadmes({ onlyMissing: true })` → `fetch_repository_readmes`。两个都真的存在、真的下载。
   - 概览：`lib/item-format.ts:295`（`noInstallStepsNote`）→ `components/library/item-detail.tsx:301`。没下载：「还没下载项目说明，暂时看不到安装步骤。」后面仍接原来的下一步（「点「打开 GitHub」看发行版或文档；也可以直接复制给 Codex，让它去读项目说明。」）；已下载但没写：保留原句；还在读取：「正在读取项目说明…」；读出错：「项目说明没读出来，暂时看不到安装步骤。…」。
   - 预览：`lib/item-format.ts:306`（`noScreenshotsNote`）→ `detail-panels.tsx:170`。没下载：标题「还没下载项目说明，暂时看不到截图。」；已下载没图：保留「项目说明里没有截图」+ 原说明。
   - 项目说明页：`item-format.ts:281-284` → `detail-panels.tsx:272-273`：标题「还没下载这个工具的项目说明。」+ 说明「点顶栏的「同步」，或到「设置 → GitHub」点「下载项目说明」，就会补上。」
   - 同样的指错地方的句子还在「怎么试」页（`detail-panels.tsx:139`，原来写「同步后或在「设置 → 自动整理与备份」里会自动下载」），一并换成「还没下载这个工具的项目说明，所以找不到安装命令。点顶栏的「同步」，或到「设置 → GitHub」点「下载项目说明」，就会补上。」（`item-format.ts:287`）。
   - 测试：`components/library/detail-panels.test.tsx:100-148,177,197`（怎么试 / 项目说明 / 预览三页各状态）、`lib/item-format.test.ts:644`、`components/library/item-detail.test.tsx:301`。
3. 最近 7 天：`lib/item-format.ts:169`（`WEEK_NEW_LABEL`）、`:179`（`buildTodayLead`）；`pages/v2/today.tsx:49,394`（统计卡、列表标题都读它）。测试 `components/library/today-page.test.tsx:21`（整页渲染）、`lib/item-format.test.ts:280,301`。
4. 合集按钮 / 下载安装包卡：`lib/item-format.ts:362-366`（`copyButtonLabel`）→ `item-detail.tsx:378`；`detail-panels.tsx:87`（`showReleases = props.hasDownload`，没找到安装包 / 发行版的迹象就不显示，不再「没找到命令就一律显示」）。测试：`item-detail.test.tsx:281,287`（合集写「帮我看看怎么用」，另外 9 种类型 / 没填都不变）、`item-format.test.ts:692`、`detail-panels.test.tsx:109-148`（没找到不显示 / 找到显示 / 有命令没安装包不显示 / 两样都有就都在 / 读取中不先说没找到）。
5. 类型「技能」：`lib/library-filters.ts:52`「技能包（给 AI 助手装的扩展）」；`library-filters.test.ts:363,380`、`filter-panel.test.tsx:127` 跟着换。
6. 状态类筛选的空状态：`lib/library-filters.ts:223`（`libraryEmptyNote`）→ `pages/v2/library.tsx:675-690`。搜了关键词：仍是「没有找到 / 换个说法试试…」+「去找方案」；只点「待 AI 整理」：「没有待整理的收藏，都整理好了。」；想试 / 试过 / 在用 / 不要了（以及没看、场景、类型、AI 已整理，或几个叠在一起）：「这一类还没有。」，这两种都不给「去找方案」。测试 `library-filters.test.ts:390`。
7. 资料库行悬停：`lib/item-format.ts:266`（`DROP_HINT_LIBRARY`）→ `components/library/item-row.tsx:24,74`（新增可选 prop `dropHint`，默认还是今日页那一句）→ `pages/v2/library.tsx:702` 传资料库那句；今日页没传，保持原句。测试 `item-row.test.tsx:69`（含 `?raw` 守卫：资料库传了、今日页没传）。
8. 设置 → AI 设置：`lib/ai-config.ts:151`（`getAiConfigMessage(ai, { onAiSettingsPage })`）→ `pages/settings.tsx:915,1002`。AI 设置页里：「还没设置 AI。先在上面选一个服务商，填好密钥和模型名，再点「测试连接」。」；别处（GitHub 页的运行检查、欢迎引导、各按钮悬停）仍是「去「设置 → AI 设置」…」。测试 `lib/ai-config.test.ts:129`（含 `?raw` 守卫：AI 设置页里 2 处都传了、GitHub 页没传）。
9. 设置 → GitHub：`lib/automation-view.ts:735-742`、`:804`；`pages/settings.tsx:404`。「令牌是 GitHub 给你的一串密码，应用用它读你的收藏。」是完整的一句；连接后的「访问配置」里只写「令牌已保存在 Mac 的钥匙串里，界面不会显示它。」不再重复解释；权限：设置页「创建令牌时，只读公开收藏的话一个权限都不用勾；还需要的话再勾「repo」（读你的私有收藏）和「gist」（上传备份用）。」，欢迎引导「怎么拿令牌」第 4 步同一句话（`GITHUB_TOKEN_EXTRA_SCOPES` 只定义一次）。测试 `lib/automation-view.test.ts:1061` 一带（卡 33 的两条期望换成新文案）。
10. 收集页：`components/capture/inbox-card.tsx:60-62` 标题去掉数字（页签上的数字保留）；`lib/capture-view.ts:193`（`emptyInboxCopy`）：待处理「没有待处理的链接。」（后面保留原来那行「上面粘贴后点「加入收集箱」，新链接会进来」）、失败「没有失败的链接。」、已归档「还没有归档的链接。」。测试 `components/capture/inbox-card.test.tsx:73,95`（新文件，整卡渲染）、`lib/capture-view.test.ts:273,285`。
11. 找方案预览：`pages/v2/solve.tsx:455`「看 Markdown 原文 / 看排版效果」；`lib/solve-view.ts:335`（`packSizeText`，不到一万不加千分位）；`lib/solve-view.ts:343-373`（新 `previewDisplayText`）→ `solve.tsx:540`：**只在排版预览里**把交接单里每个工具那一行 `★27.2k`、`最近更新 2026-07-01` 换成 `★2.7 万`、`最近更新 7 月 1 日`（不是今年的带年份、不到一万写原数字；认得出是哪个勾选的工具就用它真实的个数，不从 27.2k 倒推）；「看 Markdown 原文」和复制出去的仍是后端给的原文。测试 `lib/solve-view.test.ts:816,886`、`components/solve/solve-page.test.tsx`（整页渲染）。
12. 按类型看：`pages/v2/map.tsx:7-45`（新 `MapBlocks`：读取中「读取中…」、读完没有分类「还没有分类。同步并整理之后这里会出现。」、读出错只显示错误；页面卸载后不再写状态）。测试 `components/library/map-page.test.tsx:53`。
13. 资料库左栏类型：`lib/library-filters.ts:186`（`formOptions` 的 `label` 是短名字，新增 `hint` 放长解释）→ `components/library/filter-panel.tsx:68,103`（悬停 `title`）；详情页「类型」一行 `formTagLabel` 仍用长写法。测试 `library-filters.test.ts:177,205,214`、`filter-panel.test.tsx:113,127`。
14. 找方案列表顶上那句：`pages/v2/solve.tsx:408`「勾选的写进交接单，点一行看详情」（15 个字，比原来少 1 个；原来在 900 宽里前 15 个字放得下、第 16 个「情」折下去，所以 15 个字一定放得下，我选了「缩短」，没加 `nowrap`）。测试 `solve-page.test.tsx`。
15. 列表长名字：`components/library/item-row.tsx:49` 名字那一段加 `title={fullName}`，没改布局。测试 `item-row.test.tsx:98`。

### 8. 导出按钮的说明
- `lib/automation-view.ts:594`（`EXPORT_SECTION_HINT`）→ `pages/v2/toolbox.tsx:660`：区块说明末尾补「导出的表格用 Excel 打开中文不会乱码。」（按钮上原来就有「（CSV，可用 Excel 打开）」）。测试 `lib/automation-view.test.ts:912`。注意：这句话要等卡 37 给 CSV 加上 BOM 才名副其实，两张卡要一起合并。

## 第 5 条 grep 的输出里范围外的残留清单
命令：`rg -n "工具箱|地图|参考包|方向|仓库|理解卡|增量整理|全量整理" apps/desktop/src -g '!*.test.ts' -g '!*.test.tsx'`（不含测试文件）→ 138 行 / 36 个文件（改之前 140 行）。分类（`old-terms.test.ts` 只豁免下面 A–D）：

- **A. 旧页面和它们专用的东西**（新导航里没有入口；总控决定「旧页面文件不动」）：`pages/repositories.tsx`（27）、`pages/ai-search.tsx`（20）、`pages/tag-network.tsx`（13）、`pages/dashboard.tsx`（8）、`pages/profile.tsx`（3）、`pages/discover.tsx`（3，旧的那个，不是 `pages/v2/discover.tsx`）、`features/repositories/repository-table.tsx`（4）、`features/repositories/discovery-selection-bar.tsx`（1）、`features/rankings/personal-rankings.tsx`（2）、`features/rankings/ranking-list.tsx`（1）、`features/rankings/ranking-detail-panel.tsx`（1）、`features/knowledge/knowledge-panel.tsx`（2）、`features/discovery/recommendation-list.tsx`（1）、`features/discovery/recommendation-detail-panel.tsx`（1）、`components/copy-link-button.tsx`（4，只有旧页面在用）、`components/app-shell.tsx`（1，没有任何地方 import）、`lib/repository.ts`（1，只有旧 features 在用）。
- **B. 没有任何入口的页面**：`pages/v2/packs.tsx:12,21`（「参考包」；`PAGE_NAV` 里有它，但全仓库没有任何地方 `setCurrentPage('packs')`，而且它调的 `create_reference_pack` 会写库），没动。
- **C. 隐藏的向量面板**（审读 15：页签藏起、代码留着）：`pages/settings.tsx:1755,1986,2156`（「仓库」）。设置页看得见的部分有卡 33 自己的守卫（`automation-view.test.ts:1205`），没动。
- **D. 复制出去给 Codex 的话**（不是界面上的字，按卡 31 / 32 / 33 的约定不改）：`lib/codex-prompts.ts:104`「只算这一个仓库（排除同名的其他项目）」。
- **E. 只出现在代码注释里，不算**：`lib/contracts.ts`（17）、`lib/repo-assets.ts:2,35,324,352`、`lib/nav.ts:16,37`、`styles.css:387,1023`、`lib/api.ts:172`、`hooks/use-library-detail.ts:11`、`hooks/use-repo-community.ts:16`、`hooks/use-repo-readme.ts:22`、`components/kit/{chips,cover-image,repo-avatar}.tsx`、`components/readme-renderer.tsx:13-14`、`components/solve/format-picker.tsx:10,16`（「方向键」是键盘的方向键）、`components/library/item-detail.tsx:400`、`pages/v2/library.tsx:52`、`pages/v2/solve.tsx:107`（后三处注释里还写着「工具箱」，可以顺手改，我没动）。

## 改了哪些文件
- `lib/item-format.ts`：`monthDay` 本机时区、「最近 7 天」、`todayEmptyCopy`、资料库「不要了」说明、项目说明没下载的三处说法、合集按钮、14 个以内。
- `lib/library-filters.ts`：技能的解释、类型短名字 + 悬停解释、`libraryEmptyNote`。
- `lib/nav.ts`、`App.tsx`：页签名、删掉覆盖。
- `lib/capture-view.ts`、`components/capture/inbox-card.tsx`：收集页标题、空状态。
- `lib/solve-view.ts`、`pages/v2/solve.tsx`：预览按钮、字数、星数和日期、列表提示。
- `lib/ai-config.ts`、`pages/settings.tsx`：AI 设置页里的提示、令牌那一段。
- `lib/automation-view.ts`：令牌 / 权限常量、欢迎引导「怎么拿令牌」、导出说明。**（卡 37 也改这个文件和它的测试，见「给审核者的提示」）**
- `lib/task-progress-view.ts`（新）、`components/app-layout.tsx`、`components/welcome-flow.tsx`：任务条。
- `lib/repo-assets.ts`：Zread 悬停提示。
- `components/library/`：`detail-panels.tsx`、`item-detail.tsx`、`item-row.tsx`、`filter-panel.tsx`、`thin-tag.tsx`（注释）；`pages/v2/`：`today.tsx`、`library.tsx`、`map.tsx`、`toolbox.tsx`。
- 测试（改写 / 新增）：`lib/{nav,item-format,library-filters,capture-view,solve-view,ai-config,automation-view,discover-view,repo-assets}.test.ts`、`components/library/{detail-panels,item-detail,item-row,filter-panel,map-page}.test.tsx`、`components/solve/solve-page.test.tsx`、`components/discover/scenario-row.test.tsx`；新文件 `lib/task-progress-view.test.ts`、`lib/old-terms.test.ts`、`components/app-layout.test.tsx`、`components/capture/inbox-card.test.tsx`、`components/library/today-page.test.tsx`。

## 验证
- `tools/cards/run.sh web-build` → 通过（`tsc --noEmit` + `vite build`）。
- `tools/cards/run.sh web-test` → 32 个文件、766 项通过（起点 647）。
- `TZ=Asia/Shanghai` / `America/Los_Angeles` / `UTC` / `Pacific/Kiritimati` / `Pacific/Pago_Pago` / `Asia/Kolkata` / `Australia/Lord_Howe` 下 `tools/cards/run.sh web-test` → 都全过。
- `tools/cards/run.sh rust-fmt` → 通过。
- `tools/cards/run.sh rust-check` → 通过，6 条警告（`repair_model_json`、`restore_database_file` 等，都是起点就有的，我没碰 Rust，没有新增）。
- `tools/cards/run.sh rust-test` → `1164 passed; 0 failed; 5 ignored; 1 filtered out`（5 个 `#[ignore]` 是原来就有的；1 个钥匙串用例被脚本跳过）。在最终状态上跑的。
- 反向验证（改坏实现、跑、还原）：见验收第 1 条。还原后用 `cp` 从备份还原，再跑 `web-test` 确认全绿，备份已删。
- 没有启动 App，没有在窗口里看过任何界面（红线）：布局类的改动（左栏类型行、列表顶上那句、任务条）靠渲染测试（文字、`title`、类名）验证。

## 没做到的 / 偏离任务卡的地方
没有「没做到」的条目。下面是我自己拿了主意、和任务卡字面有出入的地方：
1. **7.2 概览那句我只换了第一半**：「还没下载项目说明，暂时看不到安装步骤。」后面仍接着原来的下一步（「点「打开 GitHub」看发行版或文档；也可以直接复制给 Codex，让它去读项目说明。」），因为这句对没下载的工具同样成立。**预览页**同理：标题照卡，下面的说明换成了「怎么补上」（原来那句「这个工具没有放界面或效果图」在没下载时是错的）。要严格只留卡里那一句，改 `item-format.ts:295-315` 一处。
2. **7.2 加了读取中 / 读出错两种说法**：原来读取中也会闪一下「没有现成的安装步骤」，读完又变；现在读取中写「正在读取项目说明…」。
3. **7.2「怎么试」页那句同样指错地方**，卡里只点了「项目说明页」，我一并改了（`detail-panels.tsx:139`），并更新了它的既有测试期望。
4. **7.3 多改了一句**：今日「最近 7 天」点进去是空的时候，原来写「这一周没有新收藏」，同样改成「最近 7 天没有新收藏」。
5. **7.6**：卡里的句子写的是「没有待整理的收藏，都整理好了。」——统一用词表要的是「待 AI 整理」，今日页同类的句子是「都整理好了，没有在等 AI 写说明的收藏」。我**照卡逐字用了**，没改；建议总控考虑改成「没有待 AI 整理的收藏，都整理好了。」。另外我把「这一类还没有。」也用在了场景 / 类型 / 「AI 已整理」筛选和几个筛选叠在一起的情况（它们同样是「把筛选当搜索」）；叠了别的筛选时不说「都整理好了」（可能只是这个场景里没有）。搜了关键词时一律按搜索说。
6. **7.8**：卡只点了没选服务商那一句；「请求地址 / 模型名 / 密钥没填」那三句在 AI 设置页里也是「去设置 → AI 设置」，我同样做了「这一页」版本（「请先在上面填写模型名。」等），其余页面不变。
7. **7.9 的措辞是我拼的**：卡只给了「令牌是 GitHub 给你的一串密码，应用用它读你的收藏。」和「勾「repo」（读你的私有收藏）和「gist」（上传备份用）」两段；权限那一整句是「创建令牌时，只读公开收藏的话一个权限都不用勾；还需要的话再勾……」。欢迎引导里「还没连接」输入框下面那句保留了令牌的解释（那里是第一次提到令牌），写成「令牌是 …。它只会保存在 Mac 的钥匙串里，不会上传到任何服务器。」
8. **7.10**：待处理的空状态也换成了卡里的「没有待处理的链接。」，下面保留原来那行说明（怎么放进收集箱）；内部状态 `ready` 界面上本来叫「待处理」，空状态同样写这句（原来写「这里没有内容」，测试期望换了新句）。
9. **7.11 星数 / 日期**：这两处是后端 `packs.rs` 生成的交接单正文，卡 36 不许碰 Rust（`packs.rs` 的测试也钉着 `★1.2k` 这种写法，复制出去给 Codex 的也仍是这个写法），所以做在前端、**只改排版预览**，复制和「看 Markdown 原文」不变。
10. **任务条（审读 47）的各阶段说法是我写的**：审读只给了「读取收藏」一个例子，其余（下载项目说明、写中文说明、保存到这台电脑……）见 `lib/task-progress-view.ts:11-27`；已经结束的任务（完成 / 失败 / 部分完成）不再标「正在做」（标题已经说了）；认不出的阶段不再把英文代号露出来（原来 `default: return stage`）。欢迎引导里的任务条（`welcome-flow.tsx`）卡里没点名，也是同样的「阶段：」「3/12 · 25%」，我用同一个函数一并改了。
11. **`MapBlocks`、`TaskProgressCard` 是为了能渲染测试才新增 / 加了 `export`**；`getAiConfigMessage` 多了可选的第二个参数；`FormFilterRow` 多了 `hint` 字段（只有资料库页在用）。都没有改命令名、JSON 键名和状态逻辑。

## 需要总控或 Fox 决定的事
1. **7.6 的「待整理」**：用「没有待整理的收藏」还是「没有待 AI 整理的收藏」（见上第 5 点）。
2. **合集的「复制给 Codex」粘出去的话**：按钮改成了「帮我看看怎么用」，但粘出去的正文（后端 `copy_item_for_codex` → `packs::render_item_for_codex_with`）和点完的提示（「已复制。粘到 Codex 后，它会先查你电脑上有没有，没有就装进独立文件夹…」）对合集仍是「装好并试用」那一套，这两处要改要动 Rust / 提示常量，不在本卡范围。
3. **「同步」一次下载的项目说明有上限**：后端同步里的项目说明跟进每次最多重新抓 150 个（`readme_follow.rs:13`，没有缓存的优先）。「点顶栏的「同步」……就会补上」对缺得多的人，可能要点几次；「设置 → GitHub」的「下载项目说明」是一次补完所有缺的。要不要在文案里说明，请定。
4. **交接单正文里的 `★27.2k` 和 `2026-07-01`**：只在预览里换了写法；要连复制出去的一起换，需要改 `packs.rs`（和它逐字断言的测试），不在本卡范围。
5. **一处顺手发现、没动的小毛病**：发现页那句 `上次去 GitHub 找是2 天前。`（`lib/discover-view.ts:16`）数字前少个空格；这句在卡 32 的范围里，不在卡 36 点名的条里，没改。

## 给审核者的提示
- **`lib/automation-view.ts` 和 `lib/automation-view.test.ts` 卡 37 也要改**（`RUNTIME_CHECK_TITLES` 一带，约 717–727 行；测试里对应 1031–1041 行的那一块）。我的改动在别的位置：`automation-view.ts:594`、`:735-742`、`:804`；测试里的 import 列表、`:912` 一块、`:1061` 一带令牌那几条、`:1274` 一条。离得最近的一处和卡 37 的区域隔了 3 行以上，应该能自动合并；合并时请看一眼 import 列表。
- **新守卫 `lib/old-terms.test.ts` 没放进 `automation-view.test.ts`**，就是为了避开上面这个文件的冲突。它读全部 `.ts/.tsx` 源码，去掉注释后查八个词；上面 A–D 类被豁免，设置页（`pages/settings.tsx`）整个豁免（它有卡 33 自己的守卫，会把隐藏的向量面板排除在外）。以后谁往新界面里写回旧叫法，它会指出文件和行号。
- **一处我改了的既有守卫**：`automation-view.test.ts:1274`（原来断言 `App.tsx` 里有 `label: '自动整理与备份'`，覆盖删掉之后这条必然要换）。除此之外没有删除或放宽任何既有断言，被我改过的都是「期望字符串换成新文案」或「输入换成本机时间构造」，上面各条里都写了。
- 合并后请在 dev 版里点一遍：顶栏页签（设置旁边的页签名）；资料库左栏「类型」（短名字、悬停长解释）和各种筛选空了的话；详情页三个没下载项目说明的页（概览 / 怎么试 / 预览 / 项目说明）和合集的底部按钮；今日统计卡；收集页标题和三个空状态；找方案预览（按钮名、★ 和日期、字数、顶上那句有没有折行）；按类型看；设置 → GitHub / AI 设置；欢迎引导；顶栏任务条。
- 过程里的小失误，如实写：① 为改 `lib/nav.test.ts` 用 `python3` 做过一次文本替换（另跑过一次 `python3 --version`），不联网、只写工作区里这个文件，之后改用 Edit / `sed`；② 有一条命令手滑带了 `cat > /tmp/__noop`（在工作区外写了一个空文件），下一条命令就 `rm` 了，已 `ls` 确认不存在；另有一条命令带了 `tee /dev/null`（只是个设备文件，没有写任何文件）。没有别的越界。
