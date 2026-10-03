---
编号: 15-card-sanitize
允许改动:
  - apps/desktop/src-tauri/src/card_view.rs
  - apps/desktop/src-tauri/src/library_view.rs
  - apps/desktop/src-tauri/src/solve.rs
  - apps/desktop/src-tauri/src/packs.rs
  - apps/desktop/src-tauri/src/lib.rs
---

# 卡 15 · 旧版占位理解卡：读取侧统一清洗，找方案别在没有意图词时硬凑

## 为什么有这张卡（总控用 Fox 开发库的真实副本跑出来的问题）
Fox 的开发库里已经有 178 张「理解卡」，其中 92 张（另外 86 张是真 AI 生成的旧版卡，有真内容）**不是 AI 细读出来的**，而是旧函数 `v2_ops::import_summary_cards`（`v2_ops.rs` 约第 510–545 行，**冻结，不许改**）把旧版摘要硬转成的**占位卡**：
- `one_liner_zh` = 摘要的前 60 个字，常常**断在半句**（如「……包含100+ AI Agent和RAG应用。每个」）；
- `capabilities` 固定是 `[{"name_zh":"查看项目说明"}]`；`problems_solved` 固定是 `["想知道这个收藏是做什么的"]`；`how_to_try` 固定是 `"打开仓库 README"`；`confidence` 是 0.4，`card_schema_version` 是 1；
- `what_it_is` 是摘要的前 400 个字，内容是真的，是一段像样的中文介绍。

结果：资料库列表第一行是半句话，「适合：想知道这个收藏是做什么的」，找方案的「为什么相关」也是这句废话，**「复制给 Codex」粘出去的文字里有「可借鉴的能力：查看项目说明」「怎么试：打开仓库 README」**。Fox 一眼就会觉得「AI 整理」是假的。
另外：找方案输入一句没有任何已知任务词的话（如「xyzzy不存在的东西」），现在会返回 6 条「全部对上」且默认勾选 3 条——因为没有意图词时 `full_match` 被写成了真，只靠两个字的碎片（「存在」）匹配。

这张卡的目标：**在读取这一侧**统一把占位卡清洗干净（不改库里的数据、不改旧函数），并让找方案在没有意图词时诚实地少给、不默认勾选。

## Fox 能看到的效果
- 资料库和今日的列表里，旧摘要转出来的项目也是一句**完整、像样的中文**（取摘要的第一句），没有「适合：想知道这个收藏是做什么的」。
- 详情页「能做什么」「怎么试」对占位卡是空的（界面会自己不显示空块），不再出现「查看项目说明」「打开仓库 README」。
- 「复制给 Codex」和参考包里没有这些废话，「它是什么」是完整的一句。
- 找方案里「为什么相关」不再是废话；乱输入一句没有任务词的话，不会再出来一串「全部对上」并默认勾选。

## 背景与先读什么
- 占位卡的来源：`apps/desktop/src-tauri/src/v2_ops.rs` 的 `import_summary_cards`（只读它，**不许改**）。测试里要用**这个真函数**造占位卡，不要手写 JSON。
- 读取方（都要接入清洗）：`library_view.rs`（`into_item`、`get_library_detail`、`load_card`、`pick_items`、`highlight_of`）；`solve.rs`（`load_repos`、`why_text`、`parse_card`）；`packs.rs`（`render_item_for_codex`、`build_pack` 以及读 `card_json` 渲染「它是什么 / 可借鉴的能力 / 怎么试」的那几处，约第 600–700 行）。
- 契约：`apps/desktop/src/lib/contracts.ts`（只读，不许改；字段形状都不变，只是值变干净）。
- 测试怎么造库：看 `library_view.rs`、`solve.rs`、`packs.rs` 各自 `mod tests` 里已有的夹具写法。

## 要做的事

### 1. 新模块 `src-tauri/src/card_view.rs`（在 `lib.rs` 里加一行 `mod card_view;`，这是 `lib.rs` 里你唯一能改的地方）
纯函数，不碰数据库：
1. 常量（与 `v2_ops.rs` 里写入的字符串逐字一致）：`LEGACY_PROBLEM = "想知道这个收藏是做什么的"`、`LEGACY_CAPABILITY = "查看项目说明"`、`LEGACY_HOW_TO_TRY = "打开仓库 README"`。
2. `is_legacy_placeholder(card: &Value) -> bool`：卡是对象，且满足下面任一条——
   - `problems_solved` 是数组，里面所有**非空字符串**元素都等于 `LEGACY_PROBLEM`，且至少有一个；
   - `capabilities` 是数组，里面所有元素（字符串，或对象的 `name_zh`）都等于 `LEGACY_CAPABILITY`，且至少有一个。
   不看 `confidence`、不看 schema 版本（真 AI 出的卡不会满足这两条）。`Value::Null`、非对象、坏数据一律返回假。
