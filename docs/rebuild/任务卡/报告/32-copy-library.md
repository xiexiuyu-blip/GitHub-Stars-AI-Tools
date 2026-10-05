# 交回报告 · 卡 32-copy-library（续作收尾）

## 一句话结论
续作「必须做」1–8 和「小修」全部做完，`web-build` / `web-test`（474 项）/ `rust-fmt` / `rust-check` / `rust-test`（1138 项）全过；有 9 处我按「最保守」做了取舍，写在「没把握」里，请总控过目。

## 续作逐条（条号 → 文件:行 → 测试）

| 续作条 | 做到了吗 | 改在哪 | 测试（断言实际渲染 / 返回的文字） |
|---|---|---|---|
| 1 地图页补完 | 是 | `lib/nav.ts:35` label「地图」→「按类型看」（key 仍是 `map`），注释 `:16`；页内标题和说明早已是 `pages/v2/map.tsx:16-17`；命令面板里没有同名入口；`App.tsx:381` 的页签条直接读 `nav.ts`，自动跟着变。「写库」那件事见下面专节 | `lib/nav.test.ts:75`；`components/library/map-page.test.tsx:12`（渲染页面：有「按类型看」和「目前只能看，不能改。」、没有按钮、没有「合并」；源码里没有 `merge_knowledge_concepts`；页面只调 `get_graph_overview`） |
| 2 「偏少」标签太宽 | 是 | 标签字只写「偏少」，「少于 14 个」在 `title`：新 `components/library/thin-tag.tsx:11`（外包 `inline-flex shrink-0`，里面是原来的 `Tag`）；资料库左栏 `filter-panel.tsx:45`；发现页那一行抽成 `components/discover/scenario-row.tsx`（类名和 DOM 不变，场景名那段 `min-w-0 flex-1 truncate` 保留），`pages/v2/discover.tsx` 改用它；常量 `lib/item-format.ts:206-207` | `filter-panel.test.tsx:43-104`、`scenario-row.test.tsx:32-86`：断言 `title="少于 14 个"`、标签包装层有 `shrink-0`、场景名有 `truncate` + `min-w-0` + `flex-1`、标签文字里没有「少于 14 个」 |
| 3 「收藏于」不带钟点 | 是 | `lib/item-format.ts:44-56` `monthDay(iso, now)`：只写日期，不是今年的写「2025 年 10 月 3 日」；`item-detail.tsx:157` 传 `now`。别处本机时间产生的「X 月 X 日 18:17」没碰 | `item-format.test.ts:97-129`（UTC 时间 `18:17` 不出现、带年份、`now` 可以是 Date / 毫秒 / 字符串）；`item-detail.test.tsx:80-100`（渲染出来「收藏于 10 月 3 日」、没有 `18:17`、去年的带年份） |
| 4 复制出去的正文改回去 | 是 | `lib/item-format.ts:159-160` `codexFallbackText` 的两句原样改回（「AI 整理的上手提示：」「请先阅读 README…不要编造安装步骤。」），并在注释里写明这是复制出去的正文；同时 `lib/codex-prompts.ts` 去掉了上一轮为绕开类型名而加的私有名字表，`buildInstallPrompt` 又直接用 `formLabel`（第 5 条后它就是原来的短名字），正文和 `HEAD` 一样 | `item-format.test.ts:199-242`：原来的三条断言一字不差放回（`AI 整理的上手提示：npm i tool`、`不要编造安装步骤`、`not.toContain('上手提示')`），另加两个整段逐行对得上的断言；`codex-prompts.test.ts:116` 逐个类型断言正文里是短名字且不带括号 |
| 5 类型名长写法 | 是 | `lib/library-filters.ts:31-73`：`formLabel()` 回到改动前的短名字（技能 / MCP / 命令行 / 应用 / 代码库 / 接口 / 服务 / 合集）；新增 `formLabelWithHint()` 返回长写法，只被筛选面板（`formOptions`，`:180`）和详情页「类型」一行（`formTagLabel`，`item-detail.tsx:109`）用。找方案行里的 `solve-hit-row.tsx:29` 仍是 `formLabel`，所以保持短名字 | `library-filters.test.ts:301-352`（短名字不带括号且 ≤ 4 字；长写法逐项；`formLabelWithHint` 对空 / 其他返回 null）；`filter-panel.test.tsx:106-125`；`item-detail.test.tsx:102-133` |
| 6 复制按钮和提示 | 是 | 字符串只在 `lib/item-format.ts:245-263` 定义一次：`COPY_INSTALL_BUTTON`、`COPY_EVALUATE_BUTTON`、`COPY_FAILED_NOTICE`、`copiedNotice(kind)`。详情底部按钮 `item-detail.tsx:379`；发现页按钮 `candidate-card.tsx:81`；成功提示 `library.tsx:422,638`、`today.tsx:319,431`、`discover.tsx:115`；失败提示 `use-copy.ts:15`、`library.tsx:402`、`today.tsx:299`、`discover.tsx:117` 全部引用同一个常量 | `item-format.test.ts:439-462`；`item-detail.test.tsx:262`；`candidate-card.test.tsx:41`；`detail-panels.test.tsx:81-136`（直接调组件函数、点「复制」，断言交出来的是 `install` / `community` 哪一种） |
| 7 「灵感」降级 | 是 | `item-detail.tsx:310-314`：从带边框的卡片改成「怎么试」卡片下面一小段灰字（`section.px-1` + 小标题「灵感」+ `text-xs text-on-surface-variant` 正文），内容为空整段不渲染；「Fox」→「你」的替换保留（`item-format.ts:233`，改成整词匹配，Firefox / Foxmail 这类词里的不动） | `item-detail.test.tsx:230-260`（没有 `border` / `rounded-lg` / `p-4`、位置在「怎么试」后面、null / 空串 / 空白都不显示、Fox 换成你）；`item-format.test.ts:415-437` |
| 8 补测试、去重 | 是 | Rust：`discover.rs:1166`、`:1214` 的断言改回字面量；上一轮新增的 3 个近乎重复的测试（和 `:1157`、`:1184` 两个老测试是同一场景）合并成 1 个，另加 2 个不重复的（见下）。前端：`use-regenerate.ts` 的判断抽成纯函数 `regenerateToast`（`item-format.ts:271`，hook 只剩调用）；3 个只比对常量的测试（`RISKS_EMPTY`、`DROP_HINT`、`DISCOVER_NO_SCENARIO` / `ADDED_TO_INBOX`）改成断言实际渲染 / 返回的文字，另有一个 `INSTALL_COPY_HINT` 的也改成渲染 | Rust：`discover.rs:1567`（等 3 秒和等 15 分钟说同一句、提示里没有数字 / 「秒」/「限流」、等待时间只记在 `rate_limit_reset_at`）、`:1597`（读「已收藏」名单失败时报错说「工具」，两个失败点）、`:1617`（找过但没有新的 vs 没找过：候选数都是 0，区别只在 `last_fetched_at`——前端靠它分「没有结果」和「还没找过」）。前端：`item-format.test.ts:464-521`（`regenerateToast` 逐个分支）、`item-detail.test.tsx:199-228`、`item-row.test.tsx:36-85`、`discover-view.test.ts:188`、`detail-panels.test.tsx:81` |

