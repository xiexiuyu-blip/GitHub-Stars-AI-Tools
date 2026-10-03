---
编号: 06-scenarios
允许改动:
  - apps/desktop/src-tauri/src/scenarios.rs
  - apps/desktop/src-tauri/src/lib.rs
  - apps/desktop/src-tauri/permissions/gsat-commands.toml
  - apps/desktop/src-tauri/gen/schemas/acl-manifests.json
---

# 卡 06 · 业务场景：种子、规则归类、命令（后端）

## Fox 能看到的效果
这是后台积木，**这张卡 Fox 暂时看不到界面变化**。做完后，库里的 495 个仓库会被归到「小红书 / 公众号·微信 / 抖音·短视频 / 海报·封面·设计 / 视频·字幕·语音 / PPT·文档 / 写作·文案 / 调研·爬虫 / 数据分析 / 知识库·笔记 / 自动化·工作流 / 电商 / SEO·增长」这 13 个业务场景里（一个仓库可以属于多个场景）。资料库页的左侧筛选、今日页的「值得先看」、找方案的「缺口」都靠它。这是 Fox 要的三个新功能之一「业务场景视图」的数据底座。

## 背景与先读什么
- **表已建好**：迁移 `packages/storage/migrations/fox/030_scenarios.sql`（`scenarios`、`entity_scenarios`，**不要改迁移**；`entity_scenarios` 的主键是 `(entity_type, entity_id, scenario_id)`，`origin` 是 `rule`/`ai`/`user`，`status` 是 `suggested`/`confirmed`/`rejected`）。迁移已在 `db/migrations.rs` 登记。
- **种子**：`docs/rebuild/spec/scenarios_seed.json`（13 个场景，每个带关键词）。用 `include_str!("../../../../docs/rebuild/spec/scenarios_seed.json")` 读（路径写法照 `v2_ops.rs` 里的 `TAXONOMY_SEED`）。里面还有 `thin_threshold`（14）和 `thin_focus`（`ec`、`seo`、`data`）。
- **照着做的现成样子**：`apps/desktop/src-tauri/src/v2_ops.rs` 里的 `seed_taxonomy` 和 `rule_classify`（技术分类的种子和规则归类，**不要改它们**）。账号取法：库里最近更新的 `github_accounts` 行。
- **契约（不许改名、不许改形状）**：`apps/desktop/src/lib/contracts.ts` 里的 `ScenarioSummary`。`list_scenarios` 必须按它返回 JSON（serde 的 `camelCase`）。
- 数据来源（归类时可读的文字）：`repositories` 的 `full_name`、`description`、`topics_json`；`repo_ai_documents` 的 `summary_zh`、`one_liner_zh`、`keywords_json`、`suggested_tags_json`，以及 `card_json`（理解卡，格式见 `docs/rebuild/spec/ai_card.schema.json`：`capabilities[].name_zh`、`problems_solved`、`use_when`、`inspiration_for_fox` 等）。`card_json` 可能是空串、坏 JSON、字段类型不对，**都要容忍**，不能让整个归类失败。
- 测试怎么造库：看 `db/migrations.rs` 里 `empty_database_applies_every_migration`，再用 SQL 插 `github_accounts`、`repositories`、`repo_ai_documents`。**所有测试只用临时库。**

## 要做的事

### 新模块 `src-tauri/src/scenarios.rs`（不依赖 Tauri）
1. **`seed_scenarios(db_path) -> Result<usize, String>`**：幂等。场景 id 取 `scn-<slug>`。缺的插入；已存在且 `origin='seed'` 的，名称、关键词、排序和种子不一致就更新；`origin='user'` 的和 `archived=1` 的**不动**。返回新插入的个数。
2. **匹配规则 `matches_keyword(haystack_lower, keyword_lower) -> bool`**：关键词和文本都转小写。关键词里**全是 ASCII 字母数字、且长度不超过 4** 的（`ppt`、`seo`、`rag`、`xhs`、`tts`、`asr`、`sql`、`wiki`、`n8n`、`rpa`……），要求在文本里是「单词」：前后都不是字母或数字才算命中（所以 `rag` 不会命中 `storage`，`seo` 不会命中 `videoseoul`，但 `video-seo-tools` 命中）；其余关键词（中文、较长的英文）直接子串匹配。写测试把这两类各覆盖到。
3. **`classify_repositories(db_path, repo_ids: Option<&[String]>) -> Result<ClassifyStats, String>`**（`None` = 全部有效仓库）：
   - 每个仓库把可读文字拼成一段小写文本：名称（`full_name`）、描述、topics、`summary_zh`、`one_liner_zh`、`keywords_json`、`suggested_tags_json`、理解卡里的能力名、解决的问题、适用场景、`inspiration_for_fox`。读不出的字段跳过。
   - 每个场景：命中的**不同关键词**个数 `n`；`n≥1` 就归入，`confidence = min(0.5 + 0.1 × (n-1), 0.8)`。
   - 写 `entity_scenarios`（`entity_type='repository'`，`origin='rule'`，`status='suggested'`）：UPSERT；**已存在的 `confirmed`、`rejected` 行，以及 `origin` 是 `ai` 或 `user` 的行，一律不改**；这个仓库**以前由规则写的、`status='suggested'`、这次不再命中的行要删掉**（陈旧建议清理）。整批在一个事务里。
   - 返回 `ClassifyStats { repositories（处理了多少仓库）, assigned（本次写入或更新的归类行数）, removed（清掉的陈旧行数）, per_scenario: Vec<(slug, count)> }`。