3. `derive_one_liner(card: &Value) -> Option<String>`：给占位卡用。取 `what_it_is`：
   - 取到第一个 `。` `！` `？` 或换行为止（**包含**这个句末标点）作为第一句；
   - 第一句不超过 60 个字符就直接用；
   - 超过 60 个字符：在前 60 个字符里找**最后一个** `，` `；` `、`，在它前面截断（不含这个标点）再加 `…`；前 60 个字符里没有这三个标点，就硬截前 60 个字符加 `…`；
   - 前后空白去掉；`what_it_is` 不是字符串或为空 → 返回 `None`。
4. `sanitize(card: &Value) -> Cow<'_, Value>`（或等价的返回「干净的卡」的写法，不是占位卡就**原样借用、不复制**）：占位卡返回一份副本，其中
   - `one_liner_zh` = `derive_one_liner` 的结果（`None` 时保留原值）；
   - `capabilities` = `[]`、`problems_solved` = `[]`、`how_to_try` 去掉（或设为 `null`）；
   - `what_it_is`：如果恰好 400 个字符（旧函数按 400 字硬截的）且其中有 `。`，截到最后一个 `。`（包含）；否则原样；
   - 其他字段（`confidence`、`review_status`、`form`、`license` 等）原样保留，**不要把置信度改高**。
5. `display_one_liner(stored: Option<&str>, card: &Value) -> Option<String>`：占位卡 → `derive_one_liner(card)`，拿不到就退回 `stored`（去空白，空串当 `None`）；非占位卡 → `stored` 去空白，空串当 `None`（和现在的行为一致）。

### 2. 接入 `library_view.rs`
- `into_item`：`one_liner` 用 `display_one_liner`；`highlight` 用清洗后的卡算（占位卡因此是 `None`）；`confidence`、`card_review`、`has_card` **不变**（占位卡仍算「已整理」，`has_card` 仍是真）。
- `get_library_detail`（`load_card` 之后）：详情的各字段都从清洗后的卡取（占位卡的 `capabilities`/`problemsSolved` 为空数组、`howToTry` 为 `null`；`whatItIs` 是清洗后的）。
- SQL 筛选里「已整理 / 整理中」的定义**不变**（仍按库里 `one_liner_zh` 非空）。`pick_items`、`week_items`、计数都不变，它们走 `into_item`，自然拿到干净的值。

### 3. 接入 `solve.rs`
- `load_repos`：用清洗后的卡取 `problems`、`capabilities`、`form`；`RepoRecord.one_liner` 和 `blocks.one_liner` 用 `display_one_liner`（所以打分、`why`、`SolveHit.oneLiner` 都不会再看到废话或半句话）。
- `why_text`：保持原有的挑选规则（先找包含意图词的问题/能力/一句话，没有就退回第一条非空的），不要加新规则；因为输入已经干净，占位卡的 `why` 会落到干净的一句话。
- **没有意图词时不硬凑**（改 `solve_query` 和 `score_repo`）：
  - 当 `main`（意图词）为空而 `extra` 非空：一个仓库至少要命中 `ceil(extra 个数 / 2)` 个不同的额外词才算命中（命中 = 该词在任一文本块里 `field_score > 0`），不够就不进结果；
  - 此时所有命中的 `full_match` 一律是 `false`，`preselected` 为空数组，`full_match_count` 为 0（因为我们不知道用户要什么，不能说「全部对上」）；
  - 当 `main` 非空：一个仓库至少要命中 1 个意图词才进结果（只靠额外词凑出分数的不要）；`full_match` 的算法不变（`missing` 为空）。
- 其余（`gap_intents`、`pending_matches`、`suggested_scenario_*`、排序、8 条上限）都不动。

### 4. 接入 `packs.rs`
- `packs.rs` 里解析 `card_json` 的唯一入口是 `card_value`（约第 624 行）：在这里把解析出来的卡过一遍 `card_view::sanitize`（返回拥有所有权的 `Value`）就够了，下游的 `resolve_one_liner`、`string_names`、`resolve_how_to_try` 等都不用动。占位卡因此没有「可借鉴的能力」「怎么试」两行，「它是什么」是清洗后的一句话。其余渲染规则、格式、错误信息不变。
- 同样，`library_view.rs` 里解析卡的唯一入口是 `parse_card`（约第 956 行），`solve.rs` 里是 `parse_card`（返回 `CardBits`）：优先在这些入口处接入，改动最小。

