---
编号: 49b-archived-flag
允许改动:
  - packages/storage/migrations/fox/038_repo_archived.sql
  - apps/desktop/src-tauri/src/db/migrations.rs
  - apps/desktop/src-tauri/src/github.rs
  - apps/desktop/src-tauri/src/storage.rs
  - apps/desktop/src-tauri/src/library_view.rs
  - apps/desktop/src/lib/contracts.ts
  - apps/desktop/src/lib/maintenance.ts
  - apps/desktop/src/lib/maintenance.test.ts
  - apps/desktop/src/lib/library-filters.ts
  - apps/desktop/src/lib/library-filters.test.ts
  - apps/desktop/src/lib/api.test.ts
  - apps/desktop/src/components/library/maintenance-tag.tsx
  - apps/desktop/src/components/library/maintenance-tag.test.tsx
  - apps/desktop/src/components/library/filter-panel.tsx
  - apps/desktop/src/components/library/filter-panel.test.tsx
  - apps/desktop/src/components/library/item-row.tsx
  - apps/desktop/src/components/library/item-row.test.tsx
  - apps/desktop/src/components/library/item-detail.tsx
  - apps/desktop/src/components/library/item-detail.test.tsx
  - apps/desktop/src/pages/v2/library.tsx
---

# 卡 49b · 「已归档」：同步存下 GitHub 的 archived，资料库标签 + 筛选

> 执行者：Grok（沿用第 2 批分配单）｜**本卡有迁移（038）和同步改动**。不加依赖、不联网（所有测试用本地 JSON 字符串和临时库，不发任何真请求）、不花额度。**不含**「找方案命中停更工具时提示」：那要改 `solve.rs`，另开卡 49c。

## Fox 能看到的效果
1. 点一次「同步」之后（旧数据要同步一次才知道哪些项目归档了）：资料库里在 GitHub 上已归档的项目，列表行多一个红色「已归档」小标签；详情页「项目最近更新」那一行后面也有同样的红色小标签。
2. 资料库左栏「维护状态」从三项变四项：活跃 · 放缓 · 疑似停更 · 已归档，每项后面有个数；点「已归档」只看已归档的项目，再点取消。
3. **四类互斥，已归档压过按推送时间算的三档**：一个项目归档了，就只算「已归档」，不再算进活跃 / 放缓 / 疑似停更的个数和筛选结果（归档前一个月还有推送也一样，因为已经不会再更新了）。
4. 还没同步过的旧数据（这一列是空的）：不画标签、不算已归档，原来的三档照旧。所以升级后「已归档」一开始是 0 个，同步一次才有。
5. 鼠标悬停「已归档」标签有一句话说明；悬停左栏的「已归档」项，那句话后面多一句提醒要同步（见下）。

## 三态与写入规则（Rust 和测试必须一模一样）
- 新列 `repositories.archived`：`INTEGER`，**可空、无默认值、不加 CHECK**（照 `032_repo_license.sql`）。`NULL` = 不知道（旧行，或 GitHub 响应里没有这个键）；`0` = 没归档；`1` = 已归档。
- 同步写入：GitHub 的 `"archived": true` → `1`；`false` → `0`；键不存在或 `null` → `NULL`。
- 覆盖规则：每次同步 `archived = COALESCE(excluded.archived, repositories.archived)`——新值是 0 / 1 就覆盖（包括 1 → 0 取消归档），新值是 NULL（不知道）就**保留旧值，不要抹掉**。`upsert_repositories` 里有**两个** `ON CONFLICT … DO UPDATE` 分支（按 `id`、按 `account_id + full_name`），两个都要改。
- 读取：只有 `archived = 1` 才算已归档；`NULL` 和 `0` 一样，都不算。

悬停文案（放在 `maintenance.ts`，前后端各处用同一套）：
- 标签和筛选项共用：「作者已在 GitHub 上把它归档：只读，不会再更新」
- 仅左栏筛选项的悬停提示，后面多一句：「作者已在 GitHub 上把它归档：只读，不会再更新。旧数据点一次「同步」后才会标出来」（新增一个只覆盖 `archived` 的 `MAINTENANCE_FILTER_HINTS`，`maintenanceOptions` 的 `hint` 优先用它，没有就用 `MAINTENANCE_HINTS`）

