# 交回报告 · 卡 33-copy-settings（续作）

## 一句话结论
续作「必须做」1–10 全部做完（第 10 条：命令面板里本来就没有「地图」入口，没有可改的）；`web-build`、`web-test`（508 项）、`rust-fmt`、`rust-check`（6 条警告，和起点一样）、`rust-test`（1146 项）全部通过。有 6 件事要总控定，见「需要总控或 Fox 决定的事」。

## 验收逐条
| 验收项（照抄任务卡 / 续作） | 做到了吗 | 证据 |
|---|---|---|
| 卡验收 1：前端新增至少 16 个测试 | 是 | `web-test` 从我接手时的 392 项变成 508 项（本轮净增 116 项）。同步状态文字 `automation-view.test.ts:685`；确认框三句 `:724`；横幅文字 `:741-:900`；导出提示 `:902`；`describeInvokeError` 几种报错 `api.test.ts:186`；各状态按钮文案 `:932`；AI 未配置提示 `ai-config.test.ts:119,127`；备份时间行 / 类型标签 `:910,:923` |
| 卡验收 2：Rust 新增至少 4 个测试（300 个 / 回传数量 / 空库 / CSV 转义） | 是 | `v2_ops.rs:1870`（300 个）、`:1900`（数量回传）、`:1923`（空库）、`:1956`（引号 / 换行）；本轮另加 6 个：`:2015,2038,2055,2073,2091,2110`；`lib.rs:7697` 1 个 |
| 卡验收 3：设置页里不再出现旧叫法（给出 `grep` 输出） | 是（见下一节） | `grep` 输出见「验收第 3 条的 grep 输出」；另有 `automation-view.test.ts:1156-:1254` 的守卫测试，旧词回来会变红 |
| 续作验收：5 项检查全过、警告数不增、`git diff --stat` 只出现允许改动里的文件 | 是 | 见「验证」；`git status` 里只有卡里「允许改动」的 22 个文件（`.tmp-card/` 已删） |
| 续作验收：1–10 各有对应测试或说明 | 是 | 见下一节「条号 → 文件:行」 |

## 条号 → 文件:行（续作 1–10，括号里是审读文件的条号）

**续作 1 · `pages/settings.tsx`（7、11、12、15、22、24、31、42、43、48）**
- 15 / 42 页签：`settings.tsx:193-202`（GitHub / AI 设置 / 通用 / GitHub 备份 / 关于，「向量」页签不再渲染，面板函数 `:1506-:2180` 和后台命令都留着）；`:212` 5 个页签改 5 列；「工具箱」页签改名「自动整理与备份」：`App.tsx:389`（`lib/nav.ts` 不在本卡范围，在 App 里覆盖）
- 7：运行检查清单只剩 4 行、删了「标签网络」那一行和「生成标签网络」按钮：`settings.tsx:635-:790`、`lib/automation-view.ts:654`（`buildReadinessRows`）；「自动生成 AI 摘要」换成指路：`settings.tsx:1292`、`lib/ai-config.ts:414`
- 31：「运行检查」，说明「看看 GitHub 连接、同步、AI 是不是都正常」，按钮「开始检查」，各行读真实数据（「已同步：资料库里有 497 个收藏」「AI 已设置：{模型名}」）：`settings.tsx:688-:707`、`automation-view.ts:654-:710`；检查结果各项名字：`automation-view.ts:718`
- 22：推荐语删掉；服务商副标「需要：密钥 + 模型名」：`ai-config.ts:266-:400`；「现在：AI 已关闭」`settings.tsx:1180` + `ai-config.ts:405`；协议名折进「高级」：`settings.tsx:1378-:1419`、`ai-config.ts:417`；「测试连接」旁「会真实调用一次 AI，会产生很少的费用」：`settings.tsx:1422-:1424`、`ai-config.ts:411`；「首字响应」→「开始回复用时」：`settings.tsx:2358`
- 24：令牌 / 密钥 / 钥匙串：`settings.tsx:348-355,404,417,493`、`ai-config.ts:424-:455`、`hooks/use-settings.ts:15-16,82,94,144,159,188,200,245,275,354`（8 处重复的句子改成同一个函数）、`automation-view.ts:731-:739`
- 11：「上传备份到 GitHub（私密 Gist）」，「备份我的标记 / 备份整个清单」，导入区常显警告：`settings.tsx:2897-:2990`（警告 `:2971` 不放在任何条件后面）、`automation-view.ts:760-:780`
- 12：清空本机数据说明和确认框：`settings.tsx:2659,2702`、`automation-view.ts:754-:756`
- 43：关于页只留版本号 / 数据放在哪个文件夹 / 许可证，其余折进「开发者信息」：`settings.tsx:2734-:2890`；更新失败见续作 4
- 48：`settings.tsx:2609,2620,2632`、`automation-view.ts:744-:752`