### 小修（低）

| 小修 | 做到了吗 | 改在哪 / 测试 |
|---|---|---|
| `paste-card.tsx` 「10/03」→「10 月 3 日」 | 是 | `components/capture/paste-card.tsx:45`；`paste-card.test.tsx:5` |
| `aria-label` 改回短名字 | 是 | `item-row.tsx:102`、`pick-card.tsx:90` 的 `aria-label` 回到 `props.label`（「不要了（X）」）；长说明只放在 `title`（`:101`、`:89`）；`item-row.test.tsx:36-85` |
| `discover.tsx` 嵌套括号 | 是 | `pages/v2/discover.tsx:126` 恢复成「场景（偏少的在前面）」（「少于 14 个」已在悬停提示里） |
| 0 个结果分「没找过」/「找过」 | 是（格子里的字见「没把握」2） | `lib/discover-view.ts:60-69`；`discover-view.test.ts:188`；`scenario-row.test.tsx:51-78` |
| 页签「安装」→「怎么试」 | 是 | `item-detail.tsx:125`（`key: 'install'` 不变）、`:305` 链接文字跟着改成「去「怎么试」页…」；`item-detail.test.tsx:164-197` |
| capture-view「新收 / 没有收 / 不会收」 | 是 | `lib/capture-view.ts:55-59,78,102,244`：新的 / 没放进收集箱 / 不会放进收集箱（「这段已经收过」顺带改成「这段已经放进过收集箱」）；`capture-view.test.ts` 对应断言换成新文案（没有删） |
| 「方向」→「场景」 | 是 | `item-format.ts:196` `pickReason`，同句的今日栏目提示 `today.tsx:463` 也一并改；`item-format.test.ts:348` |
| `discover.rs` 报错「仓库」→「工具」 | 是 | `discover.rs:400,403,406`；测试 `:1597` |
| 「项目说明（README）」首次出现处补括号 | 是 | `item-format.ts:210-225`：有中文说明且依据项目说明时，括号写在说明旁边那句「依据项目说明（README）」（`item-detail.tsx:176`），页签只写「项目说明」；没有中文说明、或说明只依据简介（审定整句不带括号）时，括号写在页签上（`item-detail.tsx:127`）。整个详情页括号只出现一次；`item-format.test.ts:383`、`item-detail.test.tsx:164-197` |