## 背景与先读什么（以集成线最新代码为准，行号只作参考）
- **参照物：迁移 032 `repo_license`**——同样是给 `repositories` 加一个来自 GitHub Stars 的可空列、旧行 NULL、同步才补。照它的做法：`packages/storage/migrations/fox/032_repo_license.sql`；`db/migrations.rs` 里 032 的登记和它的升级测试（搜 `upgrade_from_032`、`032 repo_license`）；`github.rs` 的 `GitHubRepository`（私有反序列化结构）→ `StarredRepository` → `map_starred_repository`，以及测试 `parse_starred_repositories_reads_license_variants`；`storage.rs` 的 `upsert_repositories` 和它的测试（搜 `starred_for_license`）。
- `db/migrations.rs`：`MIGRATIONS` 末尾现在是 037；测试里有多处写死「最新版本 = 037」「已应用列表以 037 结尾」「`MIGRATIONS.len()` = 37」（搜 `"037"`、`ai_job_spend_authorization`、`MIGRATIONS.len(), 37`）。**把这些期望改成 038 / 加上 `"038 repo_archived"` / 38 是预期的，不算放宽断言。**
- `library_view.rs`（卡 49、卡 50 之后的最新样子）：`is_active! / is_slow! / is_stale!` 三个宏和 `IS_ACTIVE / IS_SLOW / IS_STALE` 常量；`FILTER_WHERE` 末尾的 `:maintenance` 段；`MaintenanceCounts` 和 `maintenance_counts`；`normalize` 里 `maintenance` 的合法值检查；`RawItem`、`read_raw`、`read_ranked`、`into_item`；`LibraryItem`；三处 SELECT：`ITEM_SELECT`（列表、今日的挑选和本周新收藏共用）、`ITEM_SELECT_RANK`（相关度排序）、`get_library_detail` 里的那条 SQL。`LibraryDetail` 用 `#[serde(flatten)]` 带着 `item`，所以详情会自动带上新字段。
- 前端：`lib/contracts.ts`（`MaintenanceTier`、`LibraryItem`、`LibraryCounts.maintenance`）；`lib/maintenance.ts`（`maintenanceOf`、`MAINTENANCE_TIERS / LABELS / HINTS`）；`components/library/maintenance-tag.tsx`（`TONE`）；`lib/library-filters.ts`（`maintenanceOptions`）；`item-row.tsx` 里 `<MaintenanceTag … showActive={false} />` 那一行；`item-detail.tsx` 里 `<MaintenanceTag … showActive />` 那一行；`filter-panel.tsx` 和 `pages/v2/library.tsx` 的维护状态接线是按行通用的，预计不用改（改了要在报告里说明为什么）。

## 要做的事

### 1. 迁移 038
1. 新建 `packages/storage/migrations/fox/038_repo_archived.sql`，照 032 的写法：
   ```sql
   -- 038: 仓库是否已在 GitHub 上归档。来自 GitHub Stars 的 archived，同步时写入。
   -- 一列可空、无默认值：NULL = 不知道，0 = 没归档，1 = 已归档。旧行保持 NULL，下次同步才补。
   ALTER TABLE repositories ADD COLUMN archived INTEGER;
   INSERT INTO schema_migrations(version, name) VALUES('038', 'repo_archived');
   ```
2. 在 `db/migrations.rs` 的 `MIGRATIONS` 末尾登记（版本 `"038"`、名称 `repo_archived`、`include_str!` 的写法和 037 一样）。**不要**用 `INSERT OR IGNORE` 写 `schema_migrations`，不要改动任何已有迁移文件。
3. 不要把 `archived` 加进 `storage.rs` 开头那几张「必需表字段」清单（032 加许可证时也没加）。

### 2. 同步：`github.rs`
1. `GitHubRepository` 新增 `#[serde(default)] archived: Option<bool>`（键不存在或 `null` 都是 `None`）。
2. `StarredRepository` 新增 `pub archived: Option<bool>`；`map_starred_repository` 原样带上。
3. 构造 `StarredRepository` 的地方都要补这个字段：`github.rs` 一处，`storage.rs` 测试里几处——编译器会逐个指出来，测试里补 `archived: None`。

### 3. 同步：`storage.rs` 的 `upsert_repositories`
1. `INSERT` 的列表和 `VALUES` 各加 `archived`。`Option<bool>` 写成 SQL 字面量要用一个小函数：`Some(true)` → `1`、`Some(false)` → `0`、`None` → `NULL`（不要拼 `"true"` / `"false"`）。
2. 两个 `ON CONFLICT … DO UPDATE SET` 分支都加 `archived = COALESCE(excluded.archived, repositories.archived)`。