**续作 2 · `components/welcome-flow.tsx`（24、30）**：`welcome-flow.tsx:66,90-105,143-147,171-181,220-256,270,333-340,389-394,448`；文案集中在 `automation-view.ts:782`（`WELCOME_COPY`）和 `:731-:741`

**续作 3 · 「约整理了 N 个」读真实个数（6、39）**
- 接 `get_ai_queue_status`：`lib/api.ts:214`（`getAiQueueStatus`）、`:222`（`getAiAutomationWithRecent`，队列状态读不到就带 null，状态照常用）；合并和文案 `automation-view.ts:526-:575`
- 今日 / 资料库横幅：`hooks/use-ai-automation.ts:18`；工具箱：`pages/v2/toolbox.tsx:277,287,295,565`
- 读不到时整句去掉（只写「已用约 X token」，不估）：`automation-view.ts:556-:565`
- 一次性全部整理的横幅里「这一轮已整理 N 个」读 `fullScan` 的开始个数 / 剩余个数 / 失败个数：`automation-view.ts:176,367`
- 测试：`automation-view.test.ts:741-:900`、`api.test.ts:241-:311`

**续作 4 · 更新报错按原因分开（43）**：`lib/api.ts:290-:323`（网络 / 更新文件没找到 / 校验没通过 / 没权限写 / 认不出原因 各一句，写清下一步；「安装后重启失败」是另一句）；`providers/app-update-provider.tsx:166,233,262,269`；`components/app-update-panel.tsx:227,248-251`；测试 `api.test.ts:313-:440`。注意顺序：先签名、再网络、最后才看「文件没找到」，因为断网的报错里常带着 `latest.json` 的网址（原来的写法就是这样把断网说成「文件没找到」的）。

**续作 5 · 「之前一次性整理用的」**：`automation-view.ts:541`（`spentByEarlierFullScan`）。不再看 `!enabled`（暂停时它也是 false）；现在要同时满足：没有一次性整理在跑（`unlimited`）、每天额度是 0（`dailyBudgetTokens`）、上一轮一次性整理（`lastFullScan.finishedAt`）在 24 小时内结束、24 小时内确实花过钱。测试 `automation-view.test.ts:767-:876`，含「暂停 + 没有一次性整理 → 不能出现这句」。

**续作 6 · `updated_count`（40）**：`automation-view.ts:491`（`formatSyncComplete` 不再写「有 N 个的信息更新了」）、`lib.rs:4216,4311`（Rust 那条消息抽成 `sync_complete_message`，同样只写现在共多少 / 新增 / 取消收藏）、`settings.tsx:626`、`features/sidebar/sync-panel.tsx:60`；测试 `automation-view.test.ts:708`、`lib.rs:7697`。前端 `grep -rn updatedCount` 现在只剩类型定义和注释。

**续作 7 · 导出（8）**：`v2_ops.rs:969`（Markdown）、`:1045`（CSV）、`:1104,1111`（写文件的函数）。做法见下一节。

**续作 8 · 残留旧词和写法**：`App.tsx:304,664,699-717`；`toolbox.tsx:595`（「当前的库」→「当前的资料库」）；`automation-view.ts:109`（`12000` → `1.2 万`）、`:226-:235`（`9月1日` → `9 月 1 日`）。顺手一并改了：`app-layout.tsx:411`（「同步 Stars中」式的转圈文字）、`use-stars-workspace.ts:396,402,413,1161`、`use-settings.ts` 里拼英文原文的几句、`app-update-panel.tsx` 里 `toLocaleString()` 的日期。

**续作 9 · 工具箱**：`toolbox.tsx:619`（去重：条目副标「默认关闭，需要时再开启。」，状态行保留「开启后，只有这台电脑上的程序能连」）；出错个数：状态里有真实的「还没解决的失败数」（`AutomationStatus.failed`，后台 `count_unresolved_failures`），所以改用它：`toolbox.tsx:671`、`automation-view.ts:582`（读不到时写「还没读到运行状态」，不写「运行正常」）。另外「复制令牌」改「复制」、两条 toast 改说「微信工作台的口令」：`toolbox.tsx:99,636`。

**续作 10 · 命令面板「地图」**：`components/command-palette.tsx` 里没有任何「地图」字样的入口，没有可改的。`lib/nav.ts:35` 的页签标签还是「地图」（`nav.ts` 不是我的），所以我在 `App.tsx:391` 同样用覆盖的办法把这个页签显示成「按类型看」；`key` / `id` 没动。总控觉得多余可以直接删这一支。

