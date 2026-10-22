---
编号: 49-maintenance-badge
允许改动:
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

# 卡 49 · 维护状态：活跃 / 放缓 / 疑似停更（标签 + 资料库筛选）

> 执行者：Grok（见分配单）｜不加迁移、不改同步、不加依赖、不联网、不花额度。**「已归档」和「找方案命中停更工具时提示」不在这张卡里**：库里没存 GitHub 的 archived 标记（`repositories` 没有这一列，同步也不存），要先加迁移 + 改同步，另开卡。

## Fox 能看到的效果
1. 资料库左栏最下面多一组「维护状态」：活跃 · 放缓 · 疑似停更，每项后面有个数（和场景、类型一样，会跟着其他筛选联动）。点一下只看这一类，再点一下取消；个数是 0 的淡一点。
2. 资料库列表里：**放缓**的项目，小标签行里多一个灰色「放缓」；**疑似停更**的多一个红色「疑似停更」。**活跃的不画**（绝大多数是活跃，画了满屏都是）。
3. 详情页「项目最近更新：X」那一行后面，三种状态都画一个小标签。
4. 没有「最近推送时间」的项目：没有标签、不属于任何一档、个数里也不算；没筛选时照常出现在列表里。
5. 鼠标悬停标签或筛选项，有一句话说明门槛（见下）。

## 门槛（Rust 和前端必须一模一样）
- 距今多久 = 当前时间 − `pushed_at`（GitHub 最近一次推送时间）。
- ≤ 90 天 → `active`「活跃」；> 90 天且 ≤ 365 天 → `slow`「放缓」；> 365 天 → `stale`「疑似停更」。
- `pushed_at` 是 NULL / 空白 / 解析不了 → 没有状态（`null`）。比当前时间还晚（时钟误差）→ `active`。
- **用整数秒（前端用毫秒差）比较，不要用 `julianday` 小数相减**：浮点在「正好 90 天」的边界上会差一点点，测试会随机失败。SQL 里用 `CAST(strftime('%s', x) AS INTEGER)`；`strftime` 解析不了会得到 NULL，正好对应「没有状态」。
- 门槛常量：Rust 里 `MAINTENANCE_ACTIVE_DAYS = 90`、`MAINTENANCE_STALE_DAYS = 365`；前端 `lib/maintenance.ts` 里同名常量。SQL 里**不写** 90 / 365 的字面量，用命名参数绑定。
- 两边各有一张**相同的边界表**做测试（见验收 R1 / F1），任何一边单独改门槛都会有测试失败。

悬停文案（前后端各处用同一套，放在 `maintenance.ts`）：
- 活跃：「最近 3 个月内有更新」
- 放缓：「3 到 12 个月没有更新」
- 疑似停更：「超过 1 年没有更新，可能已经不维护了」

## 背景与先读什么（卡 50 已合并；以集成线最新代码为准，行号只作参考）
- `apps/desktop/src-tauri/src/library_view.rs`：`FILTER_WHERE`（命名参数 `:scenario / :reading / :card / :form / :pattern…`）、`filter_params!` 宏、`Normalized` + `normalize` + `cleared_scenario / cleared_reading / cleared_card / cleared_form`（每一维「去掉自己」）、`scenario_counts / reading_counts / card_state_counts / form_counts`、`list_library_view`（**不带 `now`**）、`get_today_view(path, now)`（带 `now`，测试可固定时间，它对 `now` 的合法性检查可以照抄）、`query_items`（含卡 50 加的相关度分支，**两条取数路径都要带上新参数**）。**照 `reading` 这一维的每一处接线，新增一维 `maintenance`。**
- 当前时间：`lib.rs` 用 `time::OffsetDateTime::now_utc().format(&Rfc3339)` 取；`lib.rs` 不在允许范围，所以 `list_library_view` 在 `library_view.rs` 里自己用 `time` crate 取现在，再调用新增的 `list_library_view_at(path, filters, now)`（测试用后者固定时间）。Tauri 命令 `list_library_view` 的签名不变。
- 契约测试：测试模块里逐一列出 JSON 键名的几条（搜 `object_keys(&counts_json`）和用结构体字面量构造 `LibraryCounts` 的地方，会因为新增 `maintenance` 而失败——**往期望里加上新键是预期的，不算放宽断言**。
- 前端：`lib/contracts.ts`（`LibraryFilters`、`LibraryCounts`）；`lib/library-filters.ts`（`LibraryUiState`、`createLibraryUiState`、`toFilters`、`toggleForm`、`clearFilters`、`hasActiveFilters`、`libraryEmptyNote`、`formOptions`）；`components/library/filter-panel.tsx`（`GroupTitle` + `FilterButton`）；`components/library/item-row.tsx`（行里小标签用 `@/components/kit` 的 `Tag`，色调只有 gray / blue / red）；`components/library/thin-tag.tsx`（窄左栏里小标签的写法，仿它）；`components/library/item-detail.tsx`（`项目最近更新：{relativeDays(detail.pushedAt, now)}` 那一行，`now` 在组件里 `new Date()`）；`pages/v2/library.tsx`。
- 页面接线：用 `grep -n "ui.form\|chooseForm\|toggleForm\|formOptions\|onForm\|uiKey" apps/desktop/src/pages/v2/library.tsx` 找出「类型」筛选的全部接线，`maintenance` 照样接一遍。已知要改的有：`narrowedDown`（约 514 行）、筛选按钮高亮 `className`（约 570 行）、`FilterPanel` 的 props（约 656–663 行）、`uiKey`（约 758 行，**漏了会导致切换筛选不重新取数**）。

