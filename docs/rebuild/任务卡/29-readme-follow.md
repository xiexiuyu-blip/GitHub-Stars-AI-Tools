---
编号: 29-readme-follow
允许改动:
  - packages/storage/migrations/fox/035_readme_basis.sql
  - apps/desktop/src-tauri/src/db/migrations.rs
  - apps/desktop/src-tauri/src/readme_follow.rs
  - apps/desktop/src-tauri/src/lib.rs
  - apps/desktop/src-tauri/src/card_handler.rs
  - apps/desktop/src-tauri/src/card_pipeline.rs
  - apps/desktop/src-tauri/src/storage.rs
  - apps/desktop/src-tauri/src/jobs.rs
  - apps/desktop/src/lib/api.ts
  - apps/desktop/src/lib/automation-view.ts
  - apps/desktop/src/lib/automation-view.test.ts
  - apps/desktop/src/pages/v2/toolbox.tsx
---

# 卡 29 · README 变了，理解卡自动重新整理

## 为什么有这张卡
Fox（2026-10-05）：「README 变了自动重新整理」。现状有两个洞：
1. **新收藏的项目是在没有 README 的情况下整理的。** 点「同步」后 `sync_github_stars_worker`（`lib.rs` 约 3919 行）立刻 `enqueue_new_stars`，后台队列马上把它们整理成理解卡，但 README 要另外手动点「抓取 README」才有（`fetch_repository_readmes`，只在 `only_missing` 时跳过已缓存的）。没有 README 的卡只能靠项目简介，质量差、把握度低。
2. **README 之后改了，卡永远不更新。** 增量整理只管「没有卡」的仓库（`CardEnqueueScope::Missing`）和老结构的卡（`Outdated`）。

## Fox 能看到的效果
- 点顶部「同步」：新收藏的项目**先抓 README，再排队整理**（整理出来的卡有 README 依据）；已收藏、最近有更新（GitHub 上的 `pushed_at` 比我们缓存 README 的时间新）的项目，顺手把 README 也重新抓一遍；**README 有明显改动**的，自动排队重新整理（走原来的后台队列，受每天额度管，不另花钱）。
- 只是徽章、版本号、错别字、星数这类小改动**不触发**重新整理（省 token）。
- Fox 点过「说得对」（`review_status = confirmed`）的卡**不会被覆盖**。
- 工具箱页「AI 自动整理」那一块多一行小字，例如：「README 更新检查：昨天 18:20 · 看了 23 个 · 有明显改动 3 个，已排队重新整理」；从没检查过就不显示这一行。

## 背景与先读什么
- `apps/desktop/src-tauri/src/card_pipeline.rs`：`PROMPT_VERSION`（`card-v2`，**不许改**）、`CardInput` / `load_card_input`（约 262 行：读 README、`build_readme_excerpt`）、`source_material`（算 `source_hash`，**不许改**，改了所有卡都会变「过期」）、保存卡的 `INSERT INTO repo_ai_documents … ON CONFLICT`（约 770 行）、`is_confirmed_doc`、`read_existing`。
- `apps/desktop/src-tauri/src/card_handler.rs`：`CardEnqueueScope`、`insert_scoped`、`enqueue_new_stars`、`enqueue_cards_for_scope`、`NOT_CONFIRMED_SQL`、优先级常量（`NEW_STAR_PRIORITY = 3`、`REGENERATE_PRIORITY = 2`、`BACKFILL_PRIORITY = 6`，数字小的先跑）、`AutomationStatus`。
- `apps/desktop/src-tauri/src/lib.rs`：`sync_github_stars_worker`（同步，约 3919 行）、`fetch_repository_readmes_worker`（手动抓 README，约 4143 行，含并发抓取循环、`README_FETCH_CONCURRENCY`、`partition_readme_fetch_targets`、`emit_task_progress`）。`github::fetch_readme` 返回带 `content_hash`（sha256）的 `ReadmeDocument`；`storage.save_readme`（`storage.rs` 约 979 行）整行 upsert。
- `apps/desktop/src-tauri/src/jobs.rs`：`QueueControl` / `LastFullScan`（控制文件 `fox-stars-lab.queue.json` 里的 `lastFullScan`，读写方式照抄它）。
- `apps/desktop/src/lib/automation-view.ts`（`lastFullScanText` 是同类函数）、`pages/v2/toolbox.tsx`（显示 `lastFullScanText` 的位置）。
- `packages/storage/migrations/fox/034_inbox_repos.sql`（迁移文件写法）、`db/migrations.rs`（`MIGRATIONS` 登记、断言「最后一个是 034」的测试要改成 035）。
- 表：`repo_readmes(repo_id, raw_markdown, content_hash, source_path, fetched_at)`；`repo_ai_documents`（理解卡，`card_json`、`generated_at`、`source_hash` 等）；`repositories.pushed_at`。