## 第 7 条（导出）的做法
- 去掉了 `create_dir_all`。目标文件夹不存在就返回「导出失败：目标文件夹不存在，请先建好它再导出。」，不替用户建目录（测试 `v2_ops.rs:2073`）。
- 流程：先 `prepare` 好语句（语句有问题就不碰任何文件）→ 在目标文件**同一个文件夹**里写一个隐藏的临时文件（`.<文件名>.<进程号>-<纳秒>.exporting`）→ 逐行读库、逐行写（`BufWriter`，内存不随条数涨）→ `flush` + `sync_all` → `rename` 成目标文件。任何一步出错（读库、写盘、改名）都会删掉临时文件、不动原文件（`write_export_file`，`v2_ops.rs:1111`）。
- 测试：写到一半出错 → 目标文件原样、目录里没有临时文件（`:2015`）；第一次导出就失败 → 不会凭空留下半截文件（`:2038`）；成功 → 内容对、行数对、目录里只有目标文件（`:2055`）；库读不出来（缺表）→ 旧文件原样（`:2091`）；缺文件夹（`:2073`）。
- 「没有 AI 文档行时 Markdown 退用 GitHub 简介」：**保留**。顺序是：中文一句话 → 中文摘要（`summary_zh`）→ GitHub 简介 → 空。原来的 `list_workspace` 路径是「一句话 → 摘要」，上一位重写查询时把摘要这一档丢了（CSV 里整格变空），我补回来了，所以现在是原来行为的超集；CSV 仍然不退用 GitHub 简介（原来也没有）。空串和纯空白按「没有」处理（`NULLIF(trim(..), '')`）。测试 `v2_ops.rs:2110`。

## 验收第 3 条的 grep 输出
命令：`grep -n "标签网络\|增量 AI 解析\|向量\|GSAT\|系统凭据管理器\|Personal Access Token" apps/desktop/src/pages/settings.tsx`

```
194:  // 「向量」页签先藏起来（审读第 15 条）：新界面里没有任何页面用到向量检索，点「下载并启用」却要下载约 490 MB。
1642:        text: `Embedding 服务可用：${result.model} 返回 ${result.dimensions} 维向量。`,
1659:        setMessage({ type: 'success', text: '向量检索已关闭，当前使用关键词检索。' });
1711:      : { type: 'success', text: '本地向量检索已就绪。' });
1754:          ? `向量索引已处理 ${result.totalCount} 个仓库，成功 ${succeeded} 个，失败 ${result.failedCount} 个。…`
1755:          : `向量索引已就绪：新生成 ${result.indexedCount} 个，从 SQLite 恢复 ${result.restoredCount} 个。`,
1808:          <h3 …>向量检索</h3>
1810:            默认使用本地模型生成中英双语向量，由本机 zvec 完成语义召回；远程服务仅在高级设置中启用。
1831:            <p …>向量维度</p>
1937:            向量维度
1981:                {status?.message ?? (embedding.enabled ? '可检查本机索引状态。' : '启用后可生成并使用语义向量。')}
2081:        aria-label="本地向量模型准备进度"
2138:              下载本地向量模型
```
解释：第 194 行是注释；第 1642–2138 行全部在 `EmbeddingSettingsPanel` / `EmbeddingSetupProgress` / `LocalEmbeddingDownloadDialog`（`:1506-:2180`）里——这是第 15 条要求「藏起来、代码留着」的那个面板，没有任何页签能打开它。「标签网络」「增量 AI 解析」「GSAT」「系统凭据管理器」「Personal Access Token」在这个文件里一处都没有了（隐藏面板里的「系统凭据管理器」也顺手改成了「钥匙串」）。其余被改的文件（welcome-flow、app-layout、toolbox、App、sidebar、use-settings、use-stars-workspace、app-update-*、command-palette）对前五个词的 grep 是空的。`automation-view.test.ts:1156` 的守卫测试会读这个文件的源码（去掉注释、去掉隐藏面板），旧词回来就变红。

