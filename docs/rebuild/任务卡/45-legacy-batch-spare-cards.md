---
编号: 45-legacy-batch-spare-cards
允许改动:
  - apps/desktop/src-tauri/src/storage.rs
  - apps/desktop/src-tauri/src/lib.rs
---

# 卡 45 · 旧版「写中文说明」不再碰已经有新版说明（理解卡）的收藏（后端，改 storage.rs 和 lib.rs）

> 状态：第 1 轮已交回并审核（23:38，生产代码通过、8 种改坏全部抓住；3 个新测试共用临时库名会偶发撞车），第 2 轮只修测试夹具，进行中（2026-10-05 夜，事故修复卡）｜执行者：Codex｜只改 `storage.rs`、`lib.rs`。**不动前端、不改提示词、不升 `PROMPT_VERSION`、不加迁移。**

## Fox 能看到的效果
不管点哪个按钮（资料库页的「增量 AI」「抓取并分析」、顶栏快捷菜单里的「给没有说明的收藏写中文说明」「下载项目说明」、详情页的「生成中文说明」），**已经有新版说明的收藏都不会再被旧方法重写，也不会再为它们花 token**。现在这些按钮点下去只会处理「还没有任何说明」的收藏（比如新点星的）；全部都有新版说明时，只提示「中文说明已经是最新的，不用再整理」。

## 为什么有这张卡（事故经过，请先读）
2026-10-05 22:31 起，开发版里有一个操作触发了旧版批量「写中文说明」（`batch_generate_repository_ai_documents`）。它用 `list_repositories_requiring_ai_document` 找「还要整理」的收藏，条件是 `ai.source_hash != rr.content_hash`（文档的来源哈希 ≠ README 内容哈希）。

- 旧版的 `source_hash` 存的是 **README 内容哈希**；新版理解卡（`card_pipeline.rs`）的 `source_hash` 存的是**理解卡输入的哈希**（`source_hash_of`，测试 `readme_hash_is_not_part_of_source_hash` 还明确规定 README 哈希不参与）。两套口径永远对不上，所以 **500 张新版说明全被当成「过期」**。
- 它逐张调 AI（约 25 秒一张），再用 `save_repository_ai_document` 的 `INSERT … ON CONFLICT(repo_id) DO UPDATE` 把旧版字段（`summary_zh`、`readme_zh`、`keywords_json`、`suggested_tags_json`、`model`、`prompt_version`、`source_hash`、`input_tokens`、`output_tokens`、`generated_at`）写回去。`card_json`、`one_liner_zh` 等卡的列没被碰，但 `prompt_version` 被改回 `readme-summary-v1`、关键词和建议标签被换成旧版的。
- 被每天额度挡住之前，已经重写了 106 张，花了约 56 万 token。总控已用快照把这 106 张还原，开发库现在是对的；**这张卡要堵的是同样的事再发生**。

## 背景与先读什么
- `apps/desktop/src-tauri/src/storage.rs`：
  - `list_repositories_requiring_ai_document`（约 928 行）：候选查询，条件在 `WHERE` 里。
  - `get_ai_document_source_hash`（约 970 行）。
  - `save_repository_ai_document`（约 3425 行）：`INSERT … ON CONFLICT(repo_id) DO UPDATE`。
  - 既有测试 `ai_document_candidates_only_include_missing_or_changed_sources`（约 5680 行）：必须仍然通过，**不改、不删、不放宽**。
  - 既有测试里调用 `save_repository_ai_document` 的几处（约 5853、7511、7527 行）：改完仍要通过。
- `apps/desktop/src-tauri/src/lib.rs`：
  - `generate_repository_ai_document_worker`（约 5287 行）：详情页「生成中文说明」，单个收藏，先补抓 README、再调 AI、最后 `save_repository_ai_document`。
  - `batch_generate_repository_ai_documents_worker`（约 5479 行）：批量，里面有两处用 `get_ai_document_source_hash` 判断「已是最新则跳过」。
- 「有新版说明」的判定：`repo_ai_documents.card_json` 非空（`IS NOT NULL` 且不是空字符串）。新版流水线每写一张都会带上 `card_json`；1.4.0 留下的旧文档没有这一列的值（`NULL`）。**只用这一个判据**，不要去比 `prompt_version` 字符串（以后版本号会变）。

## 要做的事

### 1. 候选查询：有新版说明的不是候选
`list_repositories_requiring_ai_document` 的条件改成：没有文档 → 是候选；**有文档但 `card_json` 为空** → 按原来的规则（README 缺失、或 `ai.source_hash != rr.content_hash`）；**有文档且 `card_json` 非空 → 永远不是候选**（不管 README 缺不缺、哈希对不对得上）。排序仍是 `starred_at DESC`，其余不变。

### 2. 新增一个只读判断，给两处 worker 用
在 `storage.rs` 加一个小方法（名字自定，例如 `has_card_document(repo_id) -> Result<bool, String>`）：这个收藏的文档存在且 `card_json` 非空时返回 `true`，其余（没有文档、`card_json` 为 `NULL` 或空字符串）返回 `false`。