## 要做的事

### 1. 迁移 035（只追加）
`packages/storage/migrations/fox/035_readme_basis.sql`，在 `db/migrations.rs` 的 `MIGRATIONS` 登记（版本 `035`，名称 `readme_basis`）：
```sql
ALTER TABLE repo_ai_documents ADD COLUMN readme_hash TEXT;   -- 最近一次「评估过」的 README 的 content_hash（生成这张卡时读到的，或之后评估为「改动不明显」的）；NULL = 生成这张卡时没有 README / 还不知道
ALTER TABLE repo_ai_documents ADD COLUMN readme_basis TEXT;  -- 生成这张卡时喂给模型的 README 摘录（build_readme_excerpt 的 text）；NULL = 没有 README 或旧卡还没记
-- 旧卡回填：README 是在这张卡生成之前抓的（fetched_at <= generated_at），就认为这张卡读的就是它
UPDATE repo_ai_documents SET readme_hash = (
  SELECT content_hash FROM repo_readmes
  WHERE repo_readmes.repo_id = repo_ai_documents.repo_id AND repo_readmes.fetched_at <= repo_ai_documents.generated_at
) WHERE IFNULL(trim(card_json), '') <> '';
INSERT INTO schema_migrations(version, name) VALUES('035', 'readme_basis');
```
（`fetched_at` / `generated_at` 都是 ISO-8601 UTC 字符串，按字符串比较即可。）`readme_basis` 回填不了（SQL 里没法算摘录），留 NULL，由第 3 步惰性补。

### 2. 保存理解卡时记下 README 依据（`card_pipeline.rs`）
- `CardInput` 加 `readme_hash: Option<String>`（`load_card_input` 里和 `raw_markdown` 一起读 `content_hash`）。**不要**把它放进 `source_material`（不影响 `source_hash`）。
- 保存卡的 `INSERT … ON CONFLICT` 同时写 `readme_hash`（有 README 才有）和 `readme_basis`（`input.readme` 的摘录文本，没有就 NULL）。
- 已有测试要继续全过（该补 `CardInput` 构造处的新字段就补）。