4. **`apply_ai_scenarios(conn, account_id, repo_id, slugs: &[String], confidence: f64) -> Result<usize, String>`**：给以后的理解卡流水线用（卡里的 `scenario_tags`）。先删掉这个仓库**`origin='ai'` 且 `status='suggested'`** 的行，再写入 `slugs` 对应场景（`origin='ai'`，`status='suggested'`）；已经有 `confirmed`/`rejected`/`user` 行的场景跳过；不认识的 slug 静默丢掉；返回写入条数。
5. **`list_scenarios_summary(db_path) -> Vec<ScenarioSummary>`**：不含 `archived=1`；按 `sort_order` 升序；`count` = 该场景下 `sync_status='active'` 且归类 `status<>'rejected'` 的仓库数；`thin` = `count <= thin_threshold` 且场景在 `thin_focus` 里。
6. **`set_repository_scenario(db_path, repo_id, scenario_id, status) -> Result<(), String>`**：`status` 只能是 `confirmed`、`rejected`、`suggested`；写成 `origin='user'`（没有这一行就新建，`confidence=1.0`）。`suggested` 表示「撤销我的确认或拒绝」：如果这一行原本是规则或 AI 写的就还原不了，所以实现为**删除这一行的用户标记**：该行 `origin='user'` 时删除；否则报错「这条归类不是你手动设置的」。仓库或场景不存在返回中文错误。

### 接入（`lib.rs`）
- 登记 `mod scenarios;`；新增 3 个命令并登记到 `generate_handler!` 和 `permissions/gsat-commands.toml`：
  - `list_scenarios() -> Vec<ScenarioSummary>`（内部先 `seed_scenarios` 再读，所以第一次打开就有场景；**不要**在这个命令里做归类）；
  - `reclassify_scenarios() -> ClassifyStats`（先 seed，再对全部有效仓库跑规则归类）；
  - `set_repository_scenario(repo_id, scenario_id, status)`。
- 数据库路径一律从 `AppStorage` 取。**不要**在启动流程里自动归类，也不要改 `prepare_knowledge`（那是旧命令）。

## 不要做
- 不改迁移、不改 `v2_ops.rs`、不改前端、不新增依赖。
- 不要读写真实的数据目录；测试只用临时库。

## 验收
1. `tools/cards/run.sh rust-test` 通过，`scenarios.rs` 新增测试**至少 18 个**，覆盖：种子幂等（连跑两次不重复）、种子更新（改关键词后重新 seed 会更新 seed 来源的行但不动 user 来源的行和已归档的行）；关键词匹配的两类规则（`rag` 不命中 `storage`、`seo` 不命中 `videoseoul`、`video-seo-tools` 命中、中文子串命中、`ppt` 在 `pptx-tools` 里命中）；多场景归类与置信度公式（1 个词 0.5、3 个词 0.7、封顶 0.8）；文字来源（只在理解卡的能力名里出现的词也能命中、`card_json` 是坏 JSON 或空串时不报错）；陈旧建议清理（改了描述后重新归类会删掉不再命中的规则行）；受保护的行（`confirmed`、`rejected`、`origin='ai'`、`origin='user'` 的行重新归类后原样不变）；`apply_ai_scenarios` 的替换语义和跳过规则；`list_scenarios_summary` 的排序、计数（不含已移除的仓库、不含被拒绝的归类）、`thin` 标志、不含已归档场景；`set_repository_scenario` 的各个转换和错误；整批归类在事务里（造一个会失败的情形验证没有留下一半）。
2. 契约测试：把 `ScenarioSummary` 序列化成 JSON，断言**键集合恰好**是 `id`、`slug`、`nameZh`、`sortOrder`、`count`、`thin`，类型分别是字符串、字符串、字符串、数字、数字、布尔。
3. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多（4 条）。
4. 给出 3 个命令在 `generate_handler!` 和 `gsat-commands.toml` 里的行号。
5. 用一个有 6–8 个仓库的夹具库跑一遍 `classify_repositories`，把每个仓库归到哪些场景（含置信度）整理成表贴进报告，方便总控人工核对规则合不合理。
6. `git diff --stat` 里只出现「允许改动」里的路径。

## 停止条件
- 要改迁移或卡外文件、要新依赖、种子里的关键词和本卡规则冲突——停手并写进报告。