## 续作第 1 条要的说明（地图页）
- **现在打开页面还会写库。** 页面只调一个命令 `get_graph_overview`（`map.tsx:10`），它在 `lib.rs:1249-1253` 里依次跑：`seed_taxonomy`（`v2_ops.rs:479`，`INSERT OR IGNORE INTO categories` 补种子分类）→ `rule_classify`（`v2_ops.rs:498`，对每个 active 仓库按关键词规则 `INSERT OR IGNORE INTO entity_categories`，`origin='rule'`、`status='suggested'`、`confidence=0.5`）→ `graph_overview`（`v2_ops.rs:837`，只读）。
- 这些写入只追加、不改已有行、不删：`entity_categories` 主键是 `(entity_type, entity_id, category_id)`，每个对象只能有一个主分类（`022_taxonomy.sql:29` 的唯一索引），所以重复打开不产生重复行，也盖不掉已有的（含你确认过的）分类；只是新收藏的东西会被悄悄补上一条「建议」分类。这件事按续作要求没改，也没新增只读命令。
- **删掉「合并」按钮之后页面仍能正常显示分类**：`map.tsx:19-26` 仍把 `get_graph_overview` 返回的每个分类（中文名 + 个数）画成卡片，数据读取没动；去掉的只是按钮、「规则初分合计」和成功提示。命令 `merge_knowledge_concepts` 仍在 `lib.rs:1567`、`:7588`（没碰）。
- 注意：我只能测到静态渲染（标题、说明、没有按钮、源码里没有合并命令、只调 `get_graph_overview`）；「有数据时画出卡片」那一段在异步读取之后，项目里没有 DOM 测试环境，没覆盖到。
- 想让「打开页面不写库」，要改 `lib.rs get_graph_overview`（或另给一个只读命令）：归总控决定。

