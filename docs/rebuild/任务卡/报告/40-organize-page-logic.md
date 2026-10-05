# 交回报告 · 卡 40-organize-page-logic

## 一句话结论

做完了本卡前端实现和测试：讲清首次整理与每天增量整理，额度耗尽仍保留手动整理及确认入口；构建通过，前端全套 800 项通过。实际界面滚动、焦点效果待总控验收。

## 验收逐条

| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| 1. `tools/cards/run.sh web-build`、`web-test` 通过（`web-test` 在沙盒里跑不了时，说明原因，总控在沙盒外跑）。 | 是 | 两个命令实际运行，退出码均为 0；类型检查和 Vite 打包成功；34 个测试文件、800 项通过、0 失败。 |
| 2. `git diff --stat` 只出现「允许改动」里的文件。 | 是 | stat 只列 `automation-view.ts`、`automation-view.test.ts`、`item-format.test.ts`、`toolbox.tsx`；`git status --short` 另外列新增且未暂存的 `pages/v2/toolbox.test.tsx`，也在白名单内。没有 git 写操作。 |
| 3. 报告里写：条号 → 文件:行；改了哪些已有测试的期望；第 2 部分 `grep` 的输出里范围外的残留清单；没把握的地方。 | 是 | 下方实现对照、测试对照、既有期望清单、搜索残留及审核提示。 |

下表路径以 `apps/desktop/src/` 为前缀。

| 要做的事条号 | 文件:行及实现 |
|---|---|
| 后端新字段 / 缺失回退 | `lib/automation-view.ts:63`：五个新字段均可选且支持 null；`:168`：待整理数依次取 `awaiting`、`needsOrganizing`、`queued + running`，保留 0 为有效值。 |
| 1.1 自动整理定义 | `lib/automation-view.ts:106` 原文常量；`pages/v2/toolbox.tsx:478` 区块 hint 使用该常量。 |
| 1.2 状态徽章及 detail | `lib/automation-view.ts:375`：额度耗尽用滚动 24 小时文案、可选排队数、现在整理和调高额度入口；未开启时解释手动整理的确认和费用；保留其他状态和色相。 |
| 1.3 说明分项 | `lib/automation-view.ts:175` 计算新版数并夹在 0 以上；`:181` 三项齐全且总数大于 0 才显示；`pages/v2/toolbox.tsx:487` 放在状态提示之后、面板之前。 |
| 1.4 面板、确认及按钮 | `lib/automation-view.ts:293` 面板类型包含标题、说明、ready 正文和按钮文字；`:318` 按服务、运行及待整理数决定状态，不看每天额度；按新版数或旧后台的上次记录区分第一步与后续手动整理；`:562` 确认第一句更新，后两句逐字保留。`pages/v2/toolbox.tsx:152` 确认出现时 `scrollIntoView({ block: 'nearest' })`，聚焦带 `data-confirm-full-scan` 的确认开始按钮；`:362` 初次点击只展开；`:367` 确认后才调用原 `start_full_scan` 且仍带 `confirmed: true`。 |
| 1.5 README 行 | `pages/v2/toolbox.tsx:506`：仍紧接面板、位于每天自动整理之前。 |
| 1.6 每天自动整理、四档额度 | `lib/automation-view.ts:99` 档位数值保持 `[0, 100000, 200000, 600000]`，更新提示并改名多整理些；`:107` 三个每日说明常量；`pages/v2/toolbox.tsx:508` 接线。额度确认、自定义校验及保存逻辑原样保留。 |
| 1.7 第二块数字 | `lib/automation-view.ts:645` 纯函数：手动进行中不受限、关闭时隐藏、开启时显示余额及已用/额度；已用优先 `dailyTokensUsed`，缺失回退 `tokens24h`；`pages/v2/toolbox.tsx:578` 使用函数。第一、第三块未改。 |
| 1.8 慢慢整理折叠区 | `lib/automation-view.ts:110` 折叠标题、说明和按钮文案；`:190` 排队结果三种文案；`pages/v2/toolbox.tsx:351` 原入队命令只换结果说明，`:587` 原生 details 默认收起，暂停在外面。 |
| 2 其他位置额度用完文案 | `lib/automation-view.ts:412` 横幅、`:484` 未整理详情、`:510` 重写提示：耗尽文案逐字匹配卡；横幅及详情按钮改去整理，手动进行中统一使用「现在整理」。`:228` 上次整理记录原样保留。 |
| 不要做 | 对比 HEAD 的只读文本核对：`lastFullScanText` 函数、额度保存/自定义校验、备份及微信后续区块、全部 Tauri invoke 调用行均逐字相同。无 Rust、依赖、迁移、命令参数改动。 |