### 4. 后端读取与筛选：`library_view.rs`
1. `LibraryItem` 新增 `pub archived: bool`（`NULL` / `0` → `false`，`1` → `true`）；`RawItem` 新增同名字段；`read_raw` 读，`into_item` 写进 `LibraryItem`。
2. 三处 SELECT 都要把 `r.archived` 加进去，建议统一放在 `a.read_status` 后面（第 12 列）；这样 `read_ranked` 里原来的 11–13 号列要跟着后挪，`get_library_detail` 里读 `forks_count` 等的序号也要跟着挪。**序号错位会让测试直接报类型错误，改完先跑相关测试。** 想用别的办法（比如单独查一遍归档的 id，像 `scenario_ids_for` 那样）也行，前提是列表、相关度排序、详情、今日挑选、本周新收藏五条路径都带上，并且各有测试。
3. `normalize`：`maintenance` 的合法值增加 `"archived"`（严格小写）；其它非法值仍然 `Err("维护状态筛选无效")`。
4. **SQL 四档互斥**：
   - 新增宏 `is_archived!()` = `IFNULL(r.archived, 0) = 1`，常量 `IS_ARCHIVED`。
   - `is_active! / is_slow! / is_stale!` 各自再加一句 `IFNULL(r.archived, 0) <> 1 AND …`（已归档的不再属于这三档）。注意写 `IFNULL(…, 0)`，不要写成 `r.archived <> 1`：后者遇到 NULL 会让「不知道」的仓库从三档里消失。
   - `FILTER_WHERE` 末尾的 `:maintenance` 段加一支 `OR (:maintenance = 'archived' AND …is_archived…)`。
   - **不新增命名参数**（已归档不用时间判断）。所以 `filter_params!` 宏和 `query_items` 手写的那份参数列表都不用动；如果你发现要动，说明接错了。
5. 计数：`MaintenanceCounts` 新增 `archived: i64`；`maintenance_counts` 多算一列 `SUM(CASE WHEN {IS_ARCHIVED} …)`，仍然是「自己不算自己」（用 `cleared_maintenance()`）。`total`、`total_count`、其它维度的计数规则**不变**。

### 5. 前端
1. `lib/contracts.ts`：`MaintenanceTier` 加 `'archived'`；`LibraryItem` 加 `archived?: boolean`（后端总会带；写成可选只是为了不用改一堆旧测试夹具，注释里写明）；`LibraryCounts.maintenance` 加 `archived?: number`（同样的理由，注释写清：已归档单独一档，压过按推送时间算的三档）。
2. `lib/maintenance.ts`：
   - `MAINTENANCE_TIERS` 末尾加 `'archived'`（固定顺序：活跃、放缓、疑似停更、已归档）；`MAINTENANCE_LABELS.archived = '已归档'`；`MAINTENANCE_HINTS.archived`、`MAINTENANCE_FILTER_HINTS`（见上面的悬停文案）。
   - `maintenanceOf(pushedAt, now, archived?)` 新增第三个可选参数：`archived === true` 时**直接返回 `'archived'`**，不看推送时间和 `now`（哪怕它们是空的、坏的）；`archived` 为 `false` / `undefined` 时，行为和现在完全一样。
3. `components/library/maintenance-tag.tsx`：`TONE.archived = 'red'`。`showActive` 为假（列表行）时只隐藏 `active`，已归档照常画。
4. `item-row.tsx`、`item-detail.tsx`：调用 `maintenanceOf` 时把归档标记传进去（行：`props.item.archived`；详情：`detail.archived`）。布局一概不动。详情页即使没有推送时间，已归档的标签也要画。
5. `lib/library-filters.ts`：`maintenanceOptions` 的 `hint` 优先用 `MAINTENANCE_FILTER_HINTS[tier]`，没有再用 `MAINTENANCE_HINTS[tier]`；顺序由 `MAINTENANCE_TIERS` 决定；注释里的「固定顺序」更新。其它逻辑（`toFilters` / `toggleMaintenance` / `clearFilters` / `hasActiveFilters` / `libraryEmptyNote`）本来就按档通用，确认一下不用改。

## 不要做
- 不做「找方案命中停更工具时提示」（要改 `solve.rs`，卡 49c）；不做字母评分；不做「已归档」的批量操作或自动处理。
- 不改 `solve*.rs`、`lib.rs`、`packs.rs`、`v2_ops.rs`、`library.rs`、`discover.rs`、`inbox*.rs`；不动 `github_recommendation_candidates` 表和它的写入（那是 GitHub 搜索推荐，不是 Stars）。
- 不改已有迁移 001–037 的任何一个字；不用 `INSERT OR IGNORE` 写 `schema_migrations`；不加 CHECK 约束、不加默认值。
- 不联网、不用真的 GitHub 响应、不调用任何 AI、不加依赖、不运行 `pnpm install`、不碰任何数据目录。
- 不改 `docs/`（总控自己更新文档）。
- 不删除、不放宽任何已有测试的断言（把期望里的 037 改成 038、给 JSON 键列表加 `archived` 除外）。

