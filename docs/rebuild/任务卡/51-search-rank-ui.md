---
编号: 51-search-rank-ui
允许改动:
  - apps/desktop/src-tauri/src/library_view.rs
  - apps/desktop/src/lib/contracts.ts
  - apps/desktop/src/lib/match-hint.ts
  - apps/desktop/src/lib/match-hint.test.ts
  - apps/desktop/src/lib/library-filters.ts
  - apps/desktop/src/lib/library-filters.test.ts
  - apps/desktop/src/components/library/match-hint.tsx
  - apps/desktop/src/components/library/match-hint.test.tsx
  - apps/desktop/src/components/library/item-row.tsx
  - apps/desktop/src/components/library/item-row.test.tsx
  - apps/desktop/src/pages/v2/library.tsx
---

# 卡 51 · 资料库结果行：显示「命中原因」和匹配度（4 档）

> 执行者：Grok（Fox 2026-10-22 说「继续卡 51」，和卡 49 / 50 同一套）｜不加迁移、不加依赖、不花额度、不联网。接卡 50 备好的 `matchReasons`；**匹配度的档位放在后端算**（只加一个很小的 Rust 字段），界面只负责画出来。

## Fox 能看到的效果
在「资料库」搜索框里输入关键词后，每一行的中文说明下面多出一行小字：
- 左边是 4 根逐级变高的小竖条（像手机信号格）：亮几根就是几档匹配度，越多越贴近你搜的词；
- 右边写「命中：项目名 / 中文说明」——你搜的词出现在这个工具的哪几个地方；
- 鼠标停在这一行上，会说明这一档是什么意思（例如「匹配度 3 / 4：搜的词主要出现在中文一句话说明里」）。

列表右上角的「N 个」前面，搜索时多一句「按相关度排序」，让人知道为什么是这个顺序。

**没输入关键词时，所有行的样子和现在完全一样**（今日页、没搜索的资料库都不多任何东西）。

用竖条、不用星号：每一行右边已经有 GitHub 的「★ 星数」，再放星号会混在一起。

## 背景与先读什么（行号以集成线 20abd43 为准）
Rust `apps/desktop/src-tauri/src/library_view.rs`：
- `LibraryItem`（约 205–225 行）：卡 50 加了 `match_reasons`、`match_score`，始终序列化。
- 评分：常量 `WEIGHT_*` / `BONUS_*`（约 742–748 行）；`MatchScore`（约 765 行）；纯函数 `score_keyword_match(terms, fields)`（约 852–920 行，空词项会被跳过）。
- 相关度查询 `query_items_by_relevance`（约 1037–1081 行）和 `RankedRaw`（约 1019 行）：最后一步 `item.match_score = Some(..)`、`item.match_reasons = ..`。
- `into_item` 里 `match_reasons: Vec::new()`、`match_score: None`（约 1308 行）；测试里的 `sample_item`（约 3986 行）；序列化键测试（约 3559–3577、3692 行）；今日页 / 详情页返回 `[]` / `null` 的测试（约 4086 行）。
- 测试写法：`score_term` 助手（约 4070 行）、`relevance_sort_orders_name_then_zh_then_description_then_topic`（约 4326 行，造 4 个互相区分的仓库）、`relevance_paginates_after_sorting`（约 4410 行）。

前端 `apps/desktop/src/`：
- `lib/contracts.ts` 约 70–72 行：`matchReasons?`、`matchScore?`。
- `components/library/item-row.tsx`：行布局，中文说明之后是 `LangDot / 场景 / 维护标签` 那一行（约 57–66 行）。
- `components/library/maintenance-tag.tsx` + `lib/maintenance.ts`：卡 49 的写法——文案常量放 `lib/`，组件放 `components/library/`，各配测试。照这个来。
- `lib/library-filters.ts` 的 `toFilters`（约 113 行）：有关键词且默认排序时发 `relevance`。
- `pages/v2/library.tsx` 约 590 行：`<span …>{resultCount == null ? '…' : resultCount} 个</span>`。
- `pages/v2/today.tsx` 也用 `ItemRow`；它拿到的 `matchReasons` 是 `[]`、`matchStars` 是 `null`，必须什么都不画。
- 「边输入边出」已经有了（`library.tsx` 里 `KEYWORD_DEBOUNCE_MS = 200`），本卡不改。
- 文案用词照 `docs/rebuild/文案审读.md` 的统一用词表：「中文说明」「项目名」「话题」，不要写成「AI 摘要」之类。

