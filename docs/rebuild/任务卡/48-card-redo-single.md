---
编号: 48-card-redo-single
允许改动:
  - packages/storage/migrations/fox/037_ai_job_spend_authorization.sql
  - apps/desktop/src-tauri/src/db/migrations.rs
  - apps/desktop/src-tauri/src/jobs.rs
  - apps/desktop/src-tauri/src/card_handler.rs
  - apps/desktop/src-tauri/src/lib.rs
  - apps/desktop/src/components/library/item-detail.tsx
  - apps/desktop/src/hooks/use-regenerate.ts
  - apps/desktop/src/lib/item-format.ts
  - apps/desktop/src/lib/api.ts
  - apps/desktop/src/lib/contracts.ts
  - apps/desktop/src/**/*.test.ts
  - apps/desktop/src/**/*.test.tsx
  - tools/dev-ui/review-harness/review_dump.rs
---

# 卡 48 · 详情页「重新整理这一张」：Fox 点确认才花 token，只开这一张的闸（后端 + 前端）

> 执行者：Grok（见分配单；两轮做不出来由总控接手）｜**这是整个项目里最敏感的一张卡：它给「每日额度是硬上限」开第二个例外。** 先读 `AGENTS.md` 里「每日 AI 额度是硬上限」那一段（已经写明了这个例外的边界），再动手。不改 `spend_guard.rs`、不改全量整理、不改每日额度的计算。

## Fox 能看到的效果
1. 打开任意一个**已经有中文说明**的工具详情页：说明下面有两种入口——还没确认过的显示「上面这段说明说得对吗？ 对 ｜ 不对，请 AI 重写（会花一点钱）」（原来就有）；已经点过「对」的显示「✓ 你确认过了 · 重新整理这一张」（新增）。
2. 点「不对，请 AI 重写」或「重新整理这一张」，**先不花钱**，只在原地展开一个确认条：「重新整理这一张？让 AI 重新读一遍这个工具的项目说明，写一份新的中文说明，大约花 1 万 token。失败了会自动再试，最多再试 2 次，每次试都可能再花一点。」（点过「对」的再加一句：「你之前点过「对」，重新整理后会换成新的说明，需要你再看一眼。」）两个按钮：「确认，重新整理」「先不了」。
3. 点「确认，重新整理」才真的开始：提示「已开始重新整理这一张。一般一两分钟，好了以后重新打开这一张，就能看到新的说明。」**哪怕「每天自动整理」的额度是 0（关着）也会做**——但只做这一张，别的排着队的任务照旧不动。
4. 自动整理被暂停时：点确认后提示「自动整理现在是暂停的，先到「自动整理」里恢复，再重新整理这一张。」，并且**什么都不会写进数据库**。没配置 AI 服务时同理。

## 设计（先读，这是 Fox 和总控定好的，不要另起炉灶）
- 花钱的决定只能由 Fox 本人点：命令 `regenerate_library_card` 必须带 `confirmed: true`，没带就拒绝、数据库一个字不动（和 `start_full_scan` 同一个做法，参考 `card_handler.rs` 的 `start_full_scan`）。
- **授权存在这一张的任务行上**：新增一列 `ai_jobs.spend_authorized_until TEXT`（迁移 037）。`request_regenerate` 在**一个 `BEGIN IMMEDIATE` 事务**里：检查 → 把这张卡改回「待确认 + 来源哈希清空」（原有行为）→ 给这一张的 `repo_card` 任务行写上「现在 + 30 分钟」的到期时间。
- 队列认领 `jobs::claim_next` 只对「带着**有效授权**（到期时间 ≥ 现在）的那一行」绕过每日额度；其它行仍然按每日额度；**暂停照样拦**（`budget_gate.paused` 那道最前面的检查不动）。
- 队列任务本来就不过 `spend_guard`（那是旧版 AI 功能的闸），所以**不用改 `spend_guard`**。已核对：`ai.rs` 里 `check_before_request` 只在 `request_openai_with` / `request_openai_streaming` / `request_anthropic_with` / `request_anthropic_streaming` 里调用，队列走的是 `ai::chat_json` → `post_openai` / `post_anthropic`，不经过它们。
- 这个授权**不是**一个开关，不进控制文件（`queue.json`）、不放进全量整理的时段、不改 `BudgetGate` 的 `unlimited`。花多少有天然上限：一行任务最多 `max_attempts`（3）次尝试，每次认领预留 12000（`CLAIM_RESERVE_TOKENS`）。
- 授权过期不会花钱：过了 30 分钟还没被认领（比如期间暂停了），这一行就变回普通排队任务，等每日额度；每日额度是 0 就一直排着，不花钱。

