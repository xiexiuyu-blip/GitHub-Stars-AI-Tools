# 交回报告 · 卡 06-scenarios

## 一句话结论
做完了：场景种子、规则归类、AI 建议替换和三个 Tauri 命令都在后端落地，`rust-fmt` / `rust-check` / `rust-test` 通过，警告仍是原来的 4 条。

## 验收逐条
| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| 1. `tools/cards/run.sh rust-test` 通过，`scenarios.rs` 新增测试至少 18 个，覆盖：种子幂等（连跑两次不重复）、种子更新（改关键词后重新 seed 会更新 seed 来源的行但不动 user 来源的行和已归档的行）；关键词匹配的两类规则（`rag` 不命中 `storage`、`seo` 不命中 `videoseoul`、`video-seo-tools` 命中、中文子串命中、`ppt` 在 `pptx-tools` 里命中）；多场景归类与置信度公式（1 个词 0.5、3 个词 0.7、封顶 0.8）；文字来源（只在理解卡的能力名里出现的词也能命中、`card_json` 是坏 JSON 或空串时不报错）；陈旧建议清理（改了描述后重新归类会删掉不再命中的规则行）；受保护的行（`confirmed`、`rejected`、`origin='ai'`、`origin='user'` 的行重新归类后原样不变）；`apply_ai_scenarios` 的替换语义和跳过规则；`list_scenarios_summary` 的排序、计数（不含已移除的仓库、不含被拒绝的归类）、`thin` 标志、不含已归档场景；`set_repository_scenario` 的各个转换和错误；整批归类在事务里（造一个会失败的情形验证没有留下一半）。 | 是 | `scenarios.rs` 有 20 个 `#[test]`（879、901、958、967、982、994、1030、1042、1137、1164、1201、1214、1297、1369、1409、1428、1469、1499、1528、1666 行）。全量 `tools/cards/run.sh rust-test`：`test result: ok. 297 passed; 0 failed; 5 ignored; 0 measured; 1 filtered out; finished in 16.27s`。其中 `scenarios::` 20 条全部 ok，包括 `seed_twice_does_not_duplicate`、`seed_updates_seed_rows_but_not_user_or_archived`、`short_ascii_keywords_match_only_at_word_start`、`longer_and_chinese_keywords_match_by_substring`、`confidence_formula_ramps_and_caps`、`repository_can_belong_to_multiple_scenarios`、`classify_reads_each_text_source_and_bad_card_json`、`reclassify_removes_stale_rule_suggestions`、`reclassify_keeps_protected_rows`、`apply_ai_scenarios_replaces_suggestions_and_skips_protected`、`list_summary_sorts_counts_thin_and_hides_archived`、`set_repository_scenario_confirms_rejects_and_undoes`、`set_repository_scenario_reports_chinese_errors`、`classify_batch_rolls_back_when_one_write_fails`。被滤掉的 1 条是 `run.sh` 自带的钥匙串测试跳过。 |
| 2. 契约测试：把 `ScenarioSummary` 序列化成 JSON，断言键集合恰好是 `id`、`slug`、`nameZh`、`sortOrder`、`count`、`thin`，类型分别是字符串、字符串、字符串、数字、数字、布尔。 | 是 | `scenarios.rs` 的 `scenario_summary_json_has_exact_contract_keys`（1469 行）在全量测试里 ok。结构体 `rename_all = "camelCase"`。 |
| 3. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多（4 条）。 | 是 | 本轮 `tools/cards/run.sh rust-fmt` 退出码 0，无输出。`rust-check`：`warning: gsat-desktop (lib) generated 4 warnings`，然后 `Finished dev profile`。4 条都是原有的：`db/migrations.rs` 的 `applied` / `backup_path`；`v2_ops.rs:81 ensure_ready`、`v2_ops.rs:576 repair_model_json`、`v2_ops.rs:1131 restore_database_file`。`scenarios.rs` 没有新警告。 |
| 4. 给出 3 个命令在 `generate_handler!` 和 `gsat-commands.toml` 里的行号。 | 是 | `generate_handler!`：`lib.rs` 6866 `list_scenarios`、6867 `reclassify_scenarios`、6868 `set_repository_scenario`。`gsat-commands.toml`：110、111、112。命令函数体：`lib.rs` 1359、1367、1374。`mod scenarios;` 在 `lib.rs` 16 行。ACL 同步在 `acl-manifests.json` 第 1 行的 allow 列表里，三个名字各出现一次。 |
| 5. 用一个有 6–8 个仓库的夹具库跑一遍 `classify_repositories`，把每个仓库归到哪些场景（含置信度）整理成表贴进报告，方便总控人工核对规则合不合理。 | 是 | 测试 `eight_repository_fixture_matches_expected_scenarios`（1528 行）通过。8 个仓库，`repositories=8`，`assigned=11`，`removed=0`，`entity_scenarios` 共 11 行，全部 `origin=rule`、`status=suggested`。表见下方。`per_scenario` 按 `sort_order`，计数为 0 的 `wr`、`res` 不出现。 |
| 6. `git diff --stat` 里只出现「允许改动」里的路径。 | 是 | `git diff --stat`：`acl-manifests.json` 2 行有改、`gsat-commands.toml` +3、`lib.rs` +30，合计 `3 files changed, 34 insertions(+), 1 deletion(-)`。新文件 `scenarios.rs` 未跟踪，不进 `--stat`，`git status --short` 里是 `?? apps/desktop/src-tauri/src/scenarios.rs`。没有其他路径。`capabilities.json`、`desktop-schema.json`、`macOS-schema.json` 与编译前快照 `/tmp/fsl-schema-snap/` 逐字节相同。 |