| 测试条号 | 证据（路径同上） |
|---|---|
| 1 额度耗尽仍 ready / 无待整理才 nothing | `lib/automation-view.test.ts:507`：`awaiting: 3` 与旧字段 0 并存仍 ready，标题、正文、按钮逐字断言；同一夹具改 `awaiting: 0` 才 nothing。 |
| 2 第一步 / 之后 | `lib/automation-view.test.ts:528`、`:548`：七组有/无分项、null/缺失/存在的上次记录，逐字断言标题、说明、正文、按钮；分项优先于上次记录，负的新版数夹到 0。 |
| 3 旧后台回退 | `lib/automation-view.test.ts:364`：旧两个回退测试保留，新增 awaiting 优先、0、null 测试；`:406` 旧后台面板各状态照常；`:594` 旧后台不显示分项。 |
| 4 分项行 | `lib/automation-view.test.ts:572`：逐字核对分项、旧版说明有/无附句、负值夹到 0、三字段分别缺失或 null、总数 0。 |
| 5 状态徽章 | `lib/automation-view.test.ts:174`、`:180`、`:189`：未开启和有/无排队数的耗尽文案逐字断言，禁止今天/明天；负余额也按耗尽。 |
| 6 横幅 / 详情 / 重写 | `lib/automation-view.test.ts:650`：手动进行中的五组既有测试更新名称；`:740`：耗尽文案、按钮逐字核对，禁止今天/明天，缺失 unlimited 仍走每日状态。 |
| 7 确认框 | `lib/automation-view.test.ts:927`：三句完整逐字数组断言，第二、第三句未改。 |
| 8 档位 | `lib/automation-view.test.ts:782`：token 列表、所有档位文案完整断言，名字含多整理些、不含集中补齐。 |
| 9 页面源码守卫 | `pages/v2/toolbox.test.tsx:1` 使用要求的 `?raw` 导入；`:8` 只展开确认/原命令参数；`:20` 滚动及焦点；`:28` scanPanel 来源、纯函数接线、页面没有余额判断；`:45` 默认折叠、按钮及暂停位置；`:61` README 顺序及旧文案禁用。折叠实际文字来自纯函数常量，源码同时有该区块中文注释。 |
| 10 既有测试更新 | 见下方点名清单。既有测试未删除、未跳过、未放宽。新增每日额度数字和入队结果文案测试见 `lib/automation-view.test.ts:773`、`:807`。 |

既有测试期望改动（只因状态对象扩展或要求的文案变化）：

- `lib/automation-view.test.ts:174`「额度是 0（未开启）」：包含不会花钱改为完整新 detail 的逐字断言。
- `:180`「开了但每天额度用完」：今天/明天改每日耗尽、24 小时累计、排队数及现在整理入口，增强为逐字断言。
- `:234`「没配置 AI 服务 > 暂停 > 没开启 > 额度用完 > 整理中 > 还没加入」：耗尽 title 更新。
- `:266`「每种状态都说得清楚」、`:299`「没配置服务 / 暂停 / 额度用完 / 在跑」：明天改过一阵（另有完整新文案断言）。
- `:407`「还没读到状态是 loading」、`:411`「没配置 AI 服务是 unavailable」、`:438`「进行中是 running」：完整状态对象增加 title/description，保留原字段和断言。
- `:669` 手动整理横幅、`:682` 队列收尾横幅、`:699` 详情、`:708` 零队列/零余额、`:720` 重写提示：当前运行名称更新成「现在整理」，进度、费用、优先级断言保留。
- `:741` 耗尽横幅、`:749` 耗尽详情/重写、`:764` unlimited 缺失：逐字更新文案/按钮，详情与重写由包含判断增强为完整对象判断。
- `:928`「确认框固定三句」：只更换第一句期望，后两句原样。
- `:1152`「横幅、详情、重新整理的按钮用统一说法」：两处调整每天额度改去整理。
- `lib/item-format.test.ts:616`「没配置服务 / 暂停 / 一次性全部整理」：手动进行中 sub 改完整新文案断言。
- `lib/item-format.test.ts:632`「开着的时候照原来的提示」：耗尽 sub 改完整新文案断言。
- `components/library/item-detail.test.tsx`、`today-page.test.tsx` 没有依赖旧文案的期望，不必改；全套运行均通过。`item-format.ts` 直接转交纯函数文案，无需修改。