### 3. 新模块 `readme_follow.rs`（在 `lib.rs` 里 `mod readme_follow;`）
纯函数（好测）：
- `pub fn normalize_for_compare(text: &str) -> String`：转小写；`![..](..)` 图片 / 徽章整个删掉；`[文字](地址)` 只留文字；裸网址（`http://`、`https://` 开头到空白）删掉；HTML 标签 `<…>` 删掉；数字删掉；只留字母和汉字（`char::is_alphabetic`），空白和标点全删；最多取前 20000 个字符。
- `pub fn similarity(a: &str, b: &str) -> f64`：两个**已规范化**的字符串各取所有连续 4 个字符的片段（不足 4 个字符的整串当 1 个片段），算 Jaccard（交集 / 并集）；两边都空是 `1.0`，一边空另一边不空是 `0.0`。
- `pub const MATERIAL_SIMILARITY: f64 = 0.80;`
- `pub fn is_material_change(basis: Option<&str>, current_excerpt: Option<&str>) -> bool`：`current_excerpt` 为 `None` 或规范化后为空（README 没了或几乎没内容）→ `false`（不重新整理）；`basis` 为 `None`/规范化后为空而 `current_excerpt` 有内容（以前没 README，现在有了）→ `true`；其余按 `similarity(规范化(basis), 规范化(current)) < MATERIAL_SIMILARITY`。
- `pub fn refresh_targets(repos: &[RepoReadmeState]) -> Vec<String>`：`RepoReadmeState { id, full_name, pushed_at: Option<String>, readme_fetched_at: Option<String> }`。需要重新抓的：没有缓存 README 的；或 `pushed_at` 的前 19 个字符（`YYYY-MM-DDTHH:MM:SS`）大于 `readme_fetched_at` 的前 19 个字符的；`pushed_at` 为 `None` 的跳过（有缓存的话）。按「没有 README 的在前，其余按 `readme_fetched_at` 从旧到新」排序，**最多 150 个**（常量 `MAX_REFRESH_PER_RUN`），超过的留给下次。
数据库相关：
- `pub fn enqueue_changed(db_path: &Path) -> Result<FollowReport, String>`（只读缓存里的 README，**不联网**）：对每个有效仓库（`sync_status = 'active'`）、有理解卡（`card_json` 非空）、**没被确认**（用 `NOT_CONFIRMED_SQL`）、有 README 缓存的：
  先分三种状态（`D` = `repo_ai_documents` 那一行，`R` = `repo_readmes` 那一行）：
  1. `D.readme_hash IS NULL`：这张卡生成时没有 README，现在 README 有了 → **算明显改动**，排队（`is_material_change(None, 当前摘录)` 本来就是 `true`）。
  2. `D.readme_hash = R.content_hash`：评估过了，跳过；但如果 `D.readme_basis IS NULL`（回填的旧卡）→ 用 `build_readme_excerpt` 算出当前摘录写进 `readme_basis`（只补这一列，不排队）。
  3. `D.readme_hash` 有值但和 `R.content_hash` 不同（README 变了）：
     - `D.readme_basis IS NULL`（旧卡，不知道它读的是哪一版）→ **没法比，当作不明显**：把 `readme_hash` 更新成当前 `content_hash`、`readme_basis` 写成当前摘录，从现在开始记；不排队。
     - 否则用 `build_readme_excerpt` 算当前摘录，`is_material_change(readme_basis, 当前摘录)`：
       - 明显改动 → 排队 `repo_card`（`INSERT OR IGNORE INTO ai_jobs …`，写法照 `insert_scoped`，优先级常量 `README_CHANGED_PRIORITY = 5`）；**不要**改 `readme_hash` / `readme_basis`（卡重做成功后由第 2 步的保存逻辑写新值）。
       - 不明显 → 只把 `readme_hash` 更新成当前的 `content_hash`（`readme_basis` 不动：和生成卡那一刻的 README 比，小改动累积到够多时仍会触发）。
  全程用 `IMMEDIATE` 事务写；每个仓库一个小事务，别一个大事务锁住整库。
- 需要 `card_handler::NOT_CONFIRMED_SQL`：把它改成 `pub(crate) const`（`card_handler.rs` 在允许改动里），别复制一份。
- `FollowReport { checked: usize, changed: usize, material: usize, queued: usize }`（`checked` = 评估了几个；`changed` = hash 变了的；`material` = 明显改动；`queued` = 真正新排进队列的）。
- `enqueue_cards_for_scope` 里加一个范围 `"readme_changed"` → 调 `enqueue_changed`（返回 `queued`），错误文案里「只能是 missing 或 outdated」同步改成含 `readme_changed`。（`insert_scoped` 的 `CardEnqueueScope` 枚举**不用**加。）