## 要做的事

### 1. 后端（`library_view.rs`）
1. `LibraryFilters` 新增 `maintenance: Option<String>`（`#[serde(default)]`）。合法值 `""`（含 `None`、纯空白）= 不筛、`"active"`、`"slow"`、`"stale"`（严格小写）；其它 → `Err("维护状态筛选无效")`。
2. `Normalized` 新增 `maintenance: String` 和当前时间 `now: String`；`normalize` 校验；新增 `cleared_maintenance()`。
3. `FILTER_WHERE` 末尾（关键词之前或之后都行）加一段：`:maintenance` 为空不限制；否则按上面的门槛对 `r.pushed_at` 判断，三档互斥，解析不了的哪一档都不属于。门槛用参数绑定（例如 `:active_secs`、`:stale_secs`，由常量天数 × 86400 算出）。参数要加进 `filter_params!` 宏和 `query_items` 里手写的那份参数列表（**两条取数路径**都要有），否则会报「缺参数」。反过来，rusqlite 对 SQL 里没出现的命名参数也会报错，所以每个新参数名都必须写进 `FILTER_WHERE` 的文本里（计数、总数、两条取数用的都是它）。
4. `list_library_view_at(path, filters, now: &str)`：和 `list_library_view` 做同样的事，只是当前时间由调用方给；`now` 不是合法时间 → `Err`（照 `get_today_view` 的写法）。`list_library_view(path, filters)` 取当前 UTC 时间后调用它。
5. 计数：`LibraryCounts` 新增 `maintenance: MaintenanceCounts { active, slow, stale }`（`#[serde(rename_all = "camelCase")]`，三个 `i64`），用 `cleared_maintenance()` 算（自己不算自己，和 `reading` 一样），其它条件（场景、状态、整理状态、类型、关键词）照常联动；没有状态的项目不计入任何一档。`total`、`total_count`、其它维度的计数规则**不变**。
6. `LibraryItem` **不加字段**（标签由前端用 `pushedAt` 和同一套门槛算）。

### 2. 前端：门槛与标签
1. `lib/contracts.ts`：`export type MaintenanceTier = 'active' | 'slow' | 'stale';`；`LibraryFilters.maintenance?: MaintenanceTier | null`；`LibraryCounts.maintenance: { active: number; slow: number; stale: number }`（注释写清：按其他筛选算、不按自己算；没有推送时间的不计）。
2. 新建 `lib/maintenance.ts`：常量 `MAINTENANCE_ACTIVE_DAYS = 90`、`MAINTENANCE_STALE_DAYS = 365`；`MAINTENANCE_TIERS`（固定顺序 active, slow, stale）；`MAINTENANCE_LABELS`（活跃 / 放缓 / 疑似停更）；`MAINTENANCE_HINTS`（上面三句）；`maintenanceOf(pushedAt, now): MaintenanceTier | null`（`now` 接受 `Date | number | string`，规则同上：空 / 坏日期 → `null`，未来 → `active`，毫秒差比较）。
3. 新建 `components/library/maintenance-tag.tsx`：`MaintenanceTag({ tier, showActive })`：`tier` 为 `null` 返回 `null`；`showActive` 为假（列表行用）时 `active` 也返回 `null`；用 `Tag`，色调 active = `blue`、slow = `gray`、stale = `red`；外面包一层 `inline-flex shrink-0` 并设 `title` 为悬停文案（仿 `ThinTag`）。
4. `item-row.tsx`：`ItemRow` 新增可选 `now?: Date | number | string`（默认 `new Date()`）；在「语言点 + 场景标签」那一行末尾放 `<MaintenanceTag tier={maintenanceOf(props.item.pushedAt, now)} showActive={false} />`。其它内容、布局一概不动。
5. `item-detail.tsx`：「项目最近更新：…」后面放 `<MaintenanceTag tier={maintenanceOf(detail.pushedAt, now)} showActive />`（`pushedAt` 为空时不画）。