## 改了哪些文件

- `apps/desktop/src/lib/automation-view.ts`：兼容新字段、整理定义及模式、额度提示、分项、数字和排队结果纯函数。
- `apps/desktop/src/lib/automation-view.test.ts`：新增边界和逐字文案测试，更新上述既有期望。
- `apps/desktop/src/lib/item-format.test.ts`：更新两条转交文案期望。
- `apps/desktop/src/pages/v2/toolbox.tsx`：按纯函数绘制、确认滚动与焦点、默认折叠队列操作。
- `apps/desktop/src/pages/v2/toolbox.test.tsx`：新增 5 项源码接线测试（未暂存，普通 stat 不列它）。
- `.card-report.md`：按命令契约 §6 交回（已被忽略）。

## 验证

- `tools/cards/run.sh web-build` → 退出码 0，包类型检查、前端 `tsc --noEmit`、Vite 打包成功（2187 模块）。
- `tools/cards/run.sh web-test` → 退出码 0，34 个测试文件、800 项通过，0 失败，1.26 秒；沙盒内可跑，无端口/被禁目录失败。
- `git diff --check` → 退出码 0，无空白错误。
- `git diff --stat` → 四个已有文件均在白名单，383 行新增、92 行删除；`git status --short` 再确认新增测试也在白名单。
- 全仓搜索使用 `rg -n '明天|额度用完|一次性全部整理|集中补齐' --glob '!.env*' --glob '!pnpm-lock.yaml' --glob '!Cargo.lock'` → 183 个命中行；范围外 29 个文件，下方列全。完整搜索临时结果只放在本次 `$TMPDIR/card40-copy-search.txt`，不依赖该临时文件交回。
- 只读 `git show HEAD:<路径>` 与工作文件文本对照 → 上次整理函数、额度保存/自定义校验、备份及微信后续区块、Tauri invoke 调用行四项一致性均为 True。
- 一次用默认登录 shell 执行 Python here-document 文本编辑失败：`zsh: can't create temp file for here document: operation not permitted`，文件未改；随后改用非登录 shell 的 `python3 -c` 完成文本编辑，未尝试写系统 /tmp。构建、测试及搜索临时文件均使用本次专用 TMPDIR。
- 本卡只改前端，未运行 Rust 构建/格式/测试，也未启动 App、调用真实 AI、访问数据目录或凭据。

全仓搜索范围外残留（未修改）：