## 背景与先读什么
- `apps/desktop/src-tauri/src/jobs.rs`：
  - `claim_next`（约 390–520 行）：先判断暂停；`BEGIN IMMEDIATE`；算 `used` 和 `spending_open`；按 `spends_tokens` 把任务类型分进 `eligible`；拼 SQL 选一行；`UPDATE … status='running'` 并记预留。**注意第一次 `if eligible.is_empty() { …return Ok(None) }`：预算关着、处理器又都是花钱的类型时，`eligible` 为空会直接返回——新逻辑要让带授权的行仍有机会被选到。**
  - `ClaimCandidate { job_type, spends_tokens }`、`BudgetGate`、`JobRecord`（约 114–173 行）；`CLAIM_RESERVE_TOKENS`；`shift_db_time` / `now_db_time`（需要时改成 `pub(crate)`）。
  - 认领的既有测试（约 1789–2000 行，`free_and_spend()`、`gate()`、`claim_one()` 这些夹具）：照它们写新测试。
  - `claim_for_worker`（约 876 行）怎么构造 `BudgetGate`：**这里不用改**。
- `apps/desktop/src-tauri/src/card_handler.rs`：`request_regenerate`（约 325–360 行，`REGENERATE_PRIORITY = 2`）；`start_full_scan`（约 513–543 行，`confirmed` 和 `provider_from_settings(settings)` 的写法）；既有 `request_regenerate` 测试（约 1985–2082 行，会因为签名变化要改）。
- 表 `ai_jobs`（`packages/storage/migrations/fox/025_ai_jobs.sql`）：**有一个唯一索引** `idx_ai_jobs_active_target ON ai_jobs(job_type, target_type, target_id) WHERE status IN ('queued','running')`——同一个仓库同时只能有一条排队 / 进行中的 `repo_card`。原来的 `request_regenerate` 用 `INSERT OR IGNORE`，仓库已经有一条排队的补旧账任务（优先级 6）时会被忽略，优先级也不会提到 2。新逻辑要改成：先 `UPDATE` 已有的 `queued` 行（见下），没有再 `INSERT`。
- 迁移：`apps/desktop/src-tauri/src/db/migrations.rs` 的 `MIGRATIONS`（最后一项是 036）；照 `packages/storage/migrations/fox/036_solve_rerank_cache.sql` 和 `035_readme_basis.sql` 的格式写 037（SQL 文件自己 `INSERT INTO schema_migrations(version, name) VALUES('037', '…')`，**不许用 `INSERT OR IGNORE`**）。
- 前端：`apps/desktop/src/components/library/item-detail.tsx`（`DetailBody` 约 80 行起，带 `key={detail.id}`，换工具会重置状态；说明卡片约 170–195 行）；`hooks/use-regenerate.ts`；`lib/item-format.ts` 约 385–400 行（旧的 `regenerateToast`）；`lib/api.ts:162`（`regenerateLibraryCard`）；`lib/contracts.ts:266`（命令说明）；三个调用点 `pages/v2/today.tsx`、`pages/v2/library.tsx`、`components/solve/hit-drawer.tsx` 都经过 `ItemDetail` 的 `onRegenerate`，**这三个文件不改**。
- 确认条的写法参考 `pages/v2/toolbox.tsx` 的 `askFullScan` / `startFullScan` 和 `pages/v2/toolbox.test.tsx` 前 20 行：「首次点击只展开确认，不调用命令；确认后才调用」，测试里用 `?raw` 读源码做守卫。
- `tools/dev-ui/review-harness/review_dump.rs:209`：总控用的真后端桥，里面直接调用 `card_handler::request_regenerate`，签名变了要同步改（参考同文件里 `start_full_scan` 那一支怎么传 settings 和 `confirmed`）。

## 要做的事

### A. 迁移 037
新建 `packages/storage/migrations/fox/037_ai_job_spend_authorization.sql`：
```sql
ALTER TABLE ai_jobs ADD COLUMN spend_authorized_until TEXT;
INSERT INTO schema_migrations(version, name) VALUES('037', 'ai_job_spend_authorization');
```
（上面是内容的意思，格式、注释照 036 写。）在 `db/migrations.rs` 的 `MIGRATIONS` 里登记（名字与 SQL 里一致，`include_str!` 路径照旁边的写），并更新既有测试里对「最后一个版本 / 迁移个数」的断言。仓库里如果有「必需表字段检测」之类列出 `ai_jobs` 各列的地方（搜 `ai_jobs` 的列名），同步加上新列；没有就不用。**这是可空列，已有的行都是 NULL，等于没授权。**