### 4. 同步和手动抓 README 里接上（`lib.rs`、`storage.rs`）
- `storage.rs` 新增 `touch_readme_fetched(repo_id, fetched_at)`：`UPDATE repo_readmes SET fetched_at = ? WHERE repo_id = ?`（README 没变时只更新「最近检查时间」，避免 `pushed_at` 一直比 `fetched_at` 新、每次同步都重抓）。再加一个读 `RepoReadmeState` 列表的方法（有效仓库 LEFT JOIN `repo_readmes`），用现有 `rusqlite` 或本文件已有的查询方式都行，跟着周围代码。
- `sync_github_stars_worker`：`upsert_repositories` 之后、`enqueue_new_stars` **之前**，插入「README 跟进」：
  1. `refresh_targets(...)` 选出要抓的仓库（新增的仓库没有 README 缓存，自然在里面）；
  2. 用和 `fetch_repository_readmes_worker` 同样的并发方式（`README_FETCH_CONCURRENCY` 个线程一批）调 `github::fetch_readme`；抓到：`content_hash` 和缓存的一样就 `touch_readme_fetched`，不一样就 `save_readme`；抓不到（`Ok(None)`，仓库没有 README）或出错：记数、不让同步失败；
  3. 抓完再 `enqueue_new_stars`（原有逻辑不变），然后 `readme_follow::enqueue_changed`；
  4. 进度用新的任务 id：`emit_task_progress(... "readme-follow", "readme", ...)`，开始 / 进行中 / 结束三种都要有（结束例如「README 跟进：看了 23 个，更新了 8 个，有明显改动 3 个，已排队重新整理」）；**原来的「同步完成：当前 … 个，新增 … 个，移除 … 个」那条消息一个字都不改**，`StarSyncSummary` 的字段也不改。
  5. 整个跟进步骤里任何错误只 `eprintln!`，同步结果照常返回。
- `fetch_repository_readmes_worker`（手动抓）：抓完后同样调一次 `readme_follow::enqueue_changed`（不联网的评估），错误只 `eprintln!`；其余行为不变。
- 把每次跟进的结果写进控制文件（见第 5 步）。

### 5. 控制文件里记「上次检查」（`jobs.rs`、`card_handler.rs`）
- `jobs.rs`：`LastReadmeCheck { checked_at: String, checked: i64, refreshed: i64, material: i64, queued: i64 }`（`#[serde(rename_all = "camelCase")]`），放进 `QueueControl` 的 `last_readme_check: Option<LastReadmeCheck>`，读写和 `lastFullScan` 一样（键名 `lastReadmeCheck`；读不懂按没有；**写的时候必须保留控制文件里别的键**，现有 `set_daily_budget` 的写法是模板）。提供 `pub fn record_readme_check(path: &Path, check: LastReadmeCheck) -> Result<(), String>`。`refreshed` = 这次真的重新抓了几个 README（`readme_follow` 前面第 4 步算出来）。
- `card_handler::AutomationStatus` 加 `last_readme_check: Option<jobs::LastReadmeCheck>`，`get_ai_automation` 的所有构造处补上。

### 6. 前端
- `lib/api.ts`（或 `automation-view.ts` 里放类型的地方，跟 `LastFullScan` 一样）：`LastReadmeCheck` 类型，`AutomationStatus.lastReadmeCheck?: LastReadmeCheck | null`。
- `lib/automation-view.ts`：`readmeCheckText(last, now = new Date())`：`null` / `undefined` → `null`；否则 `README 更新检查：{相对时间} · 看了 {checked} 个 · ` + （`material > 0` 时 `有明显改动 {material} 个，已排队重新整理`；否则 `没有明显改动`）。相对时间：不到 1 小时「刚刚」、当天「今天 HH:MM」、前一天「昨天 HH:MM」、更早「M月D日」（本地时间；格式化函数请先看 `automation-view.ts` / `lib/format.ts` 里有没有现成的，有就复用）。
- `pages/v2/toolbox.tsx`：在显示 `lastFullScanText` 的那一块附近显示这一行（小字、次要颜色，样式照抄旁边那行），`null` 时不渲染。
- `automation-view.test.ts` 新增至少 5 个测试（`null`、有明显改动、没有明显改动、昨天 / 更早的相对时间、`checked` 为 0）。

## 不要做
- 不改 `PROMPT_VERSION`、提示词、`source_material`、`ai-card` schema；不动场景 / 分类的逻辑。
- 不新增 Tauri 命令（所以不需要动 `permissions/`、`acl-manifests.json`、`capabilities/`）；不新增依赖。
- 不动额度 / 预算 / `spend_guard` / 队列认领；重新整理的任务和别的任务一样受每天额度管。
- 不改 `github::fetch_readme` 的联网方式（不要加 ETag 之类）；不改 `StarSyncSummary`；不改同步完成那条消息。
- 不读正式库；测试里不联网（README 用假数据、`refresh_targets` 是纯函数）。
- 不要为了让评估「更准」引入分词 / 向量 / 新算法：就用上面的规范化 + 4 字片段 Jaccard。