### 5. 测试（至少 24 个，新增在各自模块的 `mod tests` 和 `card_view.rs` 里）
- `card_view.rs`：`is_legacy_placeholder` 的真/假各情形（只有废话问题；只有废话能力；字符串形式的能力；两者混了真内容→假；空数组→假；`Null`/非对象→假）；`derive_one_liner`：第一句 ≤ 60 字（直接用，保留句末标点）、> 60 字有逗号（在最后一个逗号前截断加 `…`）、> 60 字没有逗号（硬截 60 + `…`）、没有句末标点、`what_it_is` 为空/缺失/非字符串；`sanitize`：占位卡各字段、非占位卡**指针相同**（借用，不复制）、`confidence` 不变、400 字硬截的 `what_it_is` 截到最后一个 `。`；`display_one_liner` 的各分支。
- **用真函数造占位卡的回归测试**（放在 `library_view.rs`、`solve.rs`、`packs.rs` 的 tests 里）：建一个临时库，往 `repo_ai_documents` 里放 `summary_zh`（用下面的两段真实摘要），调用 `crate::v2_ops::import_summary_cards(path)` 生成占位卡，然后断言：
  - `list_library_view` 的 `oneLiner` 等于第一句（见下面的期望值）、`highlight` 是 `null`、`hasCard` 是真；
  - `get_library_detail` 的 `capabilities`、`problemsSolved` 为空数组，`howToTry` 为 `null`；
  - `solve_query` 命中这个仓库时，`why` 和 `oneLiner` **不含**三个废话字符串、不是半句话；
  - `packs::render_item_for_codex` 的输出**不含** `查看项目说明`、`打开仓库 README`、`想知道这个收藏是做什么的`；
  - **整个返回的 JSON 序列化后也不含这三个字符串**（对 `list_library_view` / `get_library_detail` / `solve_query` 的结果各断言一次）。
- 找方案：`没有意图词的乱输入`（如「xyzzy不存在的东西」）在造好的库里返回的 `hits` 为空，或者每条的 `full_match` 都是 `false` 且 `preselected` 为空；额外词命中一半才算（造一个只命中 1/6 的仓库不进、命中 3/6 的进）；有意图词时「只靠额外词」的仓库不进结果；有意图词的正常查询结果和改动前一致（挑一个已有的 solve 测试保持原断言通过）。

两段真实摘要（Fox 开发库里的原文，用作测试夹具）：
1. 摘要：「Awesome LLM Apps 是一个可运行的 LLM 应用模板集合，包含100+ AI Agent和RAG应用。每个模板均为原创、自包含的完整源代码，支持3条命令即可运行。覆盖AI Agents、多智能体团队、MCP、语音AI、RAG、微调等现代AI栈，支持Claude、Gemini、GPT、Llama等多种模型提供商。采用Apache-2.0许可证，可自由商用。」→ 期望一句话：「Awesome LLM Apps 是一个可运行的 LLM 应用模板集合，包含100+ AI Agent和RAG应用。」
2. 摘要：「ChatCut Agent Plugin 是一个连接 Codex 与 ChatCut 的插件，支持通过 AI 辅助编辑 ChatCut 视频项目，具体功能包括导入媒体、修改时间线、创建动态图形、生成素材、音频转录、添加字幕、导出视频以及验证编辑可见性。」→ 第一句超过 60 个字符，在前 60 个字符内最后一个 `，` 之前截断：期望「ChatCut Agent Plugin 是一个连接 Codex 与 ChatCut 的插件…」

## 不要做
- 不改 `v2_ops.rs`（冻结）、迁移、`contracts.ts`、任何前端文件、`card_pipeline.rs`、`scenarios.rs`、`ai.rs`、`jobs.rs`、`card_handler.rs`（卡 14 在改）；`lib.rs` 里**只加** `mod card_view;` 一行。
- 不改库里的数据，不新增迁移，不新增依赖；不改 SQL 里「已整理 / 整理中」的定义。
- 不要启动 App，不要读写任何真实数据目录（测试只用临时库）。

## 验收
1. `tools/cards/run.sh rust-test` 全通过，新增测试**至少 24 个**，覆盖上面列的每一项；`rust-fmt`、`rust-check` 通过，警告数不比卡开始前多（4 条）。
2. 逐项给出：`card_view.rs` 的五个函数在哪个文件哪一行；`library_view.rs`、`solve.rs`、`packs.rs` 接入清洗的位置（文件:行）；找方案「没有意图词」规则的位置（文件:行）。
3. 在报告里贴出：两段真实摘要经过 `import_summary_cards` + 本卡清洗之后，`oneLiner`、详情字段、`render_item_for_codex` 输出的**实际文本**（从你的测试里打印出来），方便总控肉眼核对。
4. `git diff --stat` 里只出现「允许改动」里的路径；`v2_ops.rs` 没有改动；`lib.rs` 只多了一行 `mod card_view;`（给出 `git diff -- apps/desktop/src-tauri/src/lib.rs`）。

## 停止条件
- 需要改 `v2_ops.rs`、迁移、契约、前端或卡外文件；需要新依赖；发现占位卡还有别的固定废话字符串且无法在读取侧清洗——停手并写进报告。