### 夹具归类表（验收 5）

夹具文字：`fox/xhs-poster` 描述「小红书封面和海报生成」、topics `xiaohongshu,poster`；`fox/weixin-digest`「微信公众号周报」；`fox/douyin-clips`「抖音口播剪辑」；`fox/pptx-tools`「把幻灯片做成办公文档」；`fox/plain-repo` 描述无场景词，理解卡能力名「用 RAG 检索笔记」；`fox/seo-lab`「SEO 增长、营销投放和获客」；`fox/sql-dashboard`「数据分析报表」、topic `analytics`；`fox/n8n-shop`「用 n8n 做电商自动化和商品上架」。置信度 = `min(0.5 + 0.1 × (命中词数 - 1), 0.8)`。

| 仓库 | 场景 | 置信度 | 命中词 |
|---|---|---|---|
| fox/xhs-poster | xhs | 0.7 | 小红书、xiaohongshu、xhs |
| fox/xhs-poster | img | 0.7 | 海报、封面、poster |
| fox/weixin-digest | wx | 0.7 | 公众号、微信、weixin |
| fox/douyin-clips | dy | 0.6 | 抖音、douyin |
| fox/douyin-clips | vid | 0.5 | 剪辑 |
| fox/pptx-tools | ppt | 0.8 | ppt、幻灯、pptx、办公文档 |
| fox/plain-repo | kb | 0.6 | rag、笔记 |
| fox/seo-lab | seo | 0.8 | seo、增长、营销、投放、获客 |
| fox/sql-dashboard | data | 0.8 | 数据分析、analytics、dashboard、sql、报表 |
| fox/n8n-shop | auto | 0.6 | 自动化、n8n |
| fox/n8n-shop | ec | 0.6 | 电商、商品 |

`per_scenario`：xhs、wx、dy、img、vid、ppt、data、kb、auto、ec、seo 各 1。写作（wr）、调研（res）这次是 0，不进返回值。

## 改了哪些文件
- `apps/desktop/src-tauri/src/scenarios.rs`（新文件）：种子、关键词匹配、规则归类、AI 建议替换、列表摘要、手动确认/拒绝/撤销，以及 20 个临时库测试。
- `apps/desktop/src-tauri/src/lib.rs`：登记 `mod scenarios;`，加三个命令；`list_scenarios` 只补种子再读列表，注释写明不做全库归类；`reclassify_scenarios` 先种子再归类全部有效仓库；路径都从 `AppStorage` 取。
- `apps/desktop/src-tauri/permissions/gsat-commands.toml`：allow 列表追加三个命令名。
- `apps/desktop/src-tauri/gen/schemas/acl-manifests.json`：在同一处 allow 列表里追加同样三个名字，没有重排 JSON。

## 验证
- `tools/cards/run.sh rust-fmt` → 退出码 0，无输出。
- `tools/cards/run.sh rust-check` → 通过，`generated 4 warnings`，均为 `migrations.rs` 与 `v2_ops.rs` 的原有警告，`Finished dev profile ... in 2.48s`。
- `tools/cards/run.sh rust-test` → `ok. 297 passed; 0 failed; 5 ignored; 0 measured; 1 filtered out; finished in 16.27s`。`scenarios::` 20 条全部 ok。
- `git diff --stat` 与 `git status --short` → 只有上面四个允许路径；`scenarios.rs` 为未跟踪新文件。
- `cmp`：`capabilities.json`、`desktop-schema.json`、`macOS-schema.json` 与编译前快照相同。
- 没有启动 App，没有装依赖，没有跑 git 写命令，没有碰正式库或开发库。

