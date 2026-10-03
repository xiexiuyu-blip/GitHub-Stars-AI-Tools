# 交回报告 · 卡 08-solve

## 一句话结论
做完了：「找方案」后端 `solve_v2` 已按原型打分接上，只读临时库，30 个新测试和全量 `rust-test` 都通过。

## 验收逐条
| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| `tools/cards/run.sh rust-test` 通过，`solve.rs` 新增测试至少 22 个，覆盖：`tokens`（含中英混排、两字以内的汉字片段、长汉字片段切二元组、停用词被去掉、去重）；`concepts`（「做小红书封面」→ main 为 小红书、封面；「短视频加字幕」→ 短视频、字幕，且「视频」不会再被重复当意图；最长优先；零散词的长度限制）；词表按场景动态扩展且去重、按长度排序；`idf` 的数值（用一个小语料手算对照）；`field_score` 各块的权重和取最大值；多意图命中的加成公式；`extra` 的 0.3 权重；排序的并列规则；用一个 12 个仓库的夹具库跑通原型里的 5 句例子，断言「做小红书封面」有全部对上的结果、「做电商详情页图片」在夹具里没有任何全部对上的（`fullMatchCount` 为 0 且 `gapIntents` 非空）、`preselected` 在没有完全命中时为空；`hits` 上限 8；没有卡的仓库不进 `hits` 但计入 `pendingMatches`；`why` 的取值顺序和 80 字截断；`suggestedScenarioId` 与计数；空查询报错、全是停用词返回空结果；坏 JSON、空串、字段类型不对的 `card_json` 不会让查询失败；已移除（`sync_status<>'active'`）的仓库不参与。 | 是 | `solve.rs` 里 `#[test]` 共 30 个（1044 行起）。全量 `tools/cards/run.sh rust-test`：`307 passed; 0 failed; 5 ignored; 0 measured; 1 filtered out; finished in 14.65s`。30 个 `solve::tests::*` 全部 `ok`，名单见「验证」。 |
| 契约测试：把 `SolveHit`、`SolveResult` 各序列化一份，断言键集合和类型与 `contracts.ts` 一致（含 `pendingMatches`、`suggestedScenarioId` 为 `null` 的情形）。 | 是 | `solve.rs` `contract_json_keys_and_types`（2185 行）。断言键集合、字符串/数字/布尔/数组，以及 `oneLiner`/`form`/`suggestedScenarioId`/`suggestedScenarioCount` 为 `null`。该测试在全量套件里 `ok`。 |
| `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多（4 条）。 | 是 | `rust-fmt` 退出码 0。`rust-check` 退出码 0，`Finished dev profile in 3.69s`，`generated 4 warnings`。4 条都是原有死代码：`db/migrations.rs:14` 的 `applied`/`backup_path`，`v2_ops.rs:81` `ensure_ready`，`v2_ops.rs:576` `repair_model_json`，`v2_ops.rs:1131` `restore_database_file`。`solve.rs` 没有新警告。 |
| 给出 `solve_v2` 在 `generate_handler!` 和 `gsat-commands.toml` 里的行号；证明 `v2_ops.rs` 没有改动。 | 是 | `generate_handler!`：`lib.rs:6829` `solve_v2,`（紧挨 `solve_problem,`）。命令函数：`lib.rs:1089`。权限：`permissions/gsat-commands.toml:93` `"solve_v2",`。`git diff --stat -- apps/desktop/src-tauri/src/v2_ops.rs` 无输出。 |
| 性能：在测试里造 3000 个仓库的临时库，一次 `solve_query` 在本机应当不超过 1 秒；把实测耗时写进报告。超过就优化。 | 是 | `three_thousand_repos_solve_within_one_second` 在全量套件里 `ok`。该测试只计 `solve_query`，写入 `/tmp/fsl-08-solve-perf.txt`：`94 ms`，`hits 8`，`fullMatchCount 0`。断言上限是 1.0 秒。同一次过滤跑曾记到 124 ms，也低于 1 秒。 |
| 在报告里贴出夹具库上这 5 句话的完整结果摘要（意图、前三条命中的名称与得分、`fullMatchCount`、`gapIntents`、`preselected`），方便总控人工核对合不合理。 | 是 | 测试 `fixture_five_prototype_queries` 通过，并把摘要写到 `/tmp/fsl-08-solve-five.txt`。全文见下方。 |
| `git diff --stat` 里只出现「允许改动」里的路径。 | 是 | `git diff --stat` 只有 `acl-manifests.json`、`gsat-commands.toml`、`lib.rs`。新文件 `solve.rs` 未跟踪，所以不进 `diff --stat`；`git status --short` 为 `?? apps/desktop/src-tauri/src/solve.rs`。`v2_ops.rs` 无差异。`rust-check` / `rust-test` 之后没有多出卡外文件。 |

### 五句夹具摘要

夹具：未归档场景 sc-xhs / sc-wx / sc-video / sc-doc / sc-ec，另有已归档 sc-arch（关键词「小红书封面」，不进词表、不当建议场景）。有卡仓库 r01–r11，无卡 r12（描述含「小红书封面」）。得分保留 4 位小数，是测试打印值。

做小红书封面

- intents：小红书、封面。extras：无。
- fullMatchCount：1。gapIntents：无。preselected：r01。pendingMatches：1。
- suggestedScenarioId：sc-xhs。suggestedScenarioCount：2。
- 1. fox/xhs-cover score=11.0904 stars=5200 full=true matched=小红书、封面 why=想快速做小红书封面
- 2. fox/poster-lab score=3.4657 stars=8000 full=false matched=封面 missing=小红书 why=生成封面海报
- 3. fox/xhs-notes score=3.4657 stars=2100 full=false matched=小红书 missing=封面 why=写小红书文案

给公众号文章配图

- intents：公众号、配图、文章。extras：无。
- fullMatchCount：1。gapIntents：无。preselected：r04。pendingMatches：0。
- suggestedScenarioId：sc-wx。suggestedScenarioCount：2。
- 1. fox/wx-article-images score=30.2569 stars=3400 full=true matched=公众号、配图、文章 why=公众号文章缺配图
- 2. fox/wx-layout score=4.0236 stars=1800 full=false matched=公众号 missing=配图、文章 why=公众号排版太花时间
- 只有 2 条命中。

短视频加字幕

- intents：短视频、字幕。「视频」不是意图。extras：无。
- fullMatchCount：1。gapIntents：无。preselected：r06。pendingMatches：0。
- suggestedScenarioId：sc-video。suggestedScenarioCount：1。
- 1. fox/video-subtitle score=15.5673 stars=4600 full=true matched=短视频、字幕 why=短视频没有字幕

做 PPT 和文档

- intents：ppt、文档。extras：无。
- fullMatchCount：1。gapIntents：无。preselected：r08。pendingMatches：0。
- suggestedScenarioId：sc-doc。suggestedScenarioCount：1。
- 1. fox/ppt-doc score=17.1240 stars=2700 full=true matched=ppt、文档 why=要做一份 PPT

做电商详情页图片

- intents：详情页、电商、图片。extras：无。
- fullMatchCount：0。gapIntents：图片。preselected：空。pendingMatches：0。
- suggestedScenarioId：sc-ec。suggestedScenarioCount：2。
- 1. fox/detail-layout score=12.9338 stars=900 full=false matched=详情页、电商 missing=图片 why=详情页太乱
- 2. fox/ecom-main-image score=4.0236 stars=3900 full=false matched=电商 missing=详情页、图片 why=电商主图不好看

## 改了哪些文件
- `apps/desktop/src-tauri/src/solve.rs`：新模块。词表、拆意图、idf、打分、`solve_query`，以及 30 个测试。不依赖 Tauri。
- `apps/desktop/src-tauri/src/lib.rs`：`mod solve;`（第 16 行）；命令 `solve_v2`（1088–1092 行）从 `AppStorage` 取库路径；`generate_handler!` 登记（6829 行）。
- `apps/desktop/src-tauri/permissions/gsat-commands.toml`：在 `solve_problem` 后加入 `solve_v2`（第 93 行）。
- `apps/desktop/src-tauri/gen/schemas/acl-manifests.json`：`allow-gsat-commands` 的命令列表里，`solve_problem` 后加入 `solve_v2`。

## 验证
- `tools/cards/run.sh rust-fmt-fix` → 退出码 0。
- `tools/cards/run.sh rust-fmt` → 退出码 0。
- `tools/cards/run.sh rust-check` → 退出码 0，3.69s，4 条原有警告（见上）。
- `tools/cards/run.sh rust-test` → `307 passed; 0 failed; 5 ignored; 0 measured; 1 filtered out; finished in 14.65s`。被过滤的 1 条是 `run.sh` 固定跳过的钥匙串测试。5 条 ignored 是原有用例（含 `vector_index::tests::local_user_catalog_vector_search_smoke`），本卡没有加 `#[ignore]`。
- `solve::` 30 条均 `ok`：`tokens_keeps_latin_inside_chinese`、`tokens_keeps_short_han_fragment`、`tokens_splits_long_han_into_bigrams`、`tokens_removes_stopwords`、`tokens_dedups_and_splits_punctuation`、`concepts_splits_xiaohongshu_cover`、`concepts_short_video_subtitle_drops_video`、`concepts_prefers_longest_scenario_keyword`、`concepts_drops_extra_shorter_than_two`、`concepts_ppt_document_is_case_insensitive`、`lexicon_extends_sorts_and_dedups`、`idf_matches_hand_calculation`、`field_score_weights_take_the_max`、`multi_intent_bonus_includes_extra_before_multiplier`、`extra_terms_use_point_three_weight`、`ties_sort_by_stars_then_id`、`fixture_five_prototype_queries`、`hits_capped_at_eight`、`pending_matches_need_one_field_to_hold_every_intent`、`why_follows_problem_capability_one_liner_and_truncates`、`suggested_scenario_skips_archived_and_uses_sort_order`、`empty_query_returns_chinese_error`、`stopwords_or_punctuation_return_empty_result`、`bad_card_json_does_not_fail`、`removed_repositories_do_not_participate`、`no_intent_still_marks_full_match`、`contract_json_keys_and_types`、`solve_v2_is_listed_in_permissions`、`gap_intents_only_look_at_returned_hits`、`three_thousand_repos_solve_within_one_second`。
- `git diff --stat`：`acl-manifests.json | 2 +-`，`gsat-commands.toml | 1 +`，`lib.rs | 8 ++++++++`。`git diff --stat -- v2_ops.rs` 为空。
- 3000 仓实测：94 ms（全量套件这次写入的 `/tmp/fsl-08-solve-perf.txt`）。