## 验收逐条（任务卡原验收 + 续作验收）
| 验收项 | 做到了吗 | 证据 |
|---|---|---|
| 前端新增至少 14 个测试，覆盖成熟度 / 类型 / 小标签（含 stale）、依据项目说明两种显示、「注意」为空灰字、灵感 Fox→你、收集页页签没有「已整理」、capture-view 各状态、discover-view 限流 / 空状态 / 「—」→「还没找过」、community-view 状态词、codex-prompts 界面说明、日期 / 数字格式化 | 是 | web-test 从 389 项（16 个文件）到 474 项（24 个文件）；对应位置：成熟度 / stale `item-format.test.ts:362`、`item-detail.test.tsx:102`；依据两种显示 `item-detail.test.tsx:135`；注意灰字 `:199`；灵感 `:230`；收集页页签 `capture-view.test.ts:283`（页签里没有 ready）；discover-view `discover-view.test.ts:188`；community-view `community-view.test.ts:95`；codex-prompts `detail-panels.test.tsx:81`、`codex-prompts.test.ts:116`；日期 / 数字 `item-format.test.ts:31-129` |
| Rust 改了原句的更新测试；新增至少 3 个 | 是 | 3 个新测试：`discover.rs:1567`、`:1597`、`:1617`；两个老测试的期望是字面量（`:1166`、`:1214`）。`library_view.rs` 里没有「你常做」原句（只有常量 `CORE_SORT_MAX`），没改 |
| 地图页：合并按钮已没有；页面里不再引用「合并」命令（命令本身留着） | 是 | `map.tsx` 全文没有按钮和 `merge_knowledge_concepts`；`map-page.test.tsx` 守住 |
| `web-build`、`web-test`、`rust-fmt`、`rust-check`、`rust-test` 全过，警告数不增 | 是 | 见「验证」 |
| `git diff --stat` 只出现允许改动里的文件（加 `nav.ts`、`nav.test.ts`） | 是 | 31 个改动文件 + 10 个新文件（`components/library/thin-tag.tsx`、`components/discover/scenario-row.tsx` 和 8 个 `*.test.tsx`），全在 `components/library/**`、`components/capture/**`、`components/discover/**` 或允许清单里 |