### B. 认领（`jobs::claim_next`）
1. 预算关着时，花钱的任务类型不再整体排除，而是放进「需授权」一组；不花钱的、或预算开着的花钱类型放进「直接可领」一组。两组都为空才提前返回 `Ok(None)`。
2. 选行的 SQL 条件改成：`status='queued' AND (not_before IS NULL OR not_before <= ?now) AND ( job_type IN (直接可领…) OR ( job_type IN (需授权…) AND spend_authorized_until IS NOT NULL AND spend_authorized_until >= ?now ) )`；排序、`LIMIT 1`、预留、`UPDATE … WHERE id=? AND status='queued'` 都不变。比较用字符串（两边都是 `format_db_time` 的固定格式，和现有的 `finished_at >= cutoff` 一样）。
3. 其余不变：暂停照样最先拦；预留照记（带授权的行认领后也记 `CLAIM_RESERVE_TOKENS`，它会被算进「最近 24 小时已用」）；`full_scan` 逻辑一个字不动。
4. 所有「先读后写」的事务继续用 `TransactionBehavior::Immediate`（卡 39 的教训：延迟事务会 `database is locked`）。

### C. 请求重做（`card_handler::request_regenerate`）
签名改成 `request_regenerate(db_path: &Path, settings: &Value, repo_id: &str, confirmed: bool) -> Result<(), String>`，按这个顺序，**前三步失败时数据库一个字都不能动**：
1. `!confirmed` → `Err("重新整理这一张会花 token，需要你在界面上确认之后才能开始。")`。
2. 没配置 AI 服务（`provider_from_settings`，错误文字照 `start_full_scan` 那句）→ Err。
3. 队列暂停（读控制文件 `jobs::read_control(&jobs::queue_control_path(db_path)).paused`）→ `Err("自动整理现在是暂停的，先到「自动整理」里恢复，再重新整理这一张。")`。
4. 开 `BEGIN IMMEDIATE` 事务：仓库不存在 / 还没有理解卡 → 原来的错误；这个仓库已经有一条 `running` 的 `repo_card` → `Err("这一张正在整理中，等它结束再试。")`（事务回滚，什么都没改）。
5. 原有的两处写入保留：`card_json` 的 `review_status` 改回 `suggested`、`source_hash = ''`。
6. 授权到期时间 = `shift_db_time(now, 1800)`（常量 `ONE_OFF_AUTHORIZATION_SECS = 1800`，放在 `card_handler.rs` 顶部和 `REGENERATE_PRIORITY` 放一起）。然后：先
   `UPDATE ai_jobs SET priority = MIN(priority, ?REGENERATE_PRIORITY), spend_authorized_until = ?until, not_before = NULL, attempts = 0, error_kind = NULL, error_message = NULL, updated_at = ?now WHERE job_type='repo_card' AND target_type='repository' AND target_id = ?repo AND status='queued'`；
   `changed == 0` 才 `INSERT`（带 `spend_authorized_until`，优先级 `REGENERATE_PRIORITY`，`status='queued'`；`max_attempts` 用表默认值 3）。
7. 提交。

### D. 命令（`lib.rs`）
`regenerate_library_card(app_handle, id: String, confirmed: bool)`：取 `storage` 和 `read_app_settings_value(&app_handle)?`（和 `start_full_scan` 一样），调用 `card_handler::request_regenerate(storage.database_path(), &settings, &id, confirmed)`。注册（约 7710 行）和权限文件不用动（命令名没变）。

### E. 前端
1. `lib/api.ts`：`regenerateLibraryCard(id: string, confirmed: boolean)` → `call('regenerate_library_card', { id, confirmed })`。`lib/contracts.ts:266` 的命令说明同步成新参数、新行为（一两句话）。
2. `lib/item-format.ts`（旧的 `regenerateToast` 不再适用——它说的是「只排队、不一定重写」，现在点确认就一定会做）：
   - 删除 `regenerateToast`，新增并导出：
     - `SINGLE_REDO_ESTIMATE_TOKENS = 10_000` 和 `SINGLE_REDO_EXTRA_TRIES = 2`（文案里的数字从这里来，不要在字符串里手写；万位的写法用 `lib/automation-view.ts` 里已有的 token 格式化函数，没有合适的就在这里写一个小的并加测试）；
     - `redoConfirmCopy(wasConfirmed: boolean): { title: string; body: string; confirmLabel: string; cancelLabel: string }`，文字就是上面「Fox 能看到的效果」第 2 条，标题「重新整理这一张？」，按钮「确认，重新整理」「先不了」；
     - `regenerateStartedNote(): { title: string; sub: string }`：「已开始重新整理这一张」／「一般一两分钟。好了以后重新打开这一张，就能看到新的说明。」；
     - `regenerateFailureNote(message: string): { actionLabel: string | null }`：message 含「暂停」或「还没配置 AI」时 `actionLabel` 是「去自动整理」，其它情况是 `null`。
   - 旧 `regenerateToast` 的那一组测试（`item-format.test.ts` 约 584–640 行 `describe('regenerateToast：…')`）整组删除，**报告里逐条列出被删的用例名和删除原因**；换成上面三个新函数的测试。`automation-view.ts` 里的 `regenerateNote` 现在没人用了，**留着别动**（那个文件另一张卡也在改，别扩大冲突）。
