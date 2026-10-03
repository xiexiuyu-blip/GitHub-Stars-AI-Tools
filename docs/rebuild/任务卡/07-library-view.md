---
编号: 07-library-view
允许改动:
  - apps/desktop/src-tauri/src/library_view.rs
  - apps/desktop/src-tauri/src/lib.rs
  - apps/desktop/src-tauri/permissions/gsat-commands.toml
  - apps/desktop/src-tauri/gen/schemas/acl-manifests.json
---

# 卡 07 · 资料库与今日的查询（后端）

## Fox 能看到的效果
这是后台积木，**这张卡 Fox 暂时看不到界面变化**。做完后，新的「资料库」页和「今日」页有了专用的数据来源：资料库能按**业务场景、我的状态、是否已整理、形态、关键词**筛选并分页，左侧每个筛选项旁边的数字随其他条件联动；点开一个项目能拿到「先结论后原文」的完整详情；今日页能拿到「今天值得先看的 3 个」和「本周新收藏」；还能批量设置「我的状态」。

## 背景与先读什么
- **契约（不许改名、不许改形状）**：`apps/desktop/src/lib/contracts.ts` 里的 `ReadingGroup`、`LibraryFilters`、`LibraryItem`、`LibraryCounts`、`LibraryPage`、`LibraryDetail`、`SetReadingRequest`、`TodayView`。命令返回的 JSON 必须逐字段按它走（serde 的 `camelCase`，`null` 就是 `null`）。
- **现有的粗糙实现（不要改，新模块并行存在）**：`apps/desktop/src-tauri/src/v2_ops.rs` 里的 `list_workspace`（约第 560 行起）、`workspace_detail`、`set_reading_statuses`、`reading_clause`、`reading_label`；`lib.rs` 里的 `list_workspace_cards` 等命令。读懂它们怎么连表，**照它的表结构写，但要修它的毛病**：它的关键词搜索只用 LIKE 扫三列、一句话回退到 `summary_zh`、没有场景、没有计数。
- **表**：`repositories`（`sync_status='active'` 才算有效；`full_name`、`description`、`language`、`topics_json`、`html_url`、`stars_count`、`forks_count`、`starred_at`、`pushed_at`、`updated_at`）；`annotations`（`repo_id`、`account_id`、`read_status`，值只有 `unread`/`read`/`later`/`want_to_try`/`tried`/`in_use`/`watching`/`deprecated`，**没有这一行就等于 `unread`**）；`repo_ai_documents`（`one_liner_zh`、`summary_zh`、`keywords_json`、`card_json`、`card_schema_version`）；`repo_readmes`（`raw_markdown`）；`scenarios` 和 `entity_scenarios`（迁移 030，字段含义见 `docs/rebuild/任务卡/06-scenarios.md`；本卡**只读**它们，测试里直接用 SQL 往里插行）。
- **`card_json`**（理解卡）格式见 `docs/rebuild/spec/ai_card.schema.json`：`form`、`capabilities[].name_zh`、`problems_solved`、`use_when`、`not_for`、`how_to_try`、`integrates_with`、`alternatives_hint`、`inspiration_for_fox`、`maturity`、`zh_friendly`、`license`、`risks`、`confidence`，以及外层的 `review_status`（`suggested`/`confirmed`）。**它可能是空串、坏 JSON、字段类型不对（比如 `how_to_try` 是数组、`capabilities` 不是数组）**，必须容忍：取不出就当没有，绝不让整个查询失败。SQL 里用 `json_extract` 前必须先 `json_valid(card_json)`，否则一行坏 JSON 会让整条查询报错。
- 测试怎么造库：看 `db/migrations.rs` 里 `empty_database_applies_every_migration`，再用 SQL 插 `github_accounts`、`repositories`、`annotations`、`repo_ai_documents`、`scenarios`、`entity_scenarios`。**所有测试只用临时库。**

## 要做的事

### 新模块 `src-tauri/src/library_view.rs`（不依赖 Tauri）
1. **状态组映射**：`unseen = {unread, later}`、`want = {want_to_try, watching}`、`tried = {tried, read}`、`in_use = {in_use}`、`dropped = {deprecated}`。写一个函数从原始状态得到组，另一个函数从组得到要查的状态集合；没有 `annotations` 行等于 `unread`。
2. **`list_library_view(db_path, filters: LibraryFilters) -> LibraryPage`**：
   - 只含有效仓库；筛选条件全是 AND：`scenario`（该场景下有一条 `status<>'rejected'` 的归类）、`reading`（状态组）、`cardState`（`done` = `one_liner_zh` 非空；`pending` = 为空或没有卡；`all` 不筛；**默认 `done`**）、`form`（理解卡的 `form`）、`keyword`（对 `full_name`、`description`、`one_liner_zh`、`summary_zh`、`keywords_json`、`topics_json` 做不区分大小写的包含匹配；`%`、`_` 要转义）。
   - 排序：`starred`（默认，`starred_at` 新到旧）、`stars`（星数多到少）、`updated`（`pushed_at` 新到旧，空的排最后）；并列时按 `id` 升序，保证分页稳定。`limit` 默认 80、夹在 1 到 200；`offset` 不小于 0。
   - 每个条目：`LibraryItem`（契约里的全部字段）。`scenarioIds` 取该仓库所有 `status<>'rejected'` 的场景 id，按场景 `sort_order` 排；`oneLiner` 只取 `one_liner_zh`，**不要回退到摘要**；`hasCard` 就是 `oneLiner` 非空；`confidence`、`form`、`cardReview` 来自 `card_json`（读不出为 `null`）。
   - **计数 `LibraryCounts`**：`total` = 有效仓库总数（不受筛选影响）；`scenarios`、`reading`、`cardState`、`forms` 四组各自按「**除它自己之外的所有其他筛选条件**」算（这样用户在场景里点了一个，状态组的数字随之联动，但场景那一列的数字不会因为自己被选中而变成只剩一项）。`scenarios` 里每个未归档场景都要有键（没有就是 0）；`reading` 五个组都要有键；`cardState` 有 `done` 和 `pending`。
