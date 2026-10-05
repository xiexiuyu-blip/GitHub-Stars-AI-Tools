---
编号: 39-budget-ledger
允许改动:
  - apps/desktop/src-tauri/src/jobs.rs
  - apps/desktop/src-tauri/src/card_handler.rs
  - apps/desktop/src-tauri/src/spend_guard.rs
  - apps/desktop/src-tauri/src/lib.rs
---

# 卡 39 · 每天额度只算「自动」的：一次性整理用的不占额度；待整理数拆开（后端）

## 为什么有这张卡
2026-10-05 Fox 在开发版「设置 · 自动整理与备份」里看到「今天的额度用完了」，抱怨「额度用完就不能动了，可以有提示，但不能完全不给用」。查下来是**账算错了**：

- 每天额度按「最近 24 小时里已结束任务的 token 之和」算（`jobs::tokens_used_since`）。用它的有三处：认领任务 `claim_next`、状态 `queue_status`、以及旧功能和「找方案」AI 精选的预留 `spend_guard::reserve_with`。
- 昨晚（10-04 20:17–23:01）那一轮「一次性全部整理」（Fox 亲手确认的、本来就「不受每天额度限制」）用了约 262 万 token，也被算进了「最近 24 小时」。每天额度只有 60 万，于是整理完后的 24 小时里：新收藏同步后排进队列的任务被拦住（此刻就有 5 个 `repo_card` 排着不动，`budgetRemaining = 0`）；「找方案」的 AI 精选也会被拒绝、退回本地排序。
- 另外界面说「还有 500 个收藏没有中文说明」，其实 461 个是旧格式（card-v2）的说明，真没有的只有约 39 个。后端只给了一个合并的 `needsOrganizing`，前端没法分开说，也没有「在等整理的总数」（排着队但不属于这两类的，比如 README 有实质变化被重新排队的，前端看不到，于是额度用完时界面会说「全部整理好了」而实际有任务排着）。

这张卡只改后端：**一次性整理那段时间里的用量，不再计入每天额度**；并把待整理数拆开、加上「在等整理的总数」。界面怎么改在卡 40，两张卡并行做，**字段名以本卡为准**。

## Fox 能看到的效果（卡 40 一起合并后）
- 一次性全部整理做完以后，每天自动整理和「找方案」的 AI 精选照常能用，不会被「额度用完」拦一整天；
- 每天额度的「已用 / 还剩」只算自动整理和别的小动作，不含一次性整理；
- 整理页能准确说「新版说明 X 个、旧版说明 Y 个、还没有说明 Z 个」，并且额度用完时也看得到「还有 N 个在等」。

## 背景与先读什么
- `apps/desktop/src-tauri/src/jobs.rs`：
  - `tokens_used_since`（约 988 行）：最近 24 小时用量的口径（已结束的看 `finished_at`，排队 / 进行中但已预留 token 的看 `updated_at`）。
  - `claim_next`（约 355–400 行）：认领时 `used = tokens_used_since(24h)`，`spending_open = unlimited || (budget > 0 && used + CLAIM_RESERVE_TOKENS <= budget)`；`BudgetGate`（约 164 行）是它的入参，`Worker` 里构造它的地方在约 846 行。
  - `queue_status`（约 610 行）、`queue_state` 和 `budget_remaining`（约 1252–1295 行）。
  - 全量整理的开始 / 收尾：`begin_full_scan`、`settle_full_scan`、`stop_full_scan`、`finish_full_scan`（约 1062–1200 行）；控制文件的读写 `control_from_map`、`with_control_file`、`reset_spending_authorization`（约 1180、1307–1370 行）。`QueueControl` 里有 `full_scan`（正在进行）和 `last_full_scan`（上一轮，只留一条）。
- `apps/desktop/src-tauri/src/spend_guard.rs` 约 540–600 行 `reserve_with`：同样用 `jobs::tokens_used_since` 算 `used`，再交给 `refusal_for` / `refusal_for_rerank`。
- `apps/desktop/src-tauri/src/card_handler.rs`：`AutomationStatus`（约 395 行）、`count_needs_organizing`（约 420 行）、`automation_status`（约 440 行）、`outdated_sql`、`NOT_CONFIRMED_SQL`；`lib.rs` 里 `get_ai_automation` 直接序列化它。
- 本卡之前的取证：开发库里最近 24 小时 `ai_jobs` 合计 3,082,817 token，其中 10-04 20:17–23:01（UTC 12:17–15:01）那一轮一次性整理 2,617,917；控制文件 `lastFullScan` 记着这段时间。