## 原卡条号 → 文件:行（上一轮已做的部分，行号按现状更新；本轮只有标「本轮」的有变化）
| 条 | 状态 | 位置 |
|---|---|---|
| 1 待整理三种口径 | 部分 | 今日 `today.tsx:48,393`；资料库 `library-filters.ts:200-201`、`library.tsx:503-504`；收集页只留「待处理 / 失败 / 已归档」`capture-view.ts:119-125`（内部 `ready` 显示「待处理」`:129`）。找方案行标签在 `solve-hit-row.tsx:71`，归卡 31 |
| 3 仓库 / 工具 / 收藏 | 部分 | 「你已经收藏了」`capture-view.ts:99`、`candidate-card.tsx:64`；收集箱说明 `inbox-card.tsx:75-77`。「整理新收藏…」「495 个工具」「有 N 个工具已不在资料库里」在 toolbox / 顶栏 / 找方案，归卡 31 / 33 |
| 10 地图页 | 是（本轮） | 见「续作逐条」1 和第 1 条专节 |
| 17 空状态 | 是 | `today.tsx:510-524` |
| 18 推荐理由 / 栏目提示 | 是（本轮改成「场景」） | `item-format.ts:196`、`today.tsx:463`；`library_view.rs` 里没有「你常做」原句 |
| 19 「你标的」/「不要了」悬停 | 是 | `today.tsx:394-395`；`item-format.ts:200`，用在 `item-row.tsx:71`、`pick-card.tsx:78`（`aria-label` 本轮改回短名字） |
| 20 收集箱三行说明 | 是 | `inbox-card.tsx:75-77`、`:214`；`capture.tsx:363` |
| 21 发现页 | 是（本轮改了个数格子 / 空状态） | `candidate-card.tsx:64,77`；`discover-view.ts:43-100`；`discover.rs:18-19,321-322` |
| 32 说得对吗 / 没开自动整理 | 是（本轮抽成纯函数） | `item-detail.tsx:187-189`；`item-format.ts:271`、`use-regenerate.ts:21` |
| 33 复制一段话的界面说明 | 是 | `codex-prompts.ts:15`、`detail-panels.tsx:140`；复制出去的正文没改 |
| 34 类型名 / 小标签 / 可正式使用 / stale | 是（本轮分短 / 长两套） | `library-filters.ts:31-73`、`filter-panel.tsx:62`；`item-format.ts:73-79`；`item-detail.tsx:164-165` |
| 36 场景是自动猜的 | 是 | `library.tsx:534` |
| 38 项目说明 | 是（本轮补括号规则） | `item-detail.tsx:30,127,176`；`detail-panels.tsx:50,131,164-165,262-263`；`item-format.ts:210-225` |
| 45 发行版 / 问题反馈区 / 还没解决 | 是 | `detail-panels.tsx:124,354,364`；`community-view.ts:36-40` |
| 46 键位 / 复制失败 / 数字 / 日期 / 项目最近更新 | 部分 | 键位 `library.tsx:611`、`today.tsx:405`；复制失败 `use-copy.ts:15` 等（本轮统一成一个常量）；数字 `item-format.ts:16-20`；日期 `:44-56`（本轮改成只写日期）；`item-detail.tsx:156`、`candidate-card.tsx:57`。方向键在 `hooks/use-list-keys.ts`，不在允许清单 |
| 47 合并块 / 偏少 / 还有 N 个场景 | 部分 | 合并块 `item-detail.tsx:239-252`；偏少见续作 2；`item-detail.tsx:163`。任务条在 `app-layout.tsx`，归卡 33 |
| 49 资料库回退提示 | 是 | `library.tsx:598` |
| 展示小修 1 / 2 / 3 | 是 | 依据两句 `item-format.ts:213-216` → `item-detail.tsx:176`；「注意」为空灰字 `item-detail.tsx:267-283` + `item-format.ts:203`；灵感 `item-detail.tsx:310-314`；被截断的句子按卡里说的没处理 |

## 改了哪些文件
- `lib/item-format.ts`：日期只写日期；`codexFallbackText` 改回原文；偏少 / 复制 / 重写提示 / README 括号等常量和纯函数；灵感整词替换；「方向」→「场景」。
- `lib/library-filters.ts`：短名字 / 长写法两套类型名。
- `lib/codex-prompts.ts`：去掉私有名字表，`buildInstallPrompt` 回到 `formLabel`（正文与 HEAD 相同）；保留界面说明常量。
- `lib/discover-view.ts`：个数文字 / 悬停提示、空状态、放进收集箱提示（纯函数）。
- `lib/capture-view.ts`、`lib/nav.ts`、`lib/community-view.ts`：用词。
- `hooks/use-copy.ts`、`hooks/use-regenerate.ts`：引用上面的常量 / 纯函数，接口没变（`hit-drawer.tsx` 在用）。
- `components/library/`：`item-detail.tsx`、`detail-panels.tsx`、`filter-panel.tsx`、`item-row.tsx`、`pick-card.tsx`，新增 `thin-tag.tsx`。
- `components/discover/`：`candidate-card.tsx`、`index.ts`，新增 `scenario-row.tsx`。
- `components/capture/`：`inbox-card.tsx`、`paste-card.tsx`。
- `pages/v2/`：`today.tsx`、`library.tsx`、`capture.tsx`、`discover.tsx`、`map.tsx`。
- `src-tauri/src/discover.rs`：限流固定句、报错用词、测试。
- 测试：`item-format` / `library-filters` / `capture-view` / `discover-view` / `community-view` / `codex-prompts` / `nav` 的 `*.test.ts`；新增 8 个组件渲染测试（用 `react-dom/server` 的 `renderToStaticMarkup`；`map-page.test.tsx` 用 Vite 自带的 `?raw` 读页面源码原文；都没加依赖）：`components/library/{item-detail,filter-panel,item-row,detail-panels,map-page}.test.tsx`、`components/discover/{scenario-row,candidate-card}.test.tsx`、`components/capture/paste-card.test.tsx`。

