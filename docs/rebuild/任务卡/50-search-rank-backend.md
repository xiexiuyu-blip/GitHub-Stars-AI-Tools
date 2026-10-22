---
编号: 50-search-rank-backend
允许改动:
  - apps/desktop/src-tauri/src/library_view.rs
  - apps/desktop/src/lib/contracts.ts
  - apps/desktop/src/lib/library-filters.ts
  - apps/desktop/src/lib/library-filters.test.ts
  - apps/desktop/src/lib/api.test.ts
---

# 卡 50 · 资料库搜索排序 + 命中原因（后端为主，前端只改两处小地方）

> 执行者：Grok（见分配单）｜不加迁移、不加依赖、不花额度、不联网。**界面上「命中：项目名 / 中文说明」的显示、1–4 星不在这张卡里（那是卡 51）**；这张卡只把排序和数据准备好。

## Fox 能看到的效果
在「资料库」搜索框里输入关键词时：**名字里带这个词的工具排在最前**，其次是中文说明里有的，再是英文简介里有的，最后才是只在话题 / 关键词里有的；同一档里星数多的在前面。不输入关键词时，排序和原来**完全一样**。（每条结果带的「命中原因」这张卡不显示，只是备好数据。）

## 背景与先读什么（行号以集成线 d7c069f 为准）
- `apps/desktop/src-tauri/src/library_view.rs`：
  - 现状：关键词匹配只是 SQL 的 `LIKE`——宏 `keyword_term!`（约 25–52 行）对 6 个字段做「包含」，`FILTER_WHERE`（约 57–99 行）里叠四次（最多四个词项，彼此是「并且」）；`like_patterns`（约 572–586 行）把关键词按空白拆开，再把「只由已知任务词连起来的中文」拆开（`split_known_words`，约 540–570 行）；**没有任何相关度排序**，`normalize`（约 474–504 行）里 `sort` 只认 `starred / stars / updated`，`query_items`（约 634 行起）用 `ORDER BY … LIMIT :limit OFFSET :offset`。
  - `LibraryItem`（约 135–154 行）；`into_item`（约 840–866 行）；`read_raw`（约 824 行）；`ITEM_SELECT`（约 107 行，现在只取 `r.id, r.full_name, r.html_url, r.description, r.language, r.stars_count, r.starred_at, r.pushed_at, d.one_liner_zh, d.card_json, a.read_status`）。
  - 注意：`library.rs` 里还有**另一个**同名的 `LibraryItem`（旧版，约 39 行）——那个不碰；你只改 `library_view.rs` 里的这个。测试模块里的 `sample_item`（约 3497 行）也用结构体字面量构造 `LibraryItem`，加了字段后要跟着补。
  - 测试夹具：测试模块里的 `Lab` / `Repo`（约 1380–1515 行）；关键词测试照 `keyword_matches_each_column_case_insensitively`（约 1686 行）、`keyword_terms_may_sit_in_different_columns_in_any_order`（约 1733 行）、`keyword_escapes_percent_and_underscore`（约 1813 行）的写法。
- `apps/desktop/src/lib/contracts.ts`：`LibraryFilters.sort`（约 35 行）、`LibraryItem`（约 38 行起）。
- `apps/desktop/src/lib/library-filters.ts`：`toFilters`（约 113 行）；`library-filters.test.ts` 里已有的 `toFilters` 测试。
- 页面 `pages/v2/library.tsx` 是用 `toFilters(...)` 的结果调用 `list_library_view` 的，**不用改页面**。

## 要做的事

### 1. 后端：新增排序方式 `relevance`（相关度）
1. `LibraryFilters.sort` 新增合法值 `"relevance"`；`"" / starred / stars / updated` 行为一字不变；其它值仍然返回 `排序方式无效`。
2. `sort = relevance` 且有关键词 → 按下面的配方排序。`sort = relevance` 但关键词为空 → **当作 `starred`**（不报错）。
3. **哪些记录出现、有多少条、左侧各项计数，一点都不变**——`FILTER_WHERE`、`total_count`、`counts`、`keyword_term!`、`like_patterns` 的匹配规则原样不动。只改「出现的这些记录排什么顺序」。

### 2. 评分配方（Fox 定的顺序：项目名 > 中文说明 > 英文简介 > 话题）
**词项**：用和 SQL 同一批词项（`like_patterns` 里拆出来的那些，最多 4 个）。把拆词这一步抽成一个返回 `Vec<String>` 的小函数，SQL 的模式和评分共用它，不要写两套拆法。比较时词项和字段都转小写（和 `LIKE` 不分大小写保持一致）。

