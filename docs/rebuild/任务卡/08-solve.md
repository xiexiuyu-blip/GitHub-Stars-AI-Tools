---
编号: 08-solve
允许改动:
  - apps/desktop/src-tauri/src/solve.rs
  - apps/desktop/src-tauri/src/lib.rs
  - apps/desktop/src-tauri/permissions/gsat-commands.toml
  - apps/desktop/src-tauri/gen/schemas/acl-manifests.json
---

# 卡 08 · 找方案：按「意图」从收藏里找能用的（后端）

## Fox 能看到的效果
这是后台积木，**这张卡 Fox 暂时看不到界面变化**。做完后，「找方案」页输入一句话（比如「做小红书封面」），后端能把它拆成**意图**（小红书、封面），从 Fox 的收藏里找出能用的项目：哪些**所有意图都对上了**、哪些只对上一部分、哪些意图在收藏里根本没有（缺口），并给每条一句「为什么相关」。**只默认勾选「所有意图都对上了」的前 3 条，没有就一个都不勾，不硬凑。** 这就是 Fox 说的「从提问到拿到可直接粘给 Codex 的参考包」里的前半段。

## 背景与先读什么
- **契约（不许改名、不许改形状）**：`apps/desktop/src/lib/contracts.ts` 里的 `SolveHit`、`SolveResult`。命令 `solve_v2(query)` 必须逐字段按它返回 JSON（serde 的 `camelCase`）。
- **算法依据：原型里已经验证过的打分逻辑**——`docs/rebuild/原型/template.html` 约第 376–410 行（`STOP`、`tokens`、`idf`、`LEX`、`concepts`、`fieldScore`、`search`、`why`）和约第 611–640 行（`runSolve`、`pageSolve` 里关于「缺口」「勾选」的部分）。**把它移植成 Rust，行为保持一致**，只在下面「和原型的不同」里列出的地方有区别。原型在 495 个仓库上试过：「做小红书封面」「给公众号文章配图」「短视频加字幕」「做 PPT 和文档」「做电商详情页图片」这几句拆词和命中都是合理的。
- 现有的粗糙实现（**不要改**）：`v2_ops.rs` 里的 `solve_problem` 和 `lib.rs` 里的 `solve_problem` 命令——它只做关键词 LIKE，旧页面在用。
- **表**：`repositories`（有效 = `sync_status='active'`）；`repo_ai_documents`（`one_liner_zh`、`summary_zh`、`keywords_json`、`card_json`）；`scenarios` + `entity_scenarios`（迁移 030，**只读**；`entity_scenarios.status<>'rejected'` 才算归类；测试里直接用 SQL 插行）。`card_json`（格式见 `docs/rebuild/spec/ai_card.schema.json`）里取 `capabilities[].name_zh`、`problems_solved`、`form`；它**可能是空串、坏 JSON、字段类型不对，必须容忍**，取不出就当没有，不能让整个查询失败。
- 测试怎么造库：看 `db/migrations.rs` 里 `empty_database_applies_every_migration`，再用 SQL 插 `github_accounts`、`repositories`、`repo_ai_documents`、`scenarios`、`entity_scenarios`。**所有测试只用临时库。**

## 要做的事