## 改了哪些文件
- `apps/desktop/src/pages/settings.tsx`：条 7、11、12、15、22、24、31、42、43、48；服务商卡片数据搬走，页面改读 `lib/ai-config.ts`
- `apps/desktop/src/components/welcome-flow.tsx`：条 24、30
- `apps/desktop/src/App.tsx`：残留旧词、页签覆盖（工具箱 → 自动整理与备份、地图 → 按类型看）
- `apps/desktop/src/pages/v2/toolbox.tsx`：真实个数、去重、出错个数、口令措辞
- `apps/desktop/src/providers/app-update-provider.tsx`、`components/app-update-panel.tsx`：更新报错按原因分开、日期写法
- `apps/desktop/src/lib/api.ts`：`getAiQueueStatus`、`getAiAutomationWithRecent`、更新报错的分类和文案、更新状态那一句
- `apps/desktop/src/lib/automation-view.ts`：真实个数、`spentByEarlierFullScan`、运行检查清单、设置 / 欢迎引导文案
- `apps/desktop/src/lib/ai-config.ts`：服务商卡片（含文案）、密钥 / 钥匙串文案、AI 设置里的几条常量
- `apps/desktop/src/hooks/use-ai-automation.ts`、`use-settings.ts`、`use-stars-workspace.ts`、`components/app-layout.tsx`：上面提到的几处
- `apps/desktop/src/features/sidebar/connection-panel.tsx`、`sync-panel.tsx`、`system-panel.tsx`：审读第 24 条点名了 `connection-panel.tsx:64`；这三个文件现在没有任何地方引用（死代码），一并改了词
- `apps/desktop/src-tauri/src/v2_ops.rs`：导出写法、6 个测试
- `apps/desktop/src-tauri/src/lib.rs`：同步完成那一句抽成函数 + 1 个测试
- 三个测试文件：`api.test.ts`、`ai-config.test.ts`、`automation-view.test.ts`

## 验证
- `tools/cards/run.sh web-build` → 通过（tsc + vite，无警告）
- `tools/cards/run.sh web-test` → 16 个文件，508 项通过（接手时 392 项）
- `tools/cards/run.sh rust-fmt` → 通过（无差异）
- `tools/cards/run.sh rust-check` → 通过，6 条警告（没读的字段、`README_RULE_CHARS`、`reserve_at`、`ensure_ready`、`repair_model_json`、`restore_database_file`），和接手时逐条相同，没有新增
- `tools/cards/run.sh rust-test` → 1146 通过、0 失败、5 个 `#[ignore]`、1 个钥匙串用例被脚本跳过（5 个 `#[ignore]` 是原来就有的，我没加）。接手时是 1139，多出来的 7 个是我加的（`v2_ops.rs` 6 个 + `lib.rs` 1 个）
- 反向验证过一次：往 `settings.tsx` 临时塞进「标签网络 / Personal Access Token / 向量」，守卫测试 3 项变红；还原后全绿。
- 没有在真实 App 里点过（红线不许启动 App）：界面改动只靠 tsc / vite build、测试和源码守卫验证。

## 没做到的 / 偏离任务卡的地方
1. **「自动生成 AI 摘要」开关没了**（审读第 7 条要求换成指路文字，我照做）。`ai.enableAutoSummary` 默认是 `false`（`types-settings.ts:87`），但旧页面（`repositories.tsx`）、设置 → GitHub 的「下载项目说明」按钮和 `App.tsx:294,588` 的欢迎引导还会读它。如果 Fox 的设置文件里它是 `true`，现在界面上没有开关可以关它了（读不了他的设置，无法确认）。
2. **「运行检查」会真的调用 AI**：后台 `check_runtime_readiness_worker`（`lib.rs` 约 3450 行起）里有最多 3 次探测（AI 连接、相似推荐、标签网络），后两项对应的功能页面已经看不到了。我只在前端加了一句费用提示（`automation-view.ts:715`）、把结果里两项改了名字（「AI 打标签」「AI 找相似工具」）。后台这三项探测、以及它们返回的英文 / 旧词消息（「GitHub Stars API 可用…」「AI 标签网络未检测」「真实请求：GET /user/starred…」）我**没动**：`lib.rs` 里有现成测试逐字断言这些句子，卡里也没点名。
3. **欢迎引导第一页三张小卡片的文字是我写的**（审读第 30 条只给了标题、说明、三条提示、慢提示、令牌步骤）：`automation-view.ts:782` 的 `WELCOME_COPY.features`——「本地优先 / 你的收藏只保存在这台电脑上」「快速查找 / 搜名字、说明或关键词，也能按场景筛选」「中文说明 / 需要时让 AI 写，按服务商价格收费，默认不开」。觉得不妥直接改这三行。
4. **「含失败重试」这个括号我没有照审读原句**：审读写的是「最近 24 小时整理了 N 个（含重试）」。`succeeded_24h` 数的是成功的任务条数，重试是同一条任务再跑，不会多出一条；真正「含重试」的是 token。所以我写成「整理了 N 个（已用约 X token，含失败重试）」，把「含失败重试」挂在 token 上。
5. 一次手滑：在最后清理时把一条 `web-build` 的输出重定向到了 `/tmp/_unused`（工作区外），发现后立刻删了，没有别的影响；其余临时文件都在 `.tmp-card/`，已整个删除。

