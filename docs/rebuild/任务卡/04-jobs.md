---
编号: 04-jobs
允许改动:
  - apps/desktop/src-tauri/src/jobs.rs
  - apps/desktop/src-tauri/src/lib.rs
  - apps/desktop/src-tauri/src/v2_ops.rs
  - apps/desktop/src-tauri/permissions/gsat-commands.toml
  - apps/desktop/src-tauri/gen/schemas/acl-manifests.json
---

# 卡 04 · AI 任务队列消费者（后台工作线程）

## Fox 能看到的效果
这是后台积木，**这张卡 Fox 暂时看不到界面变化**。现在库里有 401 个排着队的整理任务（`ai_jobs` 表），但没有任何"人"去取它们。做完后：队列有了真正的消费者——按优先级一个个取、失败按退避重试、密钥出错自动停下整条队列、用量超过额度就停手、App 关掉再打开接着干。**而且默认一分钱也不花**：Fox 没有明确设置过「每天最多用多少 token」之前，工作线程永远不会调用任何会花钱的任务。理解卡本身怎么生成是下一张卡（05）的事，这张卡只管"取任务、跑、记账、重试、暂停"。

## 背景与先读什么
- 队列表 `ai_jobs`：`packages/storage/migrations/fox/025_ai_jobs.sql`（状态、`attempts`、`max_attempts`、`not_before`、`error_kind` 的可选值、`input_hash`、token 计数都在里面，**不要改迁移**）。
- 现有队列控制代码在 `apps/desktop/src-tauri/src/v2_ops.rs` 约第 334–440 行：`recover_interrupted_jobs`、`queue_control_path`、`set_queue_paused`、`queue_is_paused`、`list_jobs`、`cancel_job`、`set_daily_budget`、`daily_budget`、`retry_failed_jobs`，以及 `enqueue_missing_cards`（约第 1123 行）。队列控制文件是数据库旁边的 `fox-stars-lab.queue.json`。
- **现有的坑（本卡要修）**：①`set_queue_paused` 每次都把每日预算写死成 200000，等于暂停/恢复一次就把用户设的预算冲掉；②`daily_budget()` 在文件没写时默认 200000，等于默认就允许花钱；③`lib.rs` 里的 `enqueue_missing_cards` 命令（约第 1222 行）在入队前顺手 `set_daily_budget(…, 200_000)`，把"授权花钱"当成了入队的副作用。**预算必须是 Fox 明确设置的，默认是 0（不花钱）。**
- `lib.rs` 里的队列命令（约第 967–1000 行）：`list_ai_jobs`、`pause_ai_queue`、`resume_ai_queue`、`retry_failed_ai_jobs`、`cancel_ai_job`、`set_ai_daily_budget`；`run()` 的 `.setup`（约第 6619 行起）。
- 测试怎么造库：看 `db/migrations.rs` 里 `empty_database_applies_every_migration` 怎么在临时库上跑完整迁移，再用 SQL 插 `github_accounts` 和 `ai_jobs` 行。**所有测试只用临时库，不读写真实数据目录。**
- 时间格式：库里的时间列都是 `strftime('%Y-%m-%dT%H:%M:%fZ','now')` 的 ISO 字符串，同格式字符串可以直接按字典序比较。`time` 库已开 `formatting`，可以用它生成同格式的"现在"。**不允许加新依赖。**

## 要做的事