**字段与权重**（一个词项在某个字段里「包含」就算命中）：

| 字段 | 取自 | 权重 | 命中原因标签 |
|---|---|---|---|
| 项目名 | `repositories.full_name` | 100 | 项目名 |
| 中文一句话 | `repo_ai_documents.one_liner_zh` | 60 | 中文说明 |
| 中文摘要 | `repo_ai_documents.summary_zh` | 45 | 中文说明 |
| 英文简介 | `repositories.description` | 30 | 英文简介 |
| 话题 | `repositories.topics_json` 解析出来的字符串（JSON 数组里每个字符串各自比较；解析失败或不是字符串的元素当没有） | 20 | 话题 |
| 关键词 | `repo_ai_documents.keywords_json`，解析方式同上 | 20 | 话题 |

**一个词项的得分** = 它命中的所有字段里**最高**的那个权重（不叠加）。没有命中任何字段（`LIKE` 在非 ASCII 字母的大小写上和 Rust 的小写化略有差别时可能出现）→ 0 分，**这条记录仍然保留**，不能丢。

**两个加分**（每个词项各自最多加一次）：
- 词首 +20：该词项在 `full_name` 里命中，并且**任意一处**出现的位置满足「在 `full_name` 的开头，或前一个字符不是字母 / 数字（`/ - _ .` 空格等）」。例：搜 `note`，`o/my-note-app` 命中位置前一个字符是 `-` → 加分；`o/footnote` 命中在中间 → 不加。
- 仓库名整词 +30：`full_name` 里 `/` 之后的仓库名（小写）**整个等于**该词项。例：搜 `react`，`facebook/react` 加分，`remix-run/react-router` 不加。

**总分** = 各词项得分之和。

**命中原因 `matchReasons`**：把「至少有一个词项命中过的字段」对应的标签去重，按固定顺序 `项目名`、`中文说明`、`英文简介`、`话题` 排列（不是只看最高档的那个字段）。

### 3. 排序、分页、输出字段
1. 排序键依次：总分高 → 低；星数多 → 少；收藏时间新 → 旧；`id` 升序。结果是确定的，翻页不会重复或漏。
2. **分页要在排完序之后切**：`relevance` 模式下把所有匹配记录取出来（需要在 `SELECT` 里多带 `d.summary_zh`、`d.keywords_json`、`r.topics_json` 三列；别的排序方式的 `SELECT` 不动），在 Rust 里评分、排序，再按 `offset / limit` 切（`limit` 仍夹在 1–200，默认 80）。库里最多几千条，不需要缓存和索引。
3. `LibraryItem` 新增两个字段，**始终序列化**（camelCase）：
   - `matchReasons: Vec<String>`——有关键词且 `relevance` 时是上面算出来的；其它所有情况（包括今日页、详情页、别的排序）是空数组 `[]`，不是 `null`。
   - `matchScore: Option<i64>`——有关键词且 `relevance` 时是总分；其它情况 `null`。
4. 评分函数写成**纯函数**（输入：词项 + 这一条的几个字段，输出：总分 + 原因），单独测，不依赖数据库。

### 4. 前端（只改这两处）
1. `contracts.ts`：`LibraryFilters.sort` 加 `'relevance'`（注释：「relevance = 按相关度（有关键词时用）」）；`LibraryItem` 加可选字段 `matchReasons?: string[]`、`matchScore?: number | null`（注释写清：只有带关键词的相关度排序才有值）。
2. `library-filters.ts` 的 `toFilters`：**有关键词（trim 后非空）且 `state.sort` 是默认的 `'starred'` 时，发 `sort: 'relevance'`**；没关键词，或用户选了别的排序，原样。`LibraryUiState.sort` 的类型不变（不让用户选 relevance）。

## 不要做
- 不改 `FILTER_WHERE`、`keyword_term!`、`like_patterns` 的**匹配规则**（只许把拆词抽成共用函数，行为不变）、`total_count`、`counts`。
- 不改 `library.rs`、`v2_ops.rs`、`solve*.rs`、`discover.rs`、`lib.rs`；不改任何页面或组件（`library.tsx`、`item-row.tsx`、`today.tsx` 等）——界面显示是卡 51。
- 不用 FTS5、不新增迁移、不加依赖、不运行 `pnpm install`、不联网、不调用任何 AI。
- 不删除、不放宽任何已有测试的断言。