## 要做的事

### 1. 后端：每条结果带匹配度档位 `matchStars`
1. `LibraryItem` 新增 `match_stars: Option<u8>`（序列化成 `matchStars`，始终出现）。有关键词且 `sort = relevance` 时是 `Some(1..=4)`；其它所有情况（别的排序、关键词为空、今日页、详情页）是 `None`，序列化成 `null`。
2. 档位由总分和「实际参与打分的词项个数 n」算。`n` = `terms` 里非空词项的个数，至少按 1。写成**纯函数** `match_stars(score: i64, term_count: usize) -> u8`，整数比较、不用小数：
   - `score >= 100 * n` → 4（搜的词主要出现在项目名里）
   - `score >= 60 * n` → 3（主要在中文一句话说明里）
   - `score >= 30 * n` → 2（主要在中文摘要或英文简介里）
   - 其它（包括 0 分）→ 1
   也就是「每个词平均得多少分」落在哪一档。0 分的记录**仍然保留**，显示 1 档。
3. `MatchScore` 新增字段 `stars: u8`，在 `score_keyword_match` 末尾算好；`RankedRaw` 带上它，`query_items_by_relevance` 把它写进 `item.match_stars`（`Some(stars)`）。`into_item` 和测试里的 `sample_item` 补 `match_stars: None`。
4. **排序、总数、计数、匹配到哪些记录，一字不变**——这张卡不改任何排序键，也不改 `FILTER_WHERE`、`counts`。档位只是多带一个字段。
5. 以后「混合检索」（卡 52）会换算法，字段名不变。所以界面只认 `matchStars`，**不要在前端用分数自己推档位**。

### 2. 前端：契约和文案（`lib/`）
1. `contracts.ts`：`LibraryItem` 加可选字段 `matchStars?: number | null;`，注释写清：「匹配度档位 1–4，只有带关键词的相关度排序才有值」。
2. 新文件 `lib/match-hint.ts`：
   - `MATCH_STAR_MAX = 4`。
   - `normalizeMatchStars(value: unknown): 1 | 2 | 3 | 4 | null`：只认 1、2、3、4 这四个整数，其它（0、5、2.5、-1、`NaN`、字符串、`null`、`undefined`）一律返回 `null`。
   - `MATCH_STAR_HINTS: Record<1 | 2 | 3 | 4, string>`，文案一字不差用下面四句：
     - 4：`匹配度 4 / 4：搜的词主要出现在项目名里`
     - 3：`匹配度 3 / 4：搜的词主要出现在中文一句话说明里`
     - 2：`匹配度 2 / 4：搜的词主要出现在中文摘要或英文简介里`
     - 1：`匹配度 1 / 4：只是沾边，多在话题或关键词里`
   - `matchReasonsLine(reasons: readonly unknown[] | null | undefined): string | null`：只留非空字符串，去首尾空白，去重（保持原顺序），最多取前 4 个，用 ` / ` 连接，前面加 `命中：`；一个有效的都没有 → `null`。例：`['项目名', '中文说明']` → `命中：项目名 / 中文说明`。
3. `library-filters.ts` 新增 `isRelevanceSort(state: LibraryUiState): boolean`，实现就是 `toFilters(state, 0, 1).sort === 'relevance'`（复用，不要把判断条件再抄一遍）。