## 验收
1. `git diff --stat` 的文件都在「允许改动」里（总控会用 `check-scope.sh` 核对）。
2. `tools/cards/run.sh web-build`、`web-test`、`rust-fmt`、`rust-check`、`rust-test` 全部通过。**基线（集成线 5fba0a9，总控已测）：Rust 1438 通过 / 0 失败 / 5 忽略（总控完整跑，含沙盒里不跑的 1 个钥匙串用例；你在沙盒里用 `run.sh rust-test` 应看到 1437 通过 / 1 被过滤）；前端 41 个文件 891 项。** 你的结果 = 基线 + 你新增的测试数，已有测试一项不少。
3. 测试清单（每条一个测试，报告里写「第 N 条 → 测试名」）：

**迁移**（`db/migrations.rs`）
- M1 空库迁到最新：已应用列表含 `"038 repo_archived"`，`latest_migration().version` 是 `"038"`，`MIGRATIONS.len()` 是 38（改已有的期望）。
- M2 `upgrade_from_037_adds_repo_archived`（仿 `upgrade_from_032_…`）：先建一个只应用到 037 的库并插入一个仓库，再迁移到最新：该仓库 `archived IS NULL`，它原有的其它列一个字不变；`PRAGMA table_info(repositories)` 里有 `archived`，类型 `INTEGER`、可空、无默认；完整性检查通过；迁移前的备份文件产生（沿用现有升级测试的断言方式）。

**同步**（`github.rs`、`storage.rs`；全部用本地 JSON 字符串和临时库）
- G1 `parse_starred_repositories_reads_archived_variants`：`"archived": true` → `Some(true)`；`false` → `Some(false)`；键不存在 → `None`；`null` → `None`。
- S1 第一次同步 `Some(true)` / `Some(false)` / `None` 三个仓库：库里分别是 1 / 0 / NULL。
- S2 同一个仓库（同 `id`）再同步：`Some(false)` → `Some(true)`（新归档）变 1；`Some(true)` → `Some(false)`（取消归档）变 0。
- S3 再同步传 `None`：旧值保留（1 仍是 1，0 仍是 0，不被抹成 NULL）。
- S4 走第二个冲突分支（同 `account_id + full_name`、不同 `id`，比如仓库改了 id）：`archived` 也会更新，传 `None` 时保留旧值。
- S5 库里已有一行不带 `archived` 的旧仓库（NULL），同步带 `Some(true)` 后是 1。

**后端**（`library_view.rs`；`now` 用固定时间，每个仓库一行，用 `list_library_view_at`）
- L1 序列化：列表里每个 `LibraryItem` 的 JSON 有 `archived`（布尔）；`counts.maintenance` 的键 = `active, slow, stale, archived`，都是数字；`LibraryDetail` 的 JSON 有 `archived`；今日页的行（挑选、本周新收藏）有 `archived`；相关度排序那条路径的行也有。更新既有的契约测试（加新键）。
- L2 优先级：一个 `pushed_at` 是昨天（本来是活跃）但 `archived = 1` 的仓库，只在「已归档」里，不在「活跃」里；`pushed_at` 是五年前（本来是疑似停更）且归档的，只在「已归档」里；`pushed_at` 为 NULL 且归档的，也在「已归档」里。
- L3 NULL 与 0：`archived` 是 NULL 的和是 0 的仓库，按推送时间归入原来的三档，不属于已归档。
- L4 筛选：`maintenance = "archived"` 恰好得到已归档的仓库；`active / slow / stale` 的结果里没有已归档的；不带 `maintenance` 时全部出现（含已归档、含没有状态的）。
- L5 计数：四档互斥（四档个数加上「没有状态且没归档」的 = 总数）；选了 `archived` 之后四个数**不变**（自己不算自己），`total_count` = 已归档个数，而 `counts.reading / cardState / forms / scenarios` 变成只算已归档这个子集。
- L6 叠加与排序：`archived` + 关键词 + 场景 + 状态组 + 类型 = 交集，`counts.maintenance` 随这些联动；`archived` 筛选配 `starred / stars / updated` 三种排序，顺序 = 不带 `maintenance` 时把不符合的去掉后的顺序；配带关键词的 `relevance` 排序时，命中原因、分数、档位照常。
- L7 校验：`"Archived"`（大写）、`"dead"` → `Err("维护状态筛选无效")`；`"archived"` 合法；`None`、`""`、`"  "` 当不筛。
- L8 回归：卡 49 的既有边界表、筛选、计数、顺序、校验测试原样通过（`archived` 为 NULL 时行为不变）。