| 路径 | 行号 | 性质 |
|---|---|---|
| `apps/desktop/src-tauri/src/card_handler.rs` | 489, 2267, 2273, 2309 | 489 是可见未确认错误「一次性全部整理会花钱，需要你确认之后才能开始。」；其余测试/注释。 |
| `apps/desktop/src-tauri/src/spend_guard.rs` | 459, 464, 1338, 1912, 1918, 1923 | 459/464 可见拒绝提示仍指向开启「一次性全部整理」；其余测试/注释。 |
| `apps/desktop/src-tauri/src/solve_rerank.rs` | 2260, 2265, 2923, 3278, 3515 | 测试夹具里的今天额度用完。 |
| `apps/desktop/src/lib/solve-view.test.ts` | 1074, 1077, 1079 | 原因透传测试，含今天额度用完。 |
| `apps/desktop/src-tauri/src/ai.rs` | 2817 | 输出额度用完的注释，与每日额度不同。 |
| `apps/desktop/src-tauri/src/card_pipeline.rs` | 526, 565, 3680, 3682, 4053 | 输出截断、摘录额度的注释/测试。 |
| `apps/desktop/src-tauri/src/jobs.rs` | 1252 | 注释。 |
| `apps/desktop/src-tauri/src/lib.rs` | 5650 | 注释。 |
| `apps/desktop/src/components/library/item-detail.tsx` | 400 | 状态说明注释，没有旧用户文案。 |
| `tools/cards/dispatch-codex.sh` | 2 | 开发者额度注释。 |
| `docs/rebuild/任务卡/26-full-scan-ui.md` | 27, 94, 96, 97, 132 | 历史任务卡。 |
| `docs/rebuild/任务卡/30-solve-rerank.md` | 32 | 历史任务卡。 |
| `docs/rebuild/任务卡/33-copy-settings.md` | 34 | 历史任务卡。 |
| `docs/rebuild/任务卡/34-rerank-hardening.md` | 72 | 历史任务卡。 |
| `docs/rebuild/任务卡/37-copy-cleanup-rust.md` | 31 | 历史任务卡。 |
| `docs/rebuild/任务卡/39-budget-ledger.md` | 13, 16, 17, 22, 24 | 并行任务卡。 |
| `docs/rebuild/任务卡/40-organize-page-logic.md` | 14, 17, 20, 21, 22, 23, 25, 29, 54, 55, 56, 61, 72, 78, 84, 85, 86, 87, 88, 89, 92, 96, 97, 99, 100 | 本卡原始要求和旧文案背景。 |
| `docs/rebuild/任务卡/README.md` | 26, 36, 49, 50 | 状态清单。 |
| `docs/rebuild/任务卡/报告/26-full-scan-ui.md` | 4, 12, 19, 38 | 历史报告。 |
| `docs/rebuild/任务卡/报告/33-copy-settings.md` | 34, 119 | 历史报告。 |
| `docs/rebuild/任务卡/报告/37-copy-cleanup-rust.md` | 23 | 历史报告。 |
| `docs/rebuild/任务卡/续作/31-copy-solve.md` | 3 | 历史续作单。 |
| `docs/rebuild/任务卡/续作/32-copy-library.md` | 3 | 历史续作单。 |
| `docs/rebuild/任务卡/续作/33-copy-settings.md` | 3 | 历史续作单。 |
| `docs/rebuild/任务卡/续作/37-copy-cleanup-rust.md` | 3 | 历史续作单。 |
| `docs/rebuild/切库清单.md` | 17, 23, 25 | 文档。 |
| `docs/rebuild/命令契约.md` | 10, 13, 168, 175, 196 | 文档。 |
| `docs/rebuild/接手说明.md` | 136, 137, 208, 221, 266, 267, 269, 270, 273, 274, 293, 294 | 文档和历史进度。 |
| `docs/rebuild/文案审读.md` | 31, 78, 148 | 历史文案约定。 |

## 没做到的 / 偏离任务卡的地方

- 没有启动 App 做视觉截图或真实滚动/焦点验收：红线禁止启动，按卡要求采用纯函数测试和源码接线测试，不能把它们写成已实际点过界面。
- 范围外三处 Rust 用户提示的旧名称没有改；已经列出位置，改动需要另卡授权，不影响本卡前端入口及逻辑完成。
- 对任务卡内相互覆盖的文案要求采取保守解释：1.2 明确要求「其余状态文案不动」，因此 `automationState` 的运行中 label `一次性全部整理中`（`automation-view.ts:384`）保持原样；第 2 部分点名的三个函数的运行中文案均已改名。`lastFullScanText` 的旧名称按明示要求保留；代码注释和测试描述也有旧词，并非用户可见文案。
- 旧后台不提供 `awaiting` 时遵循要求回退；若它明确给 `needsOrganizing: 0` 而还有重新排队的收藏，仍不能从该旧字段知道准确去重总数。新字段可正确显示该情况，已有测试；没有擅自改变约定的回退顺序。

## 需要总控或 Fox 决定的事

- 无。本卡范围外旧名称可由总控后续另卡处理。

## 给审核者的提示

- 合并卡 39 时核对五个 camelCase 字段，尤其 `awaiting` 的去重和 `dailyTokensUsed` 排除本轮/历史手动整理用量；本卡只消费字段，并未修改后端账本。
- 在可启动 App 的环境里检查：额度耗尽且有待整理数时按钮可点；点击后确认块滚进视区且确认开始获得焦点；取消不发命令；只有确认开始调用带 `confirmed: true` 的启动命令；默认折叠区及外部暂停按钮；轮询更新不应重复抢焦点。
- `FullScanPanelView` 的 effect 只依赖确认块是否显示，不依赖每次刷新的状态对象。复用现有 Button，通过确认容器查询标记按钮来聚焦，未修改组件库或新增依赖。
- 新增测试文件未暂存，普通 `git diff --stat` 不会显示；提交应按白名单路径处理。工作区起手干净，没有上一位半成品需要恢复。

---

## 总控审核（2026-10-05，Claude）