## 需要总控或 Fox 决定的事
1. 上面第 1 点：要不要在别处把 `ai.enableAutoSummary` 强制归零（动设置迁移，不在本卡）。
2. 上面第 2 点：「运行检查」要不要去掉后台那两项旧功能的 AI 探测（省钱、也让结果和新界面对得上；需要改 `lib.rs` 和它的测试）。
3. 后台还有提示在说旧词，**都不在本卡允许改动里**：`card_handler.rs:494`（「设置 → 引擎」，`start_full_scan` 没配置 AI 时的报错）、`local_api.rs:490`（「请在工具箱里重新开启本地 API」）、`spend_guard.rs:424,432,437`（「工具箱」「全量整理（不限额）」「AI 额度」；卡里明令不碰 `spend_guard.rs`）、`lib.rs` 运行检查那批消息。建议单开一张小卡统一改。
4. CSV 没有 UTF-8 BOM。按钮上写着「可用 Excel 打开」；Excel 双击打开没有 BOM 的中文 CSV，在 Windows 上通常会乱码，Mac 上不一定。加 BOM 会让文件第一行多 3 个字节、也会影响给别的脚本读（Python 要用 `utf-8-sig`），所以我没加。要不要加，请定。
5. `lib/nav.ts` 没人认领：`:41` 的「工具箱」和 `:35` 的「地图」页签标签不在卡 31 / 32 / 33 任何一张的允许列表里（卡 32 的工作区里 `nav.ts:35` 仍是「地图」）。我在 `App.tsx` 里覆盖显示，合并后总控可以改 `nav.ts` 并删掉 `App.tsx` 那一段。
6. 审读第 47 条「任务条」那一半（`app-layout.tsx` 的「阶段：…」「已完成 N/M 批」）不在卡 31 / 32 / 33 的条号里（卡 32 只列了 47 但没有 `app-layout.tsx`），我没做。另外卡 32 的 `components/library/detail-panels.tsx:263` 还写着「设置 → 工具箱」「抓取 README」，合并时请核对。

## 给审核者的提示
- **新文件不让建，所以新的纯函数都放进了卡里允许的三个 lib 文件**：`automation-view.ts`（设置 / 欢迎引导 / 横幅 / 运行检查）、`ai-config.ts`（AI 设置、密钥、钥匙串）、`api.ts`（更新报错）。`automation-view.ts` 因此变成了文案集中地（接手时 528 行，现在 815 行），以后可以拆。
- **守卫测试读源码**：`automation-view.test.ts:1156-:1254` 用 `node:fs` 读 `settings.tsx`、`welcome-flow.tsx` 等，去掉注释后检查旧词。TypeScript 6 不再自动带上 `@types/node`，所以文件第一行加了 `/// <reference types="node" />`。这类测试不会拦住「文字写对了但结构错了」，所以关键行为（真实个数、「之前一次性整理用的」、更新报错顺序、导出写法）都另有行为测试。
- **我改过的既有断言（都是换成新文案，没有删、没有放宽）**：`parseDailyBudget` 的「至少要 12000 token」→「至少要 1.2 万 token」；`readmeCheckText` 的「9月1日」→「9 月 1 日」；`formatSyncComplete` 的整句（去掉「更新了 N 个」）；一次性全部整理两条横幅的 `detail`（加上「这一轮已整理 N 个」）；上一位写的 `recentUsageLine` 三个测试——它们断言的正是被总控指出判断错了的 `!enabled` 规则（`{ enabled: false, tokens24h: 3_083_000 }` 就出现那句话），我把它们换成按真实字段判断的一组，原来的三句话（含「（这是之前一次性整理用的）」那一句）在有证据时仍然逐字断言。
- `lib.rs` 里 `updated_count` 这个字段本身没动（后端字段不许改），只是不再把它写进任何一句话里。
- 导出 Markdown 的行序和原来一样（`starred_at DESC`），没有加二级排序键。
- 欢迎引导 / 设置里的「工作台」「进入工作台」没改（不在统一用词表里）。
- 建议合并后在 dev 版里手点一遍：设置 5 个页签（尤其 AI 设置里「高级」展开、选「自定义服务」时它自动展开）、关于页的「开发者信息」、GitHub 备份页导入区的常显警告、欢迎引导三步、工具箱「24 小时内」那一格、导出到「下载」文件夹后看提示里的个数。