**前端**
- F1 `maintenance.test.ts`：`maintenanceOf(null, now, true)` 是 `'archived'`；坏日期 + `archived = true` 是 `'archived'`；昨天 + `archived = true` 是 `'archived'`（压过活跃）；`archived = false` 或不传时，和原来的边界表结果完全一样；`MAINTENANCE_TIERS` 顺序固定且最后一个是 `'archived'`；已归档的标签文案和两种悬停文案。
- F2 `library-filters.test.ts`：`maintenanceOptions` 有四项、顺序固定（活跃、放缓、疑似停更、已归档）、已归档的个数缺失或不是有限数字当 0、已归档那一项的 `hint` 是带「同步」提醒的那句、其它三项的 `hint` 不变；`toFilters` 选了已归档时带 `maintenance: 'archived'`；`toggleMaintenance` 点已归档再点取消。
- F3 `filter-panel.test.tsx`：出现「已归档」项和它的个数；选中项 `aria-pressed="true"`；点击回调带 `'archived'`；个数为 0 且未选中的淡一点；悬停提示是上面那句。
- F4 `item-row.test.tsx`：`archived: true` → 行里有「已归档」（`pushedAt` 是昨天和 `null` 各一次）；`archived: false` 或没有这个字段 → 和以前一样（活跃不画、放缓和疑似停更照画）；行里原有内容（名字、一句话、场景标签、星数、状态标签、命中原因）不变。
- F5 `item-detail.test.tsx`：`archived: true` → 详情有「已归档」小标签（`pushedAt` 为 `null` 也有）；其它三档的行为不变。
- F6 `maintenance-tag.test.tsx`：已归档的文案、红色、`title` 是悬停文案；`showActive = false` 时已归档**仍然画**，活跃仍然不画。
- F7 `api.test.ts`：`list_library_view` 的参数原样带上 `maintenance: 'archived'`；响应里的 `archived` 和 `counts.maintenance.archived` 原样读出（仿该文件里已有写法）。

4. 改坏检查（各做一遍，写「改了什么 → 哪条测试失败」，做完还原；改完生产代码要还原干净，`git diff --stat` 里生产文件的行数和还原前一致）：
   ① `upsert_repositories` 第一个 `ON CONFLICT` 分支的 COALESCE 改成直接 `excluded.archived`；
   ② 第二个 `ON CONFLICT` 分支去掉 `archived` 的更新；
   ③ `Some(true)` 写成 `0`；
   ④ `is_active!` 去掉「不是已归档」；
   ⑤ `is_stale!` 去掉「不是已归档」；
   ⑥ `is_active!` 里的 `IFNULL(r.archived, 0) <> 1` 去掉 `IFNULL`（写成 `r.archived <> 1`）；
   ⑦ `maintenance_counts` 里 `archived` 那一列改用 `IS_STALE`；
   ⑧ `normalize` 的合法值里去掉 `"archived"`；
   ⑨ `into_item` 不写 `archived`（恒为 `false`）；
   ⑩ `maintenanceOf` 把 `archived` 的判断挪到日期判断之后；
   ⑪ `MAINTENANCE_TIERS` 把 `'archived'` 放到第一位；
   ⑫ 列表行对已归档不画标签（让 `MaintenanceTag` 在 `showActive` 为假时也隐藏已归档）。
5. 报告里写：迁移文件和登记位置（文件:行）；`archived` 在同步里经过的每一处（`github.rs:行` → `storage.rs:行`）；`library_view.rs` 里三处 SELECT 的列序号是怎么调整的；四档互斥的 SQL 怎么写的；新增的 SQL 子句在哪；前端 `maintenanceOf` 的优先级写在哪。

## 停止条件
- 需要改「允许改动」以外的文件（比如某个测试夹具在卡外文件里，类型编译不过）→ 停手写进报告。
- 需要联网、加依赖、或者改已有迁移 → 停手写进报告。
- 迁移在测试里表现出会动已有数据或其它列（除了新增 `archived` 这一列）→ 停手写进报告。
- 发现已有测试因为本卡必须被删除或放宽（把 037 改 038、给 JSON 键列表加 `archived` 除外）→ 停手写进报告。
- 想不用 `IFNULL(r.archived, 0)` 的写法，或想把 `archived` 改成非空带默认值 → 停手写进报告，不要自作主张。