### 新模块 `src-tauri/src/solve.rs`（不依赖 Tauri）
1. **意图词表 `lexicon(scenarios) -> Vec<String>`**：基础词表照抄原型的 `LEX`（小红书、公众号、抖音、视频号、微信、短视频、封面、海报、配图、插图、文案、字幕、详情页、主图、标题、选题、脚本、排版、剪辑、配音、口播、直播、私信、评论、涨粉、投放、笔记、爆款、竞品、关键词、发布、采集、下载、翻译、总结、链接、群聊、文档、表格、报告、简历、电商、淘宝、亚马逊、seo、ppt、excel、word、pdf、图片、视频、文章、知识库、爬虫、自动化、数据分析、品牌、营销、广告、邮件、写作、设计），再加上：每个未归档场景的名称按「·」拆开后的各段（「公众号·微信」→「公众号」「微信」）和场景关键词里长度不少于 2 的词；转小写、去重、**按长度从长到短排序**。
2. **拆意图 `concepts(query, lexicon) -> (main, extra)`**：和原型一致——小写后，按词表从长到短逐个检查，出现就收进 `main` 并把它从剩余文字里整段替换成空格；剩下的文字用 `tokens` 切成 `extra`，只保留长度不少于 2 个字符的。`tokens`：按空白和 `, ，。;；、/` 切开；含汉字的片段：只取其中的汉字，汉字数不超过 2 就整体当一个词，否则切成相邻两字的组合（二元组）；片段里的非汉字部分（连续的英文数字）长度大于 1 也算一个词；不含汉字的片段长度大于 1 就是一个词；去重；去掉停用词（`我想`、`想做`、`想要`、`怎么`、`如何`、`可以`、`一个`、`工具`、`帮我`、`需要`、`里的`、`做个`、`怎样`、`什么`、`有没有`、`能不能`、`一下`、`这个`、`那个`、`的工`、`具链`）。
3. **语料与打分**：把每个有效仓库的文字分成几块（全部小写）：名称（`full_name`）、一句话（`one_liner_zh`）、能力（`capabilities` 名称拼接）、问题（`problems_solved` 拼接）、关键词（`keywords_json` 拼接）、场景名（它所属场景的名称拼接）、摘要（`summary_zh`）、描述（`description`）。`idf(词) = ln(1 + N / (1 + df))`，`N` 是有效仓库总数，`df` 是上述任一块里包含该词的仓库数（同一个词的 idf 在一次查询里只算一次）。`field_score(仓库, 词) = 取下面各项里的最大值`：名称命中 `3×idf`；一句话命中 `2.5×idf`；关键词命中 `2.5×idf`；能力或问题命中 `2×idf`；场景名命中 `2×idf`；摘要命中 `1.5×idf`；描述命中 `1×idf`；都不命中为 0。`search`：对每个仓库，`main` 里每个意图词算 `field_score`，大于 0 就记一次命中并累加得分，否则记入 `missing`；`extra` 里每个词给 `0.3×field_score` 累加；命中的意图词超过 1 个时，总分乘以 `1 + 0.6×(命中数-1)`；总分大于 0 才入选。排序：得分从高到低，并列按星数从高到低，再按 `id` 升序（保证稳定）。
4. **结果 `solve_query(db_path, query) -> Result<SolveResult, String>`**：
   - 查询为空（去掉空白后没有任何内容）返回中文错误「先写一句你想做的事」。拆不出任何意图词也没有任何词（`main` 和 `extra` 都空）：返回空结果（`hits` 空，`intents`/`extras` 空），不报错。
   - **`hits` 只含「已有理解卡」的仓库**（`one_liner_zh` 非空），最多 8 条；字段按契约：`matched` 是这一条对上的意图词，`missing` 是没对上的，`fullMatch` = `main` 非空时「所有意图词都命中」，`main` 为空时恒为 `true`；`why` 取下面这个顺序里**第一个包含任一意图词或零散词**的文字：每条 `problems_solved`、每条能力名、一句话；都不含就取第一条问题、第一条能力、一句话里第一个非空的，最多 80 个字符（超出加「…」）；`scenarioIds` 按场景 `sort_order` 排。
   - `preselected`：`fullMatch` 的前 3 条的 id；没有就是空数组。`fullMatchCount`：`hits` 里 `fullMatch` 的个数。`gapIntents`：`main` 里**没有被任何一条 `hits` 命中**的意图词。
   - `suggestedScenarioId`：第一个（按 `sort_order`）满足「场景名或场景关键词里有一个词，包含了某个意图词或零散词」的未归档场景；没有就是 `null`。`suggestedScenarioCount`：该场景下有效仓库数（归类 `status<>'rejected'`），没有场景时是 `null`。
   - `pendingMatches`：**没有理解卡**、但名称或描述里就包含了**全部意图词**的有效仓库数（`main` 为空时为 0）。
   - 和原型的不同（只有这些）：①`hits` 只含有卡的仓库，名称与描述命中但没卡的只计入 `pendingMatches`；②新增「关键词」块，权重 `2.5×idf`；③意图词表按场景动态扩展；④并列时多了 `id` 升序；⑤`why` 的取值更完整。

### 接入（`lib.rs`）
- 登记 `mod solve;`，新增命令 `solve_v2(query: String) -> SolveResult` 并登记到 `generate_handler!` 和 `permissions/gsat-commands.toml`。数据库路径一律从 `AppStorage` 取；只读命令用只读方式打开。**旧的 `solve_problem` 保持不动。**

## 不要做
- 不改迁移、不改 `v2_ops.rs`、不改前端、不新增依赖；不写任何表。
- 不引入向量或联网（以后另有卡把上游的向量检索混进来）；不调用 AI。
- 不要读写真实的数据目录；测试只用临时库。

## 验收
1. `tools/cards/run.sh rust-test` 通过，`solve.rs` 新增测试**至少 22 个**，覆盖：`tokens`（含中英混排、两字以内的汉字片段、长汉字片段切二元组、停用词被去掉、去重）；`concepts`（「做小红书封面」→ main 为 小红书、封面；「短视频加字幕」→ 短视频、字幕，且「视频」不会再被重复当意图；最长优先；零散词的长度限制）；词表按场景动态扩展且去重、按长度排序；`idf` 的数值（用一个小语料手算对照）；`field_score` 各块的权重和取最大值；多意图命中的加成公式；`extra` 的 0.3 权重；排序的并列规则；**用一个 12 个仓库的夹具库跑通原型里的 5 句例子**，断言「做小红书封面」有全部对上的结果、「做电商详情页图片」在夹具里没有任何全部对上的（`fullMatchCount` 为 0 且 `gapIntents` 非空）、`preselected` 在没有完全命中时为空；`hits` 上限 8；没有卡的仓库不进 `hits` 但计入 `pendingMatches`；`why` 的取值顺序和 80 字截断；`suggestedScenarioId` 与计数；空查询报错、全是停用词返回空结果；**坏 JSON、空串、字段类型不对的 `card_json` 不会让查询失败**；已移除（`sync_status<>'active'`）的仓库不参与。
2. **契约测试**：把 `SolveHit`、`SolveResult` 各序列化一份，断言键集合和类型与 `contracts.ts` 一致（含 `pendingMatches`、`suggestedScenarioId` 为 `null` 的情形）。
3. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多（4 条）。
4. 给出 `solve_v2` 在 `generate_handler!` 和 `gsat-commands.toml` 里的行号；证明 `v2_ops.rs` 没有改动。
5. 性能：在测试里造 3000 个仓库的临时库，一次 `solve_query` 在本机应当不超过 1 秒；把实测耗时写进报告。超过就优化（比如预先把文字块转小写一次、idf 缓存）。
6. 在报告里贴出夹具库上这 5 句话的完整结果摘要（意图、前三条命中的名称与得分、`fullMatchCount`、`gapIntents`、`preselected`），方便总控人工核对合不合理。
7. `git diff --stat` 里只出现「允许改动」里的路径。

## 停止条件
- 要改迁移或卡外文件、要新依赖、原型逻辑与本卡描述有冲突——停手并写进报告，不要自己拍板改契约。