### 3. 前端：组件和页面
1. 新组件 `components/library/match-hint.tsx`：`MatchHint(props: { reasons?: readonly string[] | null; stars?: number | null })`。
   - `stars` 经 `normalizeMatchStars` 后是 `null`，且 `matchReasonsLine(reasons)` 也是 `null` → 返回 `null`，什么都不画。
   - 否则画一行：`<div className="mt-1.5 flex items-center gap-2 text-xs text-on-surface-variant" title=…>`。有效档位时 `title` 是 `MATCH_STAR_HINTS[档位]`，否则不设 `title`。里面按顺序：
     - 有效档位 → 4 根竖条：`<span role="img" aria-label="匹配度 N / 4" className="inline-flex items-end gap-[2px]">`，每根 `<i>` 宽 3px、圆角 1px，高度依次 6 / 8 / 10 / 12px；前 N 根用 `bg-primary`，其余用 `bg-border`。
     - 有命中文字 → `<span className="min-w-0 truncate">命中：项目名 / 中文说明</span>`（`truncate`，一行放不下就省略）。
   - 只有档位没有原因 → 只画竖条；只有原因没有档位 → 只画文字（不设 `title`）。
2. `item-row.tsx`：在中文说明那块 `{text ? … : null}` **之后**、`LangDot / 场景 / 维护标签` 那一行**之前**插入 `<MatchHint reasons={props.item.matchReasons} stars={props.item.matchStars} />`。其它一字不动（右侧的 ★ 星数和状态标签、悬停按钮、`why` 行都不动）。
3. `pages/v2/library.tsx`：右上角的 `{resultCount == null ? '…' : resultCount} 个`，在 `isRelevanceSort(ui)` 为真时，数字前面多一句 `按相关度排序 · `（放在同一个 `<span>` 里）；其它情况和现在一模一样。只改这一处。

## 不要做
- 不改任何排序规则、`FILTER_WHERE`、`counts`、`total_count`、评分权重和加分；不改 `matchReasons` / `matchScore` 的含义。
- 不改 `library.rs`、`v2_ops.rs`、`solve*.rs`、`discover.rs`、`lib.rs`；不改今日页（`today.tsx`）、详情页、找方案页；`item-row.tsx` 里除了插入那一行，不动别的。
- 不用星号字符（★ ☆）画匹配度；不加图标库、不加依赖、不加迁移、不运行 `pnpm install`、不联网、不调用任何 AI。
- 不删除、不放宽任何已有测试的断言。

## 验收
1. `git diff --stat`（含新文件）的文件都在「允许改动」里。
2. `tools/cards/run.sh web-build`、`web-test`、`rust-fmt`、`rust-check`、`rust-test` 全部通过；报告里写前端项数、Rust 通过 / 失败 / 忽略数。**基线（集成线 20abd43，总控已测）：Rust 完整套件 1429 通过 / 0 失败 / 5 忽略（`run.sh rust-test` 因为跳过 1 个钥匙串测试，会显示 1428）；前端 39 个文件 878 项。** 你的结果 = 基线 + 你新增的测试数，已有测试一项不少（不用再跑基线）。
3. 测试清单（每条一个测试，报告里写「第 N 条 → 测试名」）：

**Rust**
1. `match_stars` 边界表：n = 1：150、100 → 4；99、60 → 3；59、30 → 2；29、0 → 1。n = 2：200 → 4；199、120 → 3；119、60 → 2；59 → 1。n = 4：400 → 4；399、240 → 3；239、120 → 2；119 → 1。
2. 字段对应档位：造 4 个互相区分的仓库（同一个词只在项目名 / 只在中文一句话 / 只在英文简介 / 只在话题里），`sort = relevance` 搜这个词 → `matchStars` 依次是 4 / 3 / 2 / 1，且和各自的 `matchReasons` 对得上。
3. 摘要命中（45 分）→ 2；关键词命中（20 分）→ 1；`LIKE` 匹配到但评分 0 分的记录 → 仍在结果里，`matchStars == Some(1)`。
4. 多个词按平均：两个词，一个在项目名词首（120 分）、一个在话题（20 分），总分 140、n = 2 → 3；两个词都只在话题 → 1。
5. 空词项不算个数：`score_keyword_match(&["note", ""], …)` 的 `stars` 与 `score_keyword_match(&["note"], …)` 一样。
6. 非相关度排序（`starred / stars / updated`）带关键词，以及 `relevance` + 空关键词 / 全空白 → `matchStars` 为 `None`。
7. 今日页（`get_today_view`）和详情页返回的 `LibraryItem` 里 `matchStars == None`。
8. 序列化：一条 `LibraryItem` 转 JSON 后键名是 `matchStars`（驼峰），无值时是 `null`，有值时是数字。
9. 单调：同一次相关度查询（一个词项）里，列表往下 `matchStars` 不会变大。
10. 卡 50 的排序、分页、计数测试原样通过（不改期望）。