3. `hooks/use-regenerate.ts`：保留 `useRegenerate(onOpenToolbox?)` 的签名；调用 `regenerateLibraryCard(id, true)`（**注释写明：只能在用户点了确认条上的「确认，重新整理」之后调用**）；成功 → 用 `regenerateStartedNote()` 弹提示；失败 → 弹 `describeInvokeError(reason)`，并按 `regenerateFailureNote` 决定是否带「去自动整理」按钮（有 `onOpenToolbox` 才带）。不再调用 `getAiAutomation()`。
4. `components/library/item-detail.tsx`（在 `DetailBody` 里加一个 `useState<boolean>`，例如 `askingRedo`）：
   - 「不对，请 AI 重写（会花一点钱）」按钮的 `onClick` 改成 `setAskingRedo(true)`，**不再直接调用 `props.onRegenerate`**；
   - `cardReview === 'confirmed'` 时，「你确认过了」旁边加一个文字按钮「重新整理这一张」，`onClick` 同样只是 `setAskingRedo(true)`；
   - `askingRedo` 为真时，在说明卡片里展开确认条（用现有的 `Button`、设计令牌，带 `role="group"` 和 `data-redo-confirm`），文字全部来自 `redoConfirmCopy(detail.cardReview === 'confirmed')`；「确认，重新整理」的 `onClick` = 先 `setAskingRedo(false)` 再调用 `props.onRegenerate()`（`data-confirm-redo`）；「先不了」只 `setAskingRedo(false)`；
   - **`props.onRegenerate` 在整个文件里只出现在「确认，重新整理」这一个地方**；
   - 其余界面不改。
5. `tools/dev-ui/review-harness/review_dump.rs`：`regenerate_library_card` 那一支改成传 settings（照同文件 `start_full_scan` 那支）和 `args.get("confirmed")`。

## 测试（每一条都要「改坏就失败」）

### Rust：认领（`jobs.rs` 测试模块，照现有夹具）
1. 预算 0、花钱的任务**没有**授权 → 认领不到（现有行为，确认还在）。
2. 预算 0、同一行 `spend_authorized_until = NOW + 1800s` → 认领到；`record.reserved == CLAIM_RESERVE_TOKENS`；数据库里该行 `status='running'`。
3. 授权到期边界：`NOW - 1s` → 认领不到；正好 `NOW` → 认领得到（`>=`）。
4. `paused = true` → 即使带有效授权也认领不到。
5. **只开这一行**：队列里有两条花钱的任务，一条老的优先级高但没授权、一条带授权 → 第一次认领拿到带授权的，第二次认领返回 `None`。
6. 预算大于 0 但已用满 + 带授权 → 认领得到；不带授权 → 认领不到。
7. 带授权但 `not_before` 在未来 → 认领不到；不花钱的任务类型不受影响（照旧可领）。

### Rust：请求重做（`card_handler.rs` 测试模块，照 `request_regenerate` 既有测试的夹具，临时目录里造库和控制文件）
1. `confirmed = false` → Err；对库做前后对比：`repo_ai_documents.card_json`、`source_hash` 不变，`ai_jobs` 一行都没多。
2. 没配置 AI 服务 → Err，库不变。
3. 控制文件里 `paused = true` → Err，库不变。
4. 正常：产生一条 `repo_card` 任务，`priority == 2`、`status == 'queued'`、`max_attempts == 3`、`spend_authorized_until` 与现在相差 1800 秒（±5 秒），`card_json` 的 `review_status == 'suggested'`、`source_hash == ''`。
5. 该仓库已有一条排队的补旧账任务（优先级 6、`attempts = 2`、有 `not_before`、有旧错误）→ **原地更新**（同一个 id）：优先级 2、授权已写、`attempts == 0`、`not_before IS NULL`、旧错误清空；这个仓库的活动 `repo_card` 任务仍然只有 1 条。
6. 该仓库已有一条 `running` 的 `repo_card` → Err，库不变（`card_json`、`source_hash` 没动，没写授权）。
7. 另一个仓库的排队任务**没被动**（它的 `spend_authorized_until` 仍是 NULL）。
8. 仓库没有理解卡 → Err，库不变，没有任务行。
9. **接起来**：先 `request_regenerate`，再用真的 `jobs::claim_next`（预算 0，`BudgetGate` 用 `daily_budget_tokens: 0, unlimited: false`）认领 → 拿到的正是这个仓库的任务；库里另一条没授权的排队花钱任务认领不到。