### 1. 新模块 `src-tauri/src/jobs.rs`（不依赖 Tauri）
1. **队列控制 `QueueControl`**：读写 `fox-stars-lab.queue.json`，字段 `paused: bool`、`dailyBudgetTokens: u64`（**0 = 不花钱**，文件不存在或没这个字段都按 0）、`pauseReason: Option<{kind, message, at}>`。写入用"临时文件 + 改名"保证原子；每次只改自己要改的字段，其他字段原样保留。提供 `read_control`、`set_paused(path, paused)`（恢复时清掉 `pauseReason`）、`set_daily_budget(path, tokens)`（允许 0；上限 2_000_000；不再有 1000 的下限）。
2. **类型**：`ErrorKind`（`Interrupted`/`Timeout`/`RateLimit`/`Auth`/`BadJson`/`Schema`/`Network`/`ContentTooLarge`/`Other`，带 `as_str()`，取值必须和 025 迁移里 `error_kind` 的 CHECK 清单一一对应）；`JobRecord`（`id`、`account_id`、`job_type`、`target_type`、`target_id`、`attempts`、`max_attempts`、`input_hash`）；`JobFailure`：
   - `Retryable { kind, message, retry_after_secs: Option<u64> }`：还有次数就重排，否则失败；
   - `Fatal { kind, message }`：直接失败；
   - `PauseQueue { kind, message }`：认证失败这类——**整条队列停下**，这个任务回到 `queued` 且这次不计入 `attempts`，控制文件写入 `paused=true` 和 `pauseReason`；
   `JobSuccess { input_tokens, output_tokens, provider, model, prompt_version, input_hash, skipped: bool }`；
   `trait JobHandler: Send + Sync { fn job_type(&self) -> &'static str; fn spends_tokens(&self) -> bool; fn run(&self, job: &JobRecord, db_path: &Path) -> Result<JobSuccess, JobFailure>; }`。
3. **认领 `claim_next(conn, now, handled_types, budget_gate)`**：在一个 `BEGIN IMMEDIATE` 事务里选出下一个可跑的任务（`status='queued'` 且 `not_before` 为空或不晚于现在，只挑有处理器的 `job_type`，按 `priority` 升序再按 `created_at` 升序），改成 `running`、`attempts+1`、写 `started_at`。**两个工作线程不能认领到同一个任务**。花钱的处理器（`spends_tokens()==true`）只在下面三条都满足时才会被认领：队列没暂停、每日预算大于 0、最近 24 小时已用 token（已结束任务的 `input_tokens+output_tokens` 之和）小于预算。用滚动 24 小时而不是自然日，避开时区问题；允许最后一个任务让总量略超预算，在文档注释里写明。
4. **收尾**：
   - 成功：`succeeded`（`skipped==true` 时是 `skipped`），写 token、provider、model、prompt_version、input_hash、`finished_at`，清空 `error_kind`/`error_message`；
   - 重排：`attempts < max_attempts` 时回 `queued`，写 `error_kind`/`error_message`，`not_before = 现在 + 延迟`。延迟：给了 `retry_after_secs` 就用它（夹在 5 到 3600 秒之间）；否则按第几次尝试退避：第 1 次失败 30 秒、第 2 次 120 秒、第 3 次及以后 600 秒，再加最多 ±10% 的抖动（抖动来自参数里的一个数，测试里传固定值）。用尽次数：`failed`；
   - 所有收尾的 UPDATE 都带 `AND status='running'`：任务在跑的时候被用户取消了，就不能被覆盖成成功或失败。
5. **工作线程 `Worker`**：`Worker::start(db_path, handlers, config) -> Worker`、`stop(self)`、`status()`。`WorkerConfig { concurrency（1 到 4，默认 2）、idle_poll（默认 2 秒） }`。启动时先把遗留的 `running` 任务改回 `queued` 并记 `error_kind='interrupted'`（和现有 `recover_interrupted_jobs` 同效果）。循环：认领 → 在处理器里跑（用 `catch_unwind` 兜住 panic，当作 `Fatal(Other, "任务内部出错：…")`，**不能让线程死掉**）→ 收尾；没有任务时睡 `idle_poll`。`stop` 要等正在跑的任务自然结束，不再认领新的。处理器自己负责网络超时，worker 不强杀。
6. **状态 `queue_status(db_path)`（不需要 worker 在跑也能读）**：`state`（`running`/`idle`/`paused`/`budget_reached`/`stopped`）、`pauseReason`、`queued`、`running`、`succeeded24h`、`failed`、`tokens24h`、`dailyBudget`、`budgetRemaining`。