## 验证
- `tools/cards/run.sh web-test` → 24 个文件、474 项全过（上一轮 389 项）。
- `tools/cards/run.sh web-build` → `tsc --noEmit` + `vite build` 通过。
- `tools/cards/run.sh rust-fmt` → 通过。
- `tools/cards/run.sh rust-check` → 通过；6 条既有 `dead_code`（`db/migrations.rs:14`、`scenarios.rs:115`、`spend_guard.rs:495`、`v2_ops.rs:81 / 613 / 1238`），都在没动的文件里，没有新增。
- `tools/cards/run.sh rust-test` → `1138 passed; 0 failed; 5 ignored; 0 measured; 1 filtered out`。
- 变异检查（临时改坏实现再跑测试，跑完已还原，没留在工作区）：32 处前端变异（地图标签改回、偏少标签变长 / 丢 `shrink-0` / 丢 `title` / 场景名丢 `truncate`、日期带钟点、`codexFallbackText` 变宽、`formLabel` 变长、各复制提示 / 按钮字改回、灵感改回卡片 / 空也显示 / Fox 不换、`regenerateToast` 丢新句、aria-label 改成长句、页签改回「安装」、README 括号写两处、注意灰字去掉、地图页加回合并按钮…）和 5 处 Rust 变异（提示里带秒数、报错改回「仓库」、改限流句、列表丢 `last_fetched_at`）全部被至少一个测试拦住。
- 注意：`tools/cards/run.sh web-test <过滤词>` 实际跑的是 `vitest run src <过滤词>`，`src` 本身也是过滤词，所以任何过滤词都不会缩小范围，每次都是全量（约 0.5 秒）。

## 没做到的 / 偏离任务卡的地方
- 无续作条目没做。仍然不在本卡范围、没动的见「跨卡缺口清单」。
- 偏离续作字面的几处，原因都在「没把握」：发现左栏格子里的字（2）、社区复制的提示（4）、`pickReason` / 今日栏目提示改了审定句里的「方向」（7）。

## 需要总控或 Fox 决定的事
- 「按类型看」打开时还会写库（见上面专节）：要不要改 `lib.rs get_graph_overview`，或另给只读命令。
- `scenarios.rs:964` 偏少是 `count <= 14`，刚好 14 个也标「偏少」，悬停写「少于 14 个」差一个边界：要不要改成「14 个以内」。
- 「收藏于」要不要折算成本机时区的日期（见「没把握」1）。

## 跨卡缺口清单（不在本卡允许改动里，没动）
- 卡 31：`components/solve/solve-hit-row.tsx:71` 行内标签仍是「待整理」（审读 1 要「还没有中文说明」）；`components/solve/hit-drawer.tsx:65,106` 复制成功仍是「已复制，去 Codex 里粘贴」，可以直接用 `copiedNotice('install')` / `copiedNotice(kind)`（`lib/item-format.ts`）；抽屉里 `ItemDetail` 的底部按钮现在会显示「复制「帮我装好并试用」给 Codex」（抽屉复制的是同一份条目交接单，字是对得上的）；`pages/v2/solve.tsx:297,337` 的提示和按钮（审读 5「复制交接单给 Codex」）；抽屉里「加入参考包 / 从参考包移除」（审读 2）。
- 卡 33：`lib/nav.ts:41` 的「工具箱」页签（审读 42）；`components/command-palette.tsx:8-11`「同步 Stars」「导出 Markdown」「AI 队列」（审读 35）；`components/app-layout.tsx:123,336,373` 的「仓库」字样、顶栏搜索（审读 3）、任务条（审读 47）；`pages/v2/toolbox.tsx:346,531,558` 的「仓库」（审读 3、6、25）；`pages/v2/solve.tsx:456`「有 N 个项目已不在库里」（审读 3）。
- 后端（`v2_ops.rs` 不在本卡允许清单）：`v2_ops.rs:847,856,858`「地图读取失败 / 地图查询失败 / 地图行读取失败」——「按类型看」读失败时会原样显示出来，里面还有「地图」二字。
- `hooks/use-list-keys.ts`：审读 46 要「补方向键支持」，不在允许清单，没做；界面上提示文字已改成「J / K 下一个 / 上一个」。
- `library_view.rs:966,971,974` 有三条报错写「方向排名…失败」，是旧叫法；这个文件在允许清单里，但续作没点名，没改。
- `contracts.ts`、`kit/chips.tsx` 里还有「方向」「仓库」的开发注释，不是界面文字，没动。