## 验收
1. `tools/cards/run.sh rust-test` 全部通过；新增 Rust 测试**至少 24 个**，覆盖：
   - `normalize_for_compare`：徽章 / 链接 / 网址 / HTML / 数字 / 标点 / 大小写 / 空白被去掉；汉字保留；超长截断到 20000。
   - `similarity`：相同 = 1.0；完全不同 ≈ 0；两边空 = 1.0；一边空 = 0.0；对称。
   - `is_material_change`：只改版本号、徽章、一个错别字 → `false`；追加约 30% 的新内容 → `true`；追加约 10% → `false`；把一大半内容改写 → `true`；`basis=None` 而现在有内容 → `true`；现在为空 → `false`；中文长文改一句 → `false`。
   - `refresh_targets`：没有缓存的在最前；`pushed_at` 比 `fetched_at` 新的入选；一样新 / 更旧的不选；`pushed_at` 为 None 的有缓存不选；按 `fetched_at` 从旧到新；超过 150 个截断。
   - `enqueue_changed`（临时库，用 `db::migrations::migrate_database` 建库，造仓库 / README / 卡）：hash 一样不排队；hash 变了但改动不明显只更新 `readme_hash`、不排队；明显改动排队且优先级是 5；已确认的卡不排队也不更新；没有卡的仓库不管；`removed` 的仓库不管；已经有排队 / 进行中的同一个仓库任务时不重复（`INSERT OR IGNORE`）；`readme_hash = content_hash` 而 `readme_basis IS NULL` 的旧卡被补上基准、不排队；`readme_hash` 有值、和 `content_hash` 不同、`readme_basis IS NULL`（旧卡，README 后来变过）→ 不排队，`readme_hash` 和 `readme_basis` 都被更新成当前的；`readme_hash IS NULL` 且有 README（卡生成时没有 README）→ 排队；返回的 `FollowReport` 各项对得上；跑两遍第二遍 `queued = 0`。
   - 迁移 035：从 034 的库升上来；回填：README 早于卡 → `readme_hash` 有值，README 晚于卡 → NULL，没卡 → 不动；`latest_migration()` 是 `035`；已有的迁移测试里「最后是 034」改成 035。
   - 卡保存：成功生成卡后 `repo_ai_documents.readme_hash` / `readme_basis` 有值（有 README）或 NULL（没有）；已确认的卡仍不被覆盖。
   - 控制文件：`record_readme_check` 写入后读得回来；**保留别的键**（`dailyBudgetTokens`、`paused`、`fullScan`、`lastFullScan`）；读不懂时是 `None`；`AutomationStatus` 带出来。
2. `tools/cards/run.sh web-test` 全部通过，新增至少 5 个测试；`web-build`、`rust-fmt`、`rust-check` 通过，警告数不比卡开始前多。
3. 现有测试**一个断言都不许改**，除非是「最后一个迁移是 034」这类必然要跟着变的，或 `CardInput` / `AutomationStatus` 构造处补新字段。
4. `git diff --stat` 里只出现「允许改动」里的文件。
5. 报告里写：同步里新增的那一段代码位置（文件:行）；`enqueue_changed` 的 SQL 全文；各项阈值和常量（`MATERIAL_SIMILARITY`、`MAX_REFRESH_PER_RUN`、`README_CHANGED_PRIORITY`）；你没把握的地方。

## 停止条件
- 需要新的 Tauri 命令 / 权限、要改 `PROMPT_VERSION` 或 `source_material`、要改额度逻辑、要新依赖——停手并写进报告。
- 发现 `storage.rs`（命令行 `sqlite3` 子进程）和 `rusqlite` 在同一个事务里混用会死锁或丢写——停手并写进报告，别硬改架构。