## 要做的事

### 1. 每天额度的口径：排除一次性整理的时间段
1. 在控制文件里增加一个新键 `recentFullScans`：一个数组，每项 `{ "startedAt": "…Z", "finishedAt": "…Z" }`。`finish_full_scan`（整理完 / 被停 / 到时间 / 超刹车线 / 失败太多，所有收尾的路径）收尾时追加这一轮的时间段；追加时顺手清掉 `finishedAt` 早于「现在 − 25 小时」的项，最多留 16 项。老文件没有这个键 = 空数组，必须能读（读不懂的项丢掉，不影响别的键）。`reset_spending_authorization`（换库 / 恢复备份）要把它一起删掉（用量记录跟着库变了）。
   - 为什么不能只靠已有的 `last_full_scan`：它只留最近一条。例如 14:00–16:45 一轮大的整理，17:00 又点一次「现在整理 3 个」，`last_full_scan` 被小的覆盖，大的那段就又被算回每天额度。
2. 新增 `jobs` 里的一个函数（名字自定，建议 `daily_tokens_used(conn, control, now)`），返回「每天额度口径的用量」= 最近 24 小时用量（和 `tokens_used_since` 同口径）**减去**落在「一次性整理时间段」里的那些任务的 token。时间段包括：正在进行的 `control.full_scan`（从它的 `startedAt` 到现在）、`recentFullScans` 里的每一段、以及 `control.last_full_scan`（`startedAt`–`finishedAt`；这样老控制文件、还没有 `recentFullScans` 的也立即生效）。
   - 一条任务只要落在**任意一段**里就整条排除，**同一条落在两段里也只排除一次**（不能重复减，结果不能小于 0）。
   - 判断一条任务「在不在时间段里」的时间，和 `tokens_used_since` 用的是同一个：已结束的用 `finished_at`，排队 / 进行中已预留 token 的用 `updated_at`。段的两端都含。
   - 最好直接在 SQL 里用 `NOT (…)` 过滤一遍求和，不要两次求和再相减后又要处理重叠。
3. 三处改用它：
   - `claim_next` 的 `used`：`BudgetGate` 目前是 `Copy`、不带控制文件；需要把「要排除的时间段」带进 `BudgetGate`（或者让 `claim_next` 自己读控制文件，二选一；若 `BudgetGate` 因此不能再 `Copy`，同步改用到它的地方）。**现有测试的期望不准改弱**，只能因为字段新增而补充构造参数。
   - `queue_status`：`budget_remaining` 和 `queue_state`（`budget_reached`）改用新口径。`tokens_24h` **保持原样**（还是真实的最近 24 小时总用量，界面里「最近 24 小时用了多少」要说真话），另加 `daily_tokens_used: i64`（新口径，序列化成 `dailyTokensUsed`）。
   - `spend_guard::reserve_with`：旧功能的 `used` 改用新口径（`rerank_tokens_used_since` 只算「找方案」自己的那份，不动）。
4. 全量整理自己的计数（`tokens_used_since(&scan.started_at)`：已用 token、刹车线）**不动**——那是这一轮自己的账。

### 2. `AutomationStatus` 新增字段（`card_handler.rs`，camelCase 序列化）
| Rust 字段 | JSON 键 | 含义 |
|---|---|---|
| `daily_tokens_used: i64` | `dailyTokensUsed` | 每天额度口径的已用（上面第 2 点）。 |
| `active_repos: i64` | `activeRepos` | 有效收藏总数（`sync_status = 'active'`）。 |
| `needs_organizing_missing: i64` | `needsOrganizingMissing` | 有效收藏里**没有理解卡**的个数（口径和 `count_needs_organizing` 的第一个分支一致，同样不含已被用户确认的）。 |
| `needs_organizing_outdated: i64` | `needsOrganizingOutdated` | 有卡但是**旧格式 / 旧提示词版本**、且没被确认的个数（`outdated_sql()`）。 |
| `awaiting: i64` | `awaiting` | **现在在等整理**的收藏个数 = 「没有卡」+「旧格式」+「已经有 `repo_card` 任务排着队或在进行的」，**按仓库去重**（一个仓库既没卡又排着队只算一个）。要和 `start_full_scan` 一次性会处理的集合一致：它会补排 Missing 和 Outdated，再数 `count_open_cards`。 |