### 2. 修现有代码
- `v2_ops.rs`：`set_queue_paused`、`queue_is_paused`、`set_daily_budget`、`daily_budget`、`queue_control_path` 改成调用 `jobs.rs` 里的新实现（函数签名保持不变，旧调用方不用改）。**只准动这五个函数**，其余一概不动。
- `lib.rs`：`enqueue_missing_cards` 命令里**删掉** `set_daily_budget(…, 200_000)` 这一行，入队不再有授权花钱的副作用；`resume_ai_queue` 恢复时清掉 `pauseReason`；新增命令 `get_ai_queue_status`（返回上面的状态）并登记到 `generate_handler!` 和 `permissions/gsat-commands.toml`；`.setup` 里调用一个 `jobs::start_default_worker(db_path)`：目前**还没有任何处理器**，所以它什么都不做（返回 `None`），留给下一张卡注册处理器。
- 旧的 `list_ai_jobs`、`retry_failed_ai_jobs`、`cancel_ai_job`、`set_ai_daily_budget` 命令保持可用；`set_ai_daily_budget` 现在允许传 0。

## 不要做
- 不改迁移、不改 `ai.rs`、不碰钥匙串、不调用任何真实的 AI 接口；不新增依赖；不改前端。
- 不要在这张卡里写"理解卡"的处理器（那是卡 05 和后面的接线卡）。测试里的处理器全是假的。
- 不要读写真实数据目录。

## 验收
1. `tools/cards/run.sh rust-test` 通过，`jobs.rs` 新增测试**至少 20 个**，覆盖：认领顺序（优先级、创建时间、`not_before` 未到的不取、没有处理器的类型不取）；4 个线程抢 50 个任务**没有重复认领**；预算为 0 时花钱任务不被认领而不花钱的任务照跑；最近 24 小时用量达到预算后停止认领（24 小时前结束的任务不计入）；暂停时不认领；成功、skipped 的收尾字段；`Retryable` 的退避时间表（30/120/600 秒、`retry_after_secs` 的夹取、抖动参数）和用尽次数后变 `failed`；`Fatal` 直接失败；`PauseQueue`（任务回 `queued`、`attempts` 不增加、控制文件写入暂停原因、之后不再认领、`resume` 清掉原因）；处理器 panic 后线程还活着且任务变 `failed`；跑的过程中被取消的任务不会被覆盖；启动时遗留的 `running` 回到 `queued` 且 `error_kind='interrupted'`；控制文件默认预算是 0，暂停/恢复**不会冲掉**已设的预算（回归测试）；`set_daily_budget(0)` 可用且上限生效；`concurrency=2` 时两个任务确实并行（用屏障）；`stop()` 等在跑的任务结束且不再认领；`queue_status` 各字段的数字；`v2_ops::enqueue_missing_cards` 之后控制文件的预算没变。
2. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多（卡开始前 4 条）；`recover_interrupted_jobs` 的旧测试仍通过。
3. 给出：`lib.rs` 里 `enqueue_missing_cards` 命令已没有 `set_daily_budget` 调用的行号（并贴出 `grep -n "set_daily_budget" apps/desktop/src-tauri/src/lib.rs` 的输出）；`get_ai_queue_status` 在 `generate_handler!` 和 `gsat-commands.toml` 里登记的行号；`.setup` 里调用 `start_default_worker` 的行号。
4. `git diff --stat` 里只出现「允许改动」里的路径；`v2_ops.rs` 的 diff 里只有那五个函数。

## 停止条件
- 要改迁移或卡外文件、要新依赖、发现 `ai_jobs` 的 CHECK 约束和本卡的状态流转有冲突——停手并写进报告。