## 没把握 / 给审核者的提示
1. **收藏于只写日期，取的是字符串里的 UTC 日期，没有折算成本机时区**（按验收「UTC 日期不带钟点」的字面做）。北京时间 0–8 点收藏的，会比 GitHub 页面上的本机日期早一天。要折算：`monthDay` 对带 `Z` / 偏移的字符串改用本地年月日，测试里的时间要避开日界线。
2. **发现左栏格子里写「没有结果」，续作原句「找过了，这次没有结果」放在悬停提示里**。整句 10 个字放进 220px 的格子，场景名会被挤没（偏少的场景连名字都不剩），所以格子里用 4 个字（和「还没找过」同宽）。要整句直接显示，改 `discover-view.ts:60` 一处，但 `scenario-row.tsx` 得换成两行布局。
3. **「少于 14 个」差一个边界**，见上面「需要决定的事」。按续作原话没改。
4. **社区页「复制给 Codex」的提示没用安装那一句。** 详情里的 `onCopyPrompt` 同时给「让 Codex 帮你装」和「让 Codex 去查别人怎么用」用；后者只是让 Codex 搜资料，套「它会装进独立文件夹」是假话，所以回调多带了一个 `kind`（`install` / `community`），社区那条仍是原来的「已复制，去 Codex 里粘贴」。`hit-drawer.tsx` 里传的是只收一个参数的函数，类型上兼容。
5. **「新的 N 条」少了动词**：`describeCommit` 按续作映射改成「新的 4 条，2 条你已经收藏过了，…没放进收集箱。」，读得通但生硬；若要更清楚可以写「新放进收集箱 N 条」。
6. **README 括号的规则是我对「详情页首次出现处」的解释**（见小修表最后一行）。如果总控只想在固定一处加，改 `readmeTabLabel` 一个函数即可。
7. **「方向」→「场景」改了条 18 的两句审定文字**（`pickReason`、今日栏目提示），依据是续作小修 + 统一用词表；不妥就改回，`item-format.test.ts:348` 同步。
8. **没有目测。** 不能启动 App，布局类改动（偏少标签外包层、发现左栏那一行抽成 `ScenarioRow`）靠渲染测试（类名、`title`、文字）和读 CSS 验证；合并后请在 dev 版里看一眼资料库左栏（200px）和发现页左栏（220px）。
9. **提示条停留时间没改。** 安装类复制提示现在约 50 个字，提示条默认停 4.6 秒（鼠标放上去会暂停）；要加长，在 `library.tsx:422`、`today.tsx:319` 的 `toast.show` 里传 `durationMs`。
10. 复制出去的正文：`codexFallbackText`、`buildInstallPrompt`、`buildEvaluatePrompt`、`buildCommunityPrompt` 里仍有「README」「仓库」「我」等旧叫法，这是给 Codex 的话，按卡 31 / 33 / 32 的约定不改；`README_ONLY_PREFIX` 必须和后端 `card_pipeline.rs:794` 的 `NO_README_PREFIX` 一致，用来判断、不显示。