- `needs_organizing`（已有）保持不变，并且 `needs_organizing == needs_organizing_missing + needs_organizing_outdated`（加一条测试）。
- `automation_status` 里这些都在读状态时现算；失败时沿用现有的错误返回方式，不要 `unwrap`。

### 3. 不改的
- `start_full_scan` / `stop_full_scan` 的流程、确认门槛（`confirmed`）、48 小时、刹车线、失败刹车线——全都不动。
- 数据库结构：**不新增迁移**。时间段记在控制文件里。

## 测试（Rust，每条都要有；写在对应模块的 `#[cfg(test)]` 里）
1. **额度口径**：造三条已结束任务：A（窗口外，100 token）、B（在一段一次性整理时间段里，5000 token）、C（窗口外，200 token）→ 新口径用量 = 300，`tokens_24h` = 5300，`budget_remaining = budget − 300`（按现有「放不下一条预留就当 0」的规则）。
2. **大轮之后小轮**：先记一段大的（含 300 万 token 的任务）、再记一段小的（`last_full_scan` 被小的覆盖）→ 大的那段仍被排除（靠 `recentFullScans`）。
3. **重叠只排除一次**：一条任务同时落在 `last_full_scan` 和 `recentFullScans` 的同一段（或两个重叠的段）里，只减一次，用量不为负。
4. **进行中的一轮**：`control.full_scan` 正在进行时，从它开始后结束的任务不计入。
5. **窗口外照常限额**：自动整理自己用了 40k、额度 50k → `claim_next` 在剩余放不下一条预留时不再认领、`state == "budget_reached"`；而一次性整理时间段里再多的用量不影响它认领。
6. **老控制文件**：只有 `lastFullScan`、没有 `recentFullScans` 的文件，`last_full_scan` 那段照样被排除；别的键（令牌、`lastReadmeCheck` 等）在 `finish_full_scan` 写入后原样保留。`reset_spending_authorization` 之后 `recentFullScans` 没了。
7. **清理**：追加时早于 25 小时的段被清掉；超过 16 项只留最新 16 项。
8. **`spend_guard`**：一次性整理刚结束、用量很大的情况下，旧功能 / 「找方案」预留不再因为一次性整理的用量被拒；自动整理自己的用量超额度时仍然拒（保持原提示文案）。
9. **`AutomationStatus` 的 JSON 键测试**（`card_handler.rs` 里现有那条按键名断言的测试）加上 5 个新键；旧键一个不少。
10. **拆分计数**：造库——A 没有卡；B 旧格式（`card_schema_version = 1`）；C 现行格式；D 旧格式但 `review_status = confirmed`（不计入）；E 已取消收藏（`sync_status != 'active'`，不计入）；F 现行格式但有 `repo_card` 排队（计入 `awaiting`，不计入 missing / outdated）；G 没有卡又排着队（`awaiting` 里只算一次）→ 断言 `needs_organizing_missing`、`needs_organizing_outdated`、`needs_organizing`、`awaiting`、`active_repos` 的具体数。

## 不要做
- 不动前端、不新增依赖、不新增数据库迁移、不改提示词、不改 `lib.rs` 里命令的名字和参数（只在必要时跟着字段改）。
- 不放宽、不删现有断言；期望必须改的（例如 `AutomationStatus` 的键列表）在报告里点名。
- 不碰正式库 / 开发库的数据目录和钥匙串；测试只用临时目录。

## 验收
1. `tools/cards/run.sh rust-fmt`、`rust-check`、`rust-test`（可带过滤词）通过；沙盒里跑不了的测试如实写进报告，总控在沙盒外跑完整套。
2. `git diff --stat` 只出现「允许改动」里的文件。
3. 报告里写：条号 → 文件:行；新增 / 改动的测试名；`BudgetGate` 怎么带上时间段、是否去掉了 `Copy`；没把握的地方。

## 停止条件
- 要新增迁移、要改前端、或要动「允许改动」以外的文件才能做完——停手并写进报告。