### 3. 前端：筛选
1. `library-filters.ts`：`LibraryUiState.maintenance: MaintenanceTier | null`，`createLibraryUiState()` 默认 `null`；`toFilters` 选了才带 `maintenance`（没选不带这个键）；新增 `toggleMaintenance(state, tier)`（再点同一档取消）；`clearFilters` 清掉它；`hasActiveFilters` 把它算上；`libraryEmptyNote` 里「点了别的筛选」的判断把它算上；新增 `maintenanceOptions(counts)`：返回固定顺序（活跃、放缓、疑似停更）的 `{ tier, label, hint, count }`，`counts` 缺失或某个数不是有限数字都当 0。
2. `filter-panel.tsx`：在「类型」后面新增一组「维护状态」（`GroupTitle` + 三个 `FilterButton`，`title` 用悬停文案，个数为 0 的淡一点这条沿用 `FilterButton` 现有逻辑）。`FilterPanel` 新增 props：`maintenance: MaintenanceFilterRow[]`（类型放在 `library-filters.ts` 里导出）、`maintenanceTier: MaintenanceTier | null`、`onMaintenance: (tier) => void`。
3. `pages/v2/library.tsx`：按上面的接线清单接好；点「清除筛选」会一起清掉。

## 不要做
- 不做「已归档」（没有数据）、不做「找方案命中停更工具时提示」、不做字母评分。
- 不加迁移、不改同步、不加依赖、不运行 `pnpm install`、不联网、不调用任何 AI。
- 不改 `library.rs`、`lib.rs`、`v2_ops.rs`、`solve*.rs`、`discover.rs`；不改 `today.tsx`、`discover.tsx` 等其它页面；不碰卡 50 的相关度打分代码（只在它取数时多带新参数）。
- 不删除、不放宽任何已有测试的断言（契约测试往期望里**加**新键除外）。

## 验收
1. `git diff --stat` 的文件都在「允许改动」里。
2. `tools/cards/run.sh web-build`、`web-test`、`rust-fmt`、`rust-check`、`rust-test` 全部通过。**基线（集成线 6452ca8，总控已测）：Rust 1421 通过 / 0 失败 / 5 忽略（总控完整跑，含沙盒里不跑的 1 个钥匙串用例；你在沙盒里用 `run.sh rust-test` 应看到 1420 通过 / 1 被过滤）；前端 37 个文件 869 项。**。你的结果 = 基线 + 你新增的测试数，已有测试一项不少。
3. 测试清单（每条一个测试，报告里写「第 N 条 → 测试名」）：