**前端 · `lib/match-hint.test.ts`**
11. `normalizeMatchStars`：1、2、3、4 原样返回；0、5、2.5、-1、`NaN`、`'3'`、`null`、`undefined` 都返回 `null`。
12. `MATCH_STAR_HINTS`：恰好 4 个键，文案和上面四句一字不差。
13. `matchReasonsLine`：`['项目名', '中文说明']` → `命中：项目名 / 中文说明`；`[]`、`null`、`undefined` → `null`；混着空串、空白、数字、重复项时只留有效且不重复的字符串；超过 4 个只取前 4 个。

**前端 · `components/library/match-hint.test.tsx`**
14. 档位 3 + 两个原因 → 能找到文字 `命中：项目名 / 中文说明`；`role="img"` 的 `aria-label` 是 `匹配度 3 / 4`；4 根竖条里恰好 3 根是 `bg-primary`、1 根是 `bg-border`。
15. 外层的 `title` 等于 `MATCH_STAR_HINTS[3]`。
16. 原因 `[]` 且档位 `null` / `undefined` → 什么都不渲染（容器是空的）。
17. 只有原因 → 只有文字，没有竖条、没有 `title`；只有档位 → 只有竖条，没有文字。
18. 档位是无效值（5、0、`'3'`）时按没有档位处理。

**前端 · `components/library/item-row.test.tsx`**
19. 带 `matchReasons` / `matchStars` 的条目：行里出现 `命中：…` 和匹配度竖条，并且在 DOM 顺序上位于中文说明之后、语言 / 场景那一行之前（用 `compareDocumentPosition`）。
20. 不带这两个字段（或 `[]` + `null`）的条目：行里没有 `命中：`、没有 `role="img"` 的竖条；原有断言全部不变。

**前端 · `lib/library-filters.test.ts`**
21. `isRelevanceSort`：有关键词 + 默认排序 → `true`；关键词只有空白 → `false`；选了 `stars` 或 `updated` 排序 → `false`。

4. 改坏检查（各做一遍，写「改了什么 → 哪条测试失败」，做完还原）：① 把 `>= 100 * n` 改成 `> 100 * n`；② 阈值 60 改成 50；③ 比较时不乘 `n`；④ 0 分的记录档位改成 0 或 `None`；⑤ 非相关度排序也给 `Some(1)`；⑥ 同一次查询里把档位写反（高分低档）；⑦ `normalizeMatchStars` 接受 5；⑧ `matchReasonsLine` 的分隔符改成「、」；⑨ `MatchHint` 在档位无效时仍画竖条；⑩ 竖条点亮数量多一根；⑪ `ItemRow` 不传 `matchStars`；⑫ 把 `MatchHint` 挪到语言 / 场景那一行之后；⑬ `isRelevanceSort` 在没关键词时也返回 `true`。
5. 报告里写：`match_stars` 在哪个文件:行；`query_items_by_relevance` 里哪一行把档位写进 `LibraryItem`；`MatchHint` 在 `item-row.tsx` 的第几行插入；`library.tsx` 改了哪一行。

## 停止条件
- 要改排序规则、`FILTER_WHERE`、`counts`，或者要加迁移才能做 → 停手写进报告。
- 既有测试在你改完后失败，而且只能改测试期望才能过 → 停手写进报告，不要改既有测试的期望。
- 需要新依赖、要改「允许改动」以外的文件 → 停手写进报告。