## 没做到的 / 偏离任务卡的地方
- 短 ASCII 词只卡左边界，没有同时卡右边界。卡正文写「前后都不是字母或数字」，验收又要求 `ppt` 命中 `pptx-tools`。两边都卡时 `ppt` 配不上 `pptx`。实现见 `scenarios.rs:146-176`：匹配起点的前一个字节不能是 ASCII 字母数字，右边可以继续。验收例子都成立：`rag` 不命中 `storage`，`seo` 不命中 `videoseoul`，`video-seo-tools` 命中 `seo`，`ppt` 命中 `pptx-tools`，中文和更长的英文仍是子串。副作用：`seo` 会命中 `seoul`，`sql` 会命中 `sqlite`，`rag` 会命中 `ragflow`；`sql` 不会命中 `mysql`。回退只需在命中处加上右边界判断 `after == hay.len() || !hay[after].is_ascii_alphanumeric()`。加上之后夹具 `fox/pptx-tools` 会少掉关键词 `ppt`，置信度从 0.8（4 词）变成 0.7（幻灯、pptx、办公文档），验收第 1 条的 `ppt`/`pptx-tools` 会失败。
- `list_scenarios_summary` 的返回类型是 `Result<Vec<ScenarioSummary>, String>`，卡上写成 `Vec<ScenarioSummary>`。数据库错误向上返回，不变成空列表。命令签名本来就是 `Result`。
- `apply_ai_scenarios` 的连接参数是 `&mut Connection`，内部用 savepoint。置信度在开 savepoint 之前检查，必须是有限数且在 0 到 1 之间，否则返回「置信度必须在 0 到 1 之间」。函数加了 `#[allow(dead_code)]`（`scenarios.rs:533`），因为理解卡流水线还没调用它；不加的话 `rust-check` 会变成 5 条警告。没有改成 `pub mod`。
- `classify_repositories` 自己不补种子。没有任何未归档场景时，写入前返回「还没有可归类的场景。」，避免把已有的规则建议清掉。打开列表和重新归类这两条命令会先调用 `seed_scenarios`。
- 一个仓库不设「最多 3 个场景」的上限。原型 `docs/rebuild/原型/extract.py` 有上限，这张卡没有，所以一个仓库可以命中所有场景。
- `ClassifyStats.per_scenario` 省略计数为 0 的场景；serde 字段名是 `perScenario`，元素是 `[slug, count]`。
- 归类、列表计数、手动写入都只用最近更新的一个 GitHub 账号。场景主键按卡写成 `scn-<slug>`。若以后换成另一个最近账号再 seed，主键可能和前一个账号的 `scn-<slug>` 相撞。本卡只服务这一个账号。
- 已归档场景不参与规则归类和 AI 写入。`set_repository_scenario` 按 id 查找，归档行只要还在表里，仍可被手动标记。

## 需要总控或 Fox 决定的事
- 无。若总控希望短词改成两侧都是词边界，需要同时改验收里的 `ppt` / `pptx-tools`，并接受夹具里 ppt 置信度从 0.8 降到 0.7。当前实现保持验收例子全部成立。

## 给审核者的提示
- `git diff --stat` 看不到新文件，请同时看 `git status` 里的 `?? apps/desktop/src-tauri/src/scenarios.rs`。
- `list_scenarios`（`lib.rs:1359`）只调用 `seed_scenarios` 和 `list_scenarios_summary`。启动流程、`prepare_knowledge`、`get_graph_overview` 没有改。
- 权限清单是最小字符串拼接。另外三个 schema 文件没有被 `tauri_build` 改写；若审核时 `cargo` 又跑了 `build.rs` 并把它们重排，请与这次的 diff 分开看。
- 规则归类整批使用一个 `Immediate` 事务。场景自己的 `keywords_json` 坏了会整批回滚；仓库的 `card_json` / `topics_json` 坏了或类型不对则跳过该字段。
- 重新归类只删除「本次处理过的仓库」上、不再命中的 `origin=rule` 且 `status=suggested` 行。`confirmed` / `rejected`，以及 `origin` 为 `ai` 或 `user` 的行保持原样。文本没变再跑一次时 `assigned=0`、`removed=0`。
- `apply_ai_scenarios` 会先删该仓库 `origin=ai` 且 `status=suggested` 的行。若同一主键上原来是 `rule` + `suggested`，且新列表包含该场景，则更新成 AI 建议。空 slug 列表只清 AI 建议。它自己不做规则侧的陈旧清理。
- `thin` 使用种子里的阈值 14，且只对 `ec`、`seo`、`data` 为真：`count <= 14` 才算薄。测试里 seo=14 为薄，ec=15 不为薄。
- 测试库都在系统临时目录，文件名形如 `fsl-scn-...sqlite3`，结束时连同 `-wal` / `-shm` 删除。