### 3. 批量 worker：card 收藏一律跳过
`batch_generate_repository_ai_documents_worker` 里，每个收藏处理一开始（在读 README 来源、补抓 README、调 AI **之前**），如果 `has_card_document` 为真，直接算 `Skipped`。这一条对 `onlyMissing` 的两种取值、对传了 `repositoryIds` 的重试都生效（候选查询只管 `onlyMissing = true` 的那条路，这里要把另外两条路也堵上）。

### 4. 单个 worker：先拒绝，不花钱
`generate_repository_ai_document_worker` 一开始（在补抓 README、调 AI 之前），如果 `has_card_document` 为真，直接返回 `Err`，文字写成人话，例如：`这个收藏已经有新版说明了，旧的「生成中文说明」不会再覆盖它，也不会花 token。（以后会提供「重新整理这一张」。）` 不要发任何 AI 请求、不要抓 README、不要写库。

### 5. 最后一道保险：`save_repository_ai_document` 不覆盖 card 文档
把 `ON CONFLICT(repo_id) DO UPDATE SET …` 加上 `WHERE repo_ai_documents.card_json IS NULL OR repo_ai_documents.card_json = ''`，这样即使上面两处检查漏了、或者检查和写入之间刚好有一张理解卡写进来了，旧字段也写不进有 card 的文档。函数签名和返回值不变（没写进去不算错误；调用方在第 3、4 条里已经先挡掉了）。

## 测试（每一条都要能「改坏就失败」）
1. **候选**（`storage.rs` 里新增一个测试，夹具参考既有候选测试）：同一个账号下放这几种收藏，断言候选的 id 和顺序：
   - A：有文档、`card_json` 非空、`source_hash` ≠ README 哈希 → **不是**候选；
   - B：有文档、`card_json` 非空、**没有 README 记录** → **不是**候选；
   - C：有文档、`card_json` 为 `NULL`、哈希对不上 → 是候选；
   - D：有文档、`card_json` 是空字符串 `''`、哈希对不上 → 是候选（当作没有新版说明）；
   - E：完全没有文档 → 是候选；
   - F：有文档、`card_json` 为 `NULL`、哈希对得上 → 不是候选（原规则不变）。
2. **保险**（`storage.rs`）：先写一张 card 文档（`card_json = '{"x":1}'`、`prompt_version = 'card-v3'`、`summary_zh`、`keywords_json`、`source_hash`、`generated_at` 各填一个已知值），再用不同的内容调用 `save_repository_ai_document`。断言：返回 `Ok`，且 `summary_zh`、`readme_zh`、`keywords_json`、`suggested_tags_json`、`model`、`prompt_version`、`source_hash`、`input_tokens`、`output_tokens`、`generated_at`、`card_json` **全部和写入前逐列相同**。再放一张 `card_json` 为 `NULL` 的旧文档，同样调用，断言它被正常覆盖（旧行为没变）。
3. **判断方法**（`storage.rs`）：对应 A（card）、C（`NULL`）、D（空字符串）、E（没有文档）四种，分别断言 `true / false / false / false`。
4. **两处 worker**：`lib.rs` 里的 worker 需要 `AppHandle` 和网络，不用硬做单元测试；但**不要把判断逻辑写死在 worker 里**，只在那里调用上面第 2 条的方法（这样第 3 条的测试就是它们的保障）。在报告里写清楚两处调用的文件:行，以及它们都在任何网络请求、任何写库之前。
5. 报告里写明：把第 1 条的 `card_json` 条件拿掉 / 把保险里的 `WHERE` 拿掉 / 把判断方法里「非空字符串」改成只判 `IS NOT NULL`，分别会让哪几条测试失败。

## 报告里还要列出
全仓库里所有**非测试**代码里写 `repo_ai_documents` 的地方（`INSERT` / `UPDATE` / `REPLACE`），逐个说明：它会不会改写一张已有 `card_json` 的文档（会 / 不会 / 只改某几列），以及依据。**只列，不改**；有疑似会覆盖整张卡的，单独标出来由总控决定。

## 不要做
- 不改 `PROMPT_VERSION`、不改提示词、不改 `docs/rebuild/spec/ai_card.schema.json`。
- 不改 `card_pipeline.rs`、`card_handler.rs`、`jobs.rs`、`spend_guard.rs`、`readme_follow.rs`、`solve_rerank.rs` 以及任何前端文件。
- 不新增数据库迁移，不改表结构，不新增依赖。
- 不改 `source_hash` 的含义，也不去「统一」两套哈希。
- 不放宽、不删现有断言；某条既有测试必须改的，在报告里点名并说明理由。
- 不碰正式库 / 开发库的数据目录和钥匙串；测试只用临时目录；不发任何真实 AI 请求。

## 验收
1. `tools/cards/run.sh rust-fmt`、`rust-check`、`rust-test storage`、`rust-test lib` 通过（沙盒里跑不了的写进报告，总控在沙盒外跑完整套）。
2. `git diff --stat` 只出现 `apps/desktop/src-tauri/src/storage.rs` 和 `apps/desktop/src-tauri/src/lib.rs`。
3. 报告里写：条号 → 文件:行；新增的测试名；上面「改坏对照」；非测试写入点清单；没把握的地方。

## 停止条件
- 需要改「允许改动」以外的文件，或必须改表结构 / 迁移才能做完：停手，写进报告。
- 发现还有别的旧功能路径会改写 card 文档、但不在这两个文件里：不要自己去改，写进报告。