- **执行者**：Codex（`gpt-6.1-sol`，xhigh），13:57:10 派出、14:11 交回（约 14 分钟，31 条命令）。
- **范围**：`check-scope.sh` 5 个文件全在允许范围内（4 个改动 + 新增 `pages/v2/toolbox.test.tsx`）。
- **日志审计**：31 条命令全是读文件、`git status / diff / show`、`tools/cards/run.sh web-build / web-test`、`rg` 搜索和 `python3 -c` 文本编辑；没有碰正式 / 开发数据目录、钥匙串、密钥，没有 git 写命令，没有联网。报告里写的「登录 shell 的 here-document 建临时文件被拒」是沙盒正常现象，没有影响。
- **diff**：`automation-view.ts`、`toolbox.tsx`、测试全部逐行读过，和任务卡的文案表、逻辑表一致。几处我特别核对过：
  - 面板状态（`loading / unavailable / nothing / ready / running`）只看服务是否配置、是否在整理、待整理个数，**不看每天额度**；页面源码里没有 `budgetRemaining` / `dailyBudgetTokens`（有测试守着）。
  - 第一步 / 之后：有分项就看「新版说明个数是否为 0」，没有分项（旧后台）才退回「有没有上次整理记录」。
  - 待整理个数：`awaiting → needsOrganizing → queued + running`，0 是有效值，负数夹成 0。
  - 「确认开始」：第一次点只展开确认块（不调用命令）；只有确认块里的「确认开始」才调用 `start_full_scan`，并带 `confirmed: true`。确认块出现时滚进可视区、焦点给「确认开始」，effect 只依赖「确认块是否显示」，轮询刷新不会反复抢焦点。`Button` 组件把额外属性透传给 `<button>`（看过源码），所以 `data-confirm-full-scan` 能让焦点真的落上去。
  - 折叠区默认收起（原生 `<details>`，没有 `open`），「暂停」留在折叠区外。
- **「改坏实现」检查**（我自己在草稿目录里写的脚本，不进仓库）：对 `automation-view.ts` 28 种、`toolbox.tsx` 14 种，共 42 种改法，**40 种被测试抓住**。重点几种都抓住了：额度用完就锁住手动整理（AV01）、没开启每天自动整理就锁住手动整理（AV23）、第一步判断反过来（AV02/03）、额度用完文案又写「明天」（AV04/19/20/21）、`awaiting` 与 `needsOrganizing` 顺序对调或去掉回退（AV06/07/08）、确认框 48 小时改 24 小时（AV09）、最大档额度改动（AV10）、额度那一格改回原始 24 小时合计（AV11）、首个按钮跳过确认直接开始（TB05）、确认开始不带 `confirmed: true`（TB04）、折叠区默认展开（TB03）、不滚动 / 不聚焦（TB01/02）。
  - **2 种活下来**：
    1. TB06：把页面上「现有 N 个收藏：新版说明 · 旧版说明 · 还没有说明」那一行整行删掉，测试没发现——页面接线只守了「有 `countsLine` 这个变量」，没守「它被画出来」。这一行正是 Fox 被「500 个没有中文说明」搞糊涂的那个点，必须守住 → **放进卡 41**。
    2. TB14：我设计得不好的改法（给第一个按钮加一个和额度无关的 `disabled={props.starting}`），不算漏洞。真正的「页面自己判断额度锁按钮」会撞上「页面源码里不许出现 `budgetRemaining`」的守卫。
- **没做到 / 遗留**（都不影响本卡合并，统一放进后续小文案卡 41）：
  1. 状态徽章 `automationState` 在整理进行中仍叫「一次性全部整理中」（我在任务卡里写了「其余状态文案不动」，Codex 照做）。
  2. 「正在整理」面板的说明仍是旧话「第一次用、想一口气把全部收藏整理完，就用它」（任务卡写的「照旧」）——整理已经在进行时再说「就用它」不通顺。
  3. Rust 里三处用户可见提示仍叫「一次性全部整理」：`card_handler.rs:489`（未确认就开始的报错）、`spend_guard.rs:459/464`（额度拒绝提示）。
  4. 实际界面（滚动、焦点、折叠）红线不让 Codex 启动 App，没点过；我在合并、发布后用真数据复验。
- **合并后要做的**：和卡 39（后端记账，派给 Codex、同时进行）一起发布，不单发——单独发布时旧后端没有新字段，页面会退回旧逻辑（「现在就整理 500 个」、没有分项那一行）。