## 没做到的 / 偏离任务卡的地方
- 任务卡接入处写的是 `solve_v2(query: String) -> SolveResult`。实现是 `Result<solve::SolveResult, String>`（`lib.rs:1089`），和旁边的 `solve_problem` 一样。空查询因此能返回错误「先写一句你想做的事」。成功时的 JSON 字段没有改，契约文件没动。
- 其余按卡里写明的五处和原型不同：hits 只含有理解卡的仓库；关键词块权重 2.5×idf 并计入 df；词表按未归档场景扩展；并列再按 id 升序；why 按问题、能力、一句话取值。

## 需要总控或 Fox 决定的事
- 无。

## 给审核者的提示
- 只读打开：`SQLITE_OPEN_READ_ONLY`、`busy_timeout` 5000、`PRAGMA query_only=ON`（`solve.rs` `open_read`，331 行）。不写任何表。测试库在系统临时目录，用完删除 sqlite/-wal/-shm。
- `pendingMatches`：名称整段包含全部意图，或者描述整段包含全部意图（278–289 行）。两个字段各有一部分时不计。`main` 为空时为 0。
- `gapIntents`、`fullMatchCount`、`preselected` 只看已经截到最多 8 条的 `hits`（261–277 行）。第 9 名以后即使对上了某个意图，也不算补上缺口，也不增加完全命中条数。没有完全命中时 `preselected` 是空数组。
- 已归档场景不进词表、不当 `suggestedScenarioId`。仓库若仍挂着未拒绝的归档场景，场景名仍参与打分。建议场景按 `sort_order`、再按 id；场景全名或任一关键词（小写后）包含某个意图或零散词即算，是子串包含。
- 有理解卡 = `one_liner_zh` 去掉空白后非空。纯空白、只有坏 `card_json`，都不算有卡。
- 场景名按「·」（U+00B7）拆段，去掉空白后不设最短长度。场景关键词去掉空白后至少 2 个字符才进词表。等长词保持插入顺序：基础词表原顺序，然后未归档场景按 `sort_order`、id。`scenarioIds` 同样先 `sort_order` 再场景 id。
- 多意图加成先加上 extra 的 `0.3×field_score`，再把总分乘 `1+0.6×(命中数-1)`（`score_repo`，522–528 行）。`field_score` 取各块最大值。`idf = ln(1+N/(1+df))`。
- `why` 按 Unicode 字符数截断：超过 80 字取前 80 再加「…」（结果可以是 81 个字符）；正好 80 不加省略号。
- 生产代码没有 `unwrap` / `expect`。`parse_card` 对空串、坏 JSON、非对象、字段类型不对都当成没有，查询继续。`form` 只收非空字符串。
- 本卡没有改迁移、前端、`contracts.ts`、`v2_ops.rs`，也没有加依赖。界面这张卡看不到，所以没有开 App、没有跑 web 测试。