3. **`get_library_detail(db_path, id) -> LibraryDetail`**：`LibraryItem` 的全部字段 + 契约里的详情字段。数组字段取不出就是空数组，字符串取不出就是 `null`；`keywords` 取 `keywords_json`；`topics` 取 `topics_json`；`readmeExcerpt`：README 去掉 HTML 注释和图片行后取前约 1500 个字符（**按字符不按字节，不能切坏中文**），没有 README 为 `null`。`id` 不存在返回中文错误「找不到这个仓库」。
4. **`set_library_reading(db_path, ids, group) -> usize`**：把一批仓库设成状态组，写入映射：`unseen→unread`、`want→want_to_try`、`tried→tried`、`in_use→in_use`、`dropped→deprecated`；用 UPSERT 写 `annotations`（没有就建，已有的只改 `read_status` 和 `updated_at`，**不要动 `note_md` 和 `rating`**）；一次最多 200 个，只处理有效仓库，返回实际改了几个；整批一个事务。
5. **`get_today_view(db_path) -> TodayView`**：
   - `picks`：从「状态组是 `unseen` 且 `hasCard`」的有效仓库里挑最多 3 个。挑法：每个候选的「方向排名」= 它所属场景里**最小的 `sort_order`**，但只有 `sort_order ≤ 7` 的场景算「核心方向」（小红书、公众号、抖音、海报封面、视频字幕语音、PPT 文档、写作文案），不属于任何核心方向的排名当作 99；先按方向排名升序，**同一个方向最多入选一个**（尽量挑出不同方向），不够 3 个再放宽，并列时按 `confidence` 降序、再按 `starred_at` 新到旧。
   - `weekNew`：近 7 天（`starred_at` 在 7 天内）收藏的有效仓库，不论有没有卡、不论状态，最多 8 个，新到旧。
   - `counts`：`total`、`unseen`（状态组 unseen 的有效仓库数）、`weekNew`（近 7 天数量，不受 8 个上限影响）、`cardsPending`（没有理解卡的有效仓库数）、`cardsSuggested`（有卡但 `review_status` 不是 `confirmed` 的数量）。
   - `syncedAt`：取 `MAX(repositories.updated_at)`，没有仓库为 `null`。
   - 「现在」由参数传入（测试要用固定时间）。

### 接入（`lib.rs`）
- 登记 `mod library_view;`；新增 4 个命令并登记到 `generate_handler!` 和 `permissions/gsat-commands.toml`：`list_library_view(filters)`、`get_library_detail(id)`、`set_library_reading(ids, group)`、`get_today_view()`。命令参数名按契约（`filters` 是对象；`group` 是字符串）。数据库路径一律从 `AppStorage` 取；只读命令用只读方式打开（可参考 `lib.rs` 里 `open_pack_database` 的写法，如果合并后存在）。

## 不要做
- 不改迁移、不改 `v2_ops.rs`、不改前端、不新增依赖。不写 `scenarios`/`entity_scenarios`（本卡只读它们）。
- 不要读写真实的数据目录；测试只用临时库。

## 验收
1. `tools/cards/run.sh rust-test` 通过，`library_view.rs` 新增测试**至少 24 个**，覆盖：状态组映射的全部 8 个原始值与「没有 annotations 行」；各个筛选单独和组合（场景、状态组、卡片状态、形态、关键词含 `%` 和 `_` 的转义、大小写不敏感）；`cardState` 默认是 `done`；三种排序和并列稳定、分页没有重复也没有遗漏；`oneLiner` 不回退到摘要；**坏 JSON、空串、字段类型不对的 `card_json` 不会让查询失败**，且这些行 `hasCard` 的判定正确；`LibraryCounts` 的联动语义（选了场景后，`scenarios` 计数不变而 `reading` 计数随之变；选了状态后反过来）、`total` 不受筛选影响、没有归类的场景也有键且为 0；`scenarioIds` 的排序和 `rejected` 不计入；详情的各字段回退（`how_to_try` 是数组时的处理、`capabilities` 不是数组时为空数组）和 `readmeExcerpt` 的截断（用全中文长文本验证不切坏字符）；`set_library_reading` 的映射、不动 `note_md`/`rating`、不存在的 id 和已移除的仓库被忽略、上限 200；`get_today_view` 的挑选（核心方向优先、同方向最多一个、不够再放宽、只含 unseen 且有卡的）、`weekNew` 的 7 天边界和 8 个上限、各个 `counts`、`syncedAt`。
2. **契约测试**：把 `LibraryItem`、`LibraryCounts`、`LibraryPage`、`LibraryDetail`、`TodayView` 各序列化一份，断言键集合和类型和 `contracts.ts` 一致（键名、`null` 与数组的位置都要对）。
3. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多（4 条）。
4. 给出 4 个命令在 `generate_handler!` 和 `gsat-commands.toml` 里的行号；证明 `v2_ops.rs` 没有改动。
5. 性能：用脚本在测试里造 3000 个仓库的临时库，`list_library_view` 的一次默认查询（含计数）在本机应当不超过 1 秒；把实测耗时写进报告。超过就优化查询（比如把 JSON 解析放到只对当前页的行做）。
6. `git diff --stat` 里只出现「允许改动」里的路径。

## 停止条件
- 要改迁移或卡外文件、要新依赖、契约里某个字段在库里取不到来源——停手并写进报告，不要自己改契约。