**Rust**（固定 `now = 2026-10-22T12:00:00Z`，每个 `pushed_at` 一个仓库，用 `list_library_view_at`）
- R1 边界表：`pushed_at = now` → 活跃；`now + 1 天` → 活跃；`2026-07-24T12:00:00Z`（正好 90 天）→ 活跃；`2026-07-24T11:59:59Z`（90 天 + 1 秒）→ 放缓；`2025-10-22T12:00:00Z`（正好 365 天）→ 放缓；`2025-10-22T11:59:59Z`（365 天 + 1 秒）→ 疑似停更；五年前 → 疑似停更；NULL、空串、纯空白、`not-a-date` → 哪一档都不是。
- R2 每一档的筛选结果恰好是该档的仓库；不带 `maintenance` 时全部出现（含没有状态的）；`total_count` 对得上。
- R3 计数：不筛时 `counts.maintenance` 是三档各自的个数（没有状态的不计入）；选了 `stale` 后三个数**不变**（自己不算自己），`total_count` = 疑似停更的个数，而 `counts.reading / cardState / forms / scenarios` 变成只算疑似停更这个子集。
- R4 叠加：`maintenance` + 关键词 + 场景 + 状态组 + 类型 = 交集；`counts.maintenance` 随关键词、场景等联动。
- R5 校验：`"dead"`、`"Active"` → `Err("维护状态筛选无效")`；`None`、`""`、`"  "` 都当不筛。
- R6 顺序：`maintenance` 筛选配 `starred / stars / updated` 三种排序，顺序 = 不带 `maintenance` 时把不符合的去掉后的顺序。
- R7 `list_library_view`（不带 `now`）用真实时钟：用 `time` 取当前时间，造一个「现在 − 1 天」和一个「现在 − 400 天」的仓库，分别在活跃 / 疑似停更里。
- R8 序列化：`counts` 的键 = `total, scenarios, reading, cardState, forms, maintenance`；`counts.maintenance` 的键 = `active, slow, stale`，都是数字；更新既有的契约测试（加新键）。
- R9 不带 `maintenance` 时，既有的全部筛选 / 计数 / 排序 / 关键词测试原样通过。

**前端**
- F1 `maintenance.test.ts`：和 R1 同一张边界表（用 ISO 串和毫秒构造）；`null` / `undefined` / `''` / 空白 / 坏日期 → `null`；`now` 接受 `Date`、数字、ISO 串；常量 = 90 / 365；三档标签与悬停文案。
- F2 `library-filters.test.ts`：默认 `maintenance === null`；`toFilters` 选了才带、没选不带这个键；`toggleMaintenance` 再点取消、点另一档切换；`clearFilters` 清掉；`hasActiveFilters` 为真；`libraryEmptyNote` 只选了 `maintenance` 时是「这一类还没有。」；`maintenanceOptions` 顺序固定、个数正确、缺失或脏数据当 0。
- F3 `filter-panel.test.tsx`：出现「维护状态」组标题和三项及个数；选中项 `aria-pressed="true"`；点击回调带对应档；个数为 0 且未选中的淡一点；`countsKnown = false` 时个数位显示「…」；每项有悬停提示。
- F4 `item-row.test.tsx`：放缓 → 有「放缓」；疑似停更 → 有「疑似停更」；活跃 / `pushedAt = null` / 坏日期 → 没有任何维护标签；行里原有内容（名字、一句话、场景标签、星数、状态标签）不变。
- F5 `item-detail.test.tsx`：三档都画对应的小标签；`pushedAt = null` 不画；有悬停说明。
- F6 `maintenance-tag.test.tsx`：各档文案和色调（活跃 blue、放缓 gray、疑似停更 red）；`showActive = false` 时活跃返回空；`tier = null` 返回空；`title` 是悬停文案。
- F7 `api.test.ts`：`list_library_view` 的参数原样带上 `maintenance`（仿该文件里 `cardState` 那条）。

4. 改坏检查（各做一遍，写「改了什么 → 哪条测试失败」，做完还原）：① SQL 里「活跃」的 `<=` 改成 `<`；② 「放缓 / 疑似停更」分界的 `<=` 改成 `<`；③ `counts.maintenance` 改成用带 `maintenance` 的完整条件算（自己算自己）；④ 没有推送时间的算进疑似停更；⑤ 未来时间不算活跃；⑥ 前端 `MAINTENANCE_ACTIVE_DAYS` 90 改成 60；⑦ `toFilters` 不再带 `maintenance`；⑧ 列表行给活跃也画标签；⑨ `clearFilters` 不清 `maintenance`；⑩ `maintenanceOptions` 的顺序改成疑似停更在前；⑪ `uiKey` 里去掉 `maintenance`（页面源码里的守卫：如果没有现成测试能抓住，在 `library-filters.test.ts` 加一条读 `library.tsx` 源码的守卫测试，仿该仓库其它「源码守卫」的写法）。
5. 报告里写：门槛常量在哪个文件:行；新增的 SQL 子句在哪；`maintenance` 接线的每一处（文件:函数）；`list_library_view_at` 在哪。

## 停止条件
- 需要迁移或改同步才能做 → 停手写进报告。
- 这个 SQLite 版本里 `strftime('%s', …)` 的整数秒方案行不通 → 停手写进报告，不要退回 `julianday` 小数相减。
- 要改「允许改动」以外的文件 → 停手写进报告。