## 验收
1. `git diff --stat` 的文件都在「允许改动」里。
2. `tools/cards/run.sh web-build`、`web-test`、`rust-fmt`、`rust-check`、`rust-test` 全部通过；报告里写前端项数、Rust 通过 / 失败 / 忽略数。**基线（集成线 d7c069f，总控已测）：Rust 1396 通过 / 0 失败 / 5 忽略；前端 37 个文件 868 项。** 你的结果 = 基线 + 你新增的测试数，已有测试一项不少（不用再跑基线）。
3. 测试清单（每条一个测试，报告里写「第 N 条 → 测试名」）：

**Rust · 评分纯函数**
1. 权重顺序：同一个词只在项目名 / 只在中文一句话 / 只在英文简介 / 只在话题里命中，得分依次是 100 / 60 / 30 / 20。
2. 摘要权重 45，关键词权重 20（原因标签分别是「中文说明」「话题」）。
3. 取最高不叠加：同一个词同时在项目名和英文简介里 → 100，不是 130。
4. 词首加分：`o/my-note-app` 搜 `note` 得 100+20；`o/footnote` 搜 `note` 得 100。
5. 仓库名整词加分：`facebook/react` 搜 `react` 得 100+20+30（词首 + 整词）；`remix-run/react-router` 搜 `react` 得 100+20。
6. 多个词项相加：`["小红书","封面"]` 一个在项目名一个在中文说明 → 100 + 60。
7. 原因去重且按固定顺序：话题和项目名都命中 → `["项目名","话题"]`；中文一句话和摘要都命中 → 只有一个 `"中文说明"`。
8. 一个词项没命中任何字段 → 0 分、空原因，但函数不报错。
9. 话题 / 关键词的 JSON 坏了（`not-json`、数组里混着数字）→ 当没有，不 panic。

**Rust · 查询**
10. 排序：造 4 个互相区分的仓库（名字里有 / 只中文说明有 / 只英文简介有 / 只话题有），`sort = relevance` 搜同一个词 → 顺序正好是这 4 个，且每条的 `matchReasons` 与之对应。
11. 同档里星数多的在前；星数也相同时收藏时间新的在前；全都相同时 `id` 小的在前（给出三组例子）。
12. 分页：6 个匹配仓库，`limit=2` 分三页（offset 0/2/4）拼起来 == 一页取完的顺序，没有重复和遗漏；`total_count` 等于 6。
13. `relevance` + 空关键词 / 全空白 → 不报错，顺序等于 `starred`，`matchReasons` 为 `[]`、`matchScore` 为 `null`。
14. 其它排序（`starred / stars / updated`）带关键词 → 顺序和之前一字不差，`matchReasons` 为 `[]`、`matchScore` 为 `null`。
15. `sort = "foo"` 仍报 `排序方式无效`。
16. 所有现有的关键词测试（各列不分大小写、多词乱序、已知词拆分、`%` `_` 转义）原样通过；`relevance` 下再各测一次「出现哪些记录」与 `starred` 下完全相同（只有顺序可能不同）。
17. 其它筛选叠加：场景 / 状态 / 类型筛选 + 关键词 + `relevance` → 记录集合等于不带 `relevance` 时，计数 `counts` 等于不带 `relevance` 时。
18. 今日页（`get_today_view`）和详情页返回的 `LibraryItem` 里 `matchReasons == []`、`matchScore == null`。
19. 序列化：一条 `LibraryItem` 转 JSON 后有 `"matchReasons":[]` 和 `"matchScore":null`（键名驼峰）。

**前端 · `library-filters.test.ts`**
20. 有关键词、`sort = 'starred'` → `toFilters` 的 `sort === 'relevance'`；关键词只有空白 → 仍是 `'starred'`；`sort = 'stars'` 且有关键词 → 仍是 `'stars'`。

4. 改坏检查（各做一遍，写「改了什么 → 哪条测试失败」，做完还原）：① 项目名和中文一句话的权重对调；② 去掉词首加分；③ 去掉仓库名整词加分；④ 同分时不按星数排；⑤ `offset / limit` 改成在排序之前就切；⑥ `relevance` + 空关键词改成报错；⑦ 话题命中的原因标签写成「中文说明」；⑧ `toFilters` 有关键词时不再发 `relevance`；⑨ 把 `LIKE` 匹配到但评分 0 分的记录丢掉（过滤掉 `score == 0`）。
5. 报告里写：评分函数在哪个文件:行；SQL 取数和评分共用的拆词函数在哪；`relevance` 模式的 `SELECT` 多带了哪三列。

## 停止条件
- 要改 `FILTER_WHERE` 的匹配规则、`counts` 或加迁移才能做 → 停手写进报告。
- 既有关键词测试在你改完后失败，而且只能改测试期望才能过 → 停手写进报告，不要改既有测试的期望。
- 需要新依赖、要改「允许改动」以外的文件 → 停手写进报告。