### Rust：迁移
- 037 已登记；从 036 升级后 `ai_jobs` 有 `spend_authorized_until` 列，且既有行都是 NULL；最后一个版本是 037。

### 前端
- `item-format.test.ts`：`redoConfirmCopy(false/true)` 的标题、正文（含 `1 万`、`2 次`、点过「对」那句只在 true 时有）、按钮文字；`regenerateStartedNote`；`regenerateFailureNote`（含「暂停」→ 有按钮；含「还没配置 AI」→ 有按钮；「正在整理中」→ 没有）；文案里的数字等于常量。
- `item-detail.test.tsx`（`renderToStaticMarkup` + `?raw` 源码守卫，照该文件现有写法）：初始渲染里**没有** `data-redo-confirm`；`confirmed` 的详情里能看到「重新整理这一张」，`suggested` 的看到「不对，请 AI 重写（会花一点钱）」；源码里 `props.onRegenerate` 恰好出现 1 次、且在「确认，重新整理」这个按钮的处理函数里；两个展开入口的处理函数里没有 `onRegenerate`。
- `api.test.ts` 约 164–167 行改成断言 `{ id: 'repo-1', confirmed: true }`；加一条：`regenerateLibraryCard('repo-1', false)` 传 `confirmed: false`。
- 其它因为签名 / 删除 `regenerateToast` 而失败的测试：只改到能反映新行为，**逐条列在报告里**。

## 不要做
- 不改 `spend_guard.rs`、`ai.rs`、`card_pipeline.rs`、`readme_follow.rs`、`solve_rerank.rs`；不改全量整理（`begin_full_scan` / `settle_full_scan` / `full_scan_*`）和每日额度的计算（`daily_tokens_excluding` 等）；不改 `QueueControl` 和控制文件格式。
- 不把授权放进控制文件、不新增任何「全局」的绕过开关、不让一次确认授权多于一个仓库。
- 不改 `pages/v2/today.tsx`、`pages/v2/library.tsx`、`components/solve/hit-drawer.tsx`、`lib/automation-view.ts`。
- 不改 001–036 迁移文件；不新增依赖；不运行 `pnpm install`。
- 不调用真实的 AI 接口（测试用现有的假客户端 / 直接造库）。

## 验收
1. `git diff --stat` 的文件都在「允许改动」里。
2. `tools/cards/run.sh web-build`、`web-test`、`rust-fmt`、`rust-check`、`rust-test` 全部通过；报告里写前端项数、Rust 通过 / 失败 / 忽略数，**和改动前基线对比**（先在改动前跑一次基线）。
3. 逐条对应上面的测试清单，报告里写「第 N 条 → 测试名」。
4. 改坏检查（各做一遍，写「改了什么 → 哪条测试失败」，做完还原）：① 认领 SQL 去掉 `spend_authorized_until >= ?now` 的条件（变成只要有授权列非空）；② `>=` 改成 `>`；③ 认领里去掉暂停检查；④ `request_regenerate` 里去掉 `confirmed` 检查；⑤ 去掉暂停检查；⑥ 把 UPDATE 改回 `INSERT OR IGNORE`；⑦ 授权写成对所有排队的 `repo_card` 行（去掉 `target_id = ?` 条件）；⑧ 前端把 `onRegenerate` 加回「不对，请 AI 重写」按钮的处理函数里。
5. 报告里列出本卡所有「先读后写」的事务，逐个写出是不是 `Immediate`、依据文件:行。
6. 报告里列出：本卡新增的、会让任务绕过每日额度的代码路径（应该只有 `claim_next` 里那一处），写出文件:行，并说明它的限制条件（有效授权 + 未暂停 + 只这一行）。

## 停止条件
- 要改 `spend_guard.rs`、全量整理、控制文件格式才能做 → 停手写进报告（说明为什么）。
- 迁移测试暗示要改 001–036 任何文件 → 停手写进报告。
- 认领的改动让既有测试（预算、暂停、24 小时窗口、全量整理）失败，且改测试才能过 → 停手写进报告，不要改既有测试的期望。
- 需要新依赖 → 停手写进报告。
