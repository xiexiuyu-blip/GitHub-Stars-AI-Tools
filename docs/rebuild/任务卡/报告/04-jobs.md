# 交回报告 · 卡 04-jobs

## 一句话结论
做完了：`ai_jobs` 有了可认领、记账、退避、暂停的消费者；默认预算是 0，本卡不注册任何会花钱的处理器。

## 验收逐条
| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| 1. `tools/cards/run.sh rust-test` 通过，`jobs.rs` 新增测试**至少 20 个**，覆盖：认领顺序（优先级、创建时间、`not_before` 未到的不取、没有处理器的类型不取）；4 个线程抢 50 个任务**没有重复认领**；预算为 0 时花钱任务不被认领而不花钱的任务照跑；最近 24 小时用量达到预算后停止认领（24 小时前结束的任务不计入）；暂停时不认领；成功、skipped 的收尾字段；`Retryable` 的退避时间表（30/120/600 秒、`retry_after_secs` 的夹取、抖动参数）和用尽次数后变 `failed`；`Fatal` 直接失败；`PauseQueue`（任务回 `queued`、`attempts` 不增加、控制文件写入暂停原因、之后不再认领、`resume` 清掉原因）；处理器 panic 后线程还活着且任务变 `failed`；跑的过程中被取消的任务不会被覆盖；启动时遗留的 `running` 回到 `queued` 且 `error_kind='interrupted'`；控制文件默认预算是 0，暂停/恢复**不会冲掉**已设的预算（回归测试）；`set_daily_budget(0)` 可用且上限生效；`concurrency=2` 时两个任务确实并行（用屏障）；`stop()` 等在跑的任务结束且不再认领；`queue_status` 各字段的数字；`v2_ops::enqueue_missing_cards` 之后控制文件的预算没变。 | 是 | `tools/cards/run.sh rust-test`：277 passed，0 failed，5 ignored，1 filtered，12.57s。`jobs.rs` 有 25 个 `#[test]`。对应测试：`claim_orders_by_priority_then_created_at`、`claim_skips_not_before_in_the_future`、`claim_skips_job_types_without_a_handler`、`four_threads_do_not_claim_the_same_job_twice`、`zero_budget_claims_free_jobs_and_skips_spending_jobs`、`rolling_24h_usage_stops_spending_claims`、`paused_gate_claims_nothing`、`success_writes_result_fields_and_clears_errors`、`skipped_marks_skipped_status`、`retryable_uses_backoff_table_clamp_and_jitter`、`retryable_fails_when_attempts_are_exhausted`、`fatal_fails_immediately`、`pause_queue_requeues_without_counting_the_attempt`、`handler_panic_fails_the_job_and_the_worker_continues`、`cancelled_job_is_not_overwritten`、`recover_running_jobs_marks_them_interrupted`、`worker_start_returns_running_jobs_to_queue_before_claim`、`control_file_defaults_to_zero_and_pause_keeps_budget`、`set_daily_budget_accepts_zero_and_enforces_cap`、`two_workers_run_jobs_at_the_same_time`、`stop_waits_for_the_running_job_and_claims_nothing_else`、`queue_status_counts_match_the_rows`、`enqueue_missing_cards_does_not_change_the_budget`。另有 `error_kind_strings_match_the_check_constraint`、`start_default_worker_does_nothing_without_handlers`。 |
| 2. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多（卡开始前 4 条）；`recover_interrupted_jobs` 的旧测试仍通过。 | 是 | `rust-fmt` exit 0。`rust-check` exit 0，`generated 4 warnings`，都是原有的：`db/migrations.rs` 的 `MigrationReport.applied` / `backup_path` 未读；`v2_ops::ensure_ready`、`repair_model_json`、`restore_database_file` 未使用。旧测试 `v2_ops::tests::interrupted_job_returns_to_queue`、`five_interrupted_jobs_resume_and_tampered_backup_is_refused` 在全量里为 ok。`cancel_returns_job_and_trending_html_stays_off`（含 `set_daily_budget(..., 50_000) == 50_000`）也 ok。 |
| 3. 给出：`lib.rs` 里 `enqueue_missing_cards` 命令已没有 `set_daily_budget` 调用的行号（并贴出 `grep -n "set_daily_budget" apps/desktop/src-tauri/src/lib.rs` 的输出）；`get_ai_queue_status` 在 `generate_handler!` 和 `gsat-commands.toml` 里登记的行号；`.setup` 里调用 `start_default_worker` 的行号。 | 是 | `enqueue_missing_cards` 在 `lib.rs:1352-1355`，只调用 `v2_ops::enqueue_missing_cards`。`grep -n "set_daily_budget" apps/desktop/src-tauri/src/lib.rs` 只有一行：`1001:    v2_ops::set_daily_budget(storage.database_path(), tokens)`，这是 `set_ai_daily_budget` 命令。`get_ai_queue_status`：`generate_handler!` 在 `lib.rs:6812`；`gsat-commands.toml:83`。`.setup` 调用在 `lib.rs:6792`。 |
| 4. `git diff --stat` 里只出现「允许改动」里的路径；`v2_ops.rs` 的 diff 里只有那五个函数。 | 是 | `git status --short`：`acl-manifests.json`、`gsat-commands.toml`、`lib.rs`、`v2_ops.rs` 为修改，`jobs.rs` 为未跟踪新文件。`git diff --stat`：这四个已跟踪文件，`4 files changed, 21 insertions(+), 22 deletions(-)`。`v2_ops.rs` 的 diff 只动 `queue_control_path`、`set_queue_paused`、`queue_is_paused`、`set_daily_budget`、`daily_budget`。`cargo check` / `cargo test` 没有改出其他 `gen/schemas` 文件。 |

## 改了哪些文件
- `apps/desktop/src-tauri/src/jobs.rs`：新模块。队列控制文件、认领、收尾、工作线程、`queue_status`、`start_default_worker`（固定返回 `None`），以及 25 个测试。
- `apps/desktop/src-tauri/src/v2_ops.rs`：上面五个函数改成调用 `jobs`。签名不变。
- `apps/desktop/src-tauri/src/lib.rs`：`mod jobs`；新增 `get_ai_queue_status` 并登记到 `generate_handler!`；删掉 `enqueue_missing_cards` 命令里的 `set_daily_budget(..., 200_000)`；`.setup` 里在拿到存储后调用 `start_default_worker`，有返回值才 `app.manage`。
- `apps/desktop/src-tauri/permissions/gsat-commands.toml`：允许 `get_ai_queue_status`。
- `apps/desktop/src-tauri/gen/schemas/acl-manifests.json`：同一条命令写进 `allow-gsat-commands`。

## 验证
- `tools/cards/run.sh rust-fmt` → exit 0。
- `tools/cards/run.sh rust-check` → exit 0，约 4.98s 后再次增量通过。警告 4 条，见验收 2。第一次 check 曾到 32 条（新模块在不编译测试时被当成未使用，加上 `queue_is_paused` 失去了唯一的生产调用方）。随后加了 `dead_code` 允许，再跑 check 回到 4 条。
- `tools/cards/run.sh rust-test` → `test result: ok. 277 passed; 0 failed; 5 ignored; 0 measured; 1 filtered out; finished in 12.57s`。5 个 ignored 是原来的（dev 库、大模型、本机向量），1 个 filtered 是运行器跳过的钥匙串测试。
- `git diff --stat` 与 `git status --short` 见验收 4。未提交。

## 没做到的 / 偏离任务卡的地方
- 没有界面，也没有启动 App。这张卡本来就没有界面变化。
- 为了把 `rust-check` 警告压回 4 条，做了两处任务卡没写的允许：`jobs.rs` 模块级 `#![allow(dead_code)]`（处理器要到下一张卡才注册，`cargo check` 不编译测试）；`v2_ops::queue_is_paused` 上的 `#[allow(dead_code)]`。旧的 `set_daily_budget` 会调用它来记住暂停位，那次调用按卡被换成 `jobs::set_daily_budget` 之后，生产代码里就没有别的调用了。运行行为没变，测试仍调用这个函数。
- 认领排序在卡写的 `priority`、`created_at` 之后多了 `id` 升序，只为并列时次序稳定。
- 抖动不是随机数。`finish_job` 接收一个固定的 `jitter`（夹在 -1.0 到 1.0，再乘 ±10%）。工作线程始终传 `0.0`。给了 `retry_after_secs` 时不再加抖动，只夹到 5..=3600 秒。
- `resume_ai_queue` 的函数体没改。它本来就调用 `set_queue_paused(..., false)`，清 `pauseReason` 做在 `jobs::set_paused(false)` 里。

## 需要总控或 Fox 决定的事
- 本卡不读写真实数据目录，因此不会改已经存在的 `fox-stars-lab.queue.json`。如果那份文件里的 `dailyBudgetTokens` 已经是 200000（旧的暂停/恢复会写成这个数，工具箱按钮也会写成这个数），新代码分不出「这是旧 bug」和「Fox 明确设过 200000」，会照这个数放行花钱任务。本卡没有注册花钱处理器，所以合并后仍然不会调用 AI。若要让这台机器也回到「默认不花钱」，需要在开发库副本上把该字段改成 0，或由 Fox 自己改。不要改正式库。
- `apps/desktop/src/pages/v2/toolbox.tsx` 仍会把每日预算设成 200000。本卡不允许改前端。如果这个按钮不该等于授权花钱，需要另开卡。

## 给审核者的提示
- `health_summary` 没改，但它调用 `daily_budget()`。控制文件不存在时，显示从 200000 变成 0。这是本卡要求的默认值。
- 预算 0 是「还没授权」，`queue_status.state` 为 `idle`，不是 `budget_reached`。`budget_reached` 只在预算大于 0、最近 24 小时用量达到预算、且没有 `running`、也没暂停时出现。`stopped` 是已暂停并且写了 `pauseReason`（例如认证失败）；只有 `paused=true` 没有原因时是 `paused`。暂停优先于 `running`。
- 暂停挡住所有处理器，包括不花钱的。花钱任务还要：未暂停、预算大于 0、滚动 24 小时内已结束任务（`succeeded` / `skipped` / `failed`，且 `finished_at` 不空）的 input+output 之和小于预算。窗口含边界（`finished_at >= now-24h`）。取消和仍在跑的不计入。`succeeded24h` 只数窗口内 `succeeded`，不含 `skipped`。
- 用量在任务结束时才入账。最后一个在跑的任务可以把总量撑过预算；并发时，在任何一个结束之前，最多可以再领走 `concurrency` 个花钱任务。模块注释里写了。
- `set_daily_budget` 允许 0，超过 2_000_000 的存成 2_000_000 并返回实际存下的数。1000 的下限已去掉。未知 JSON 字段保留。文件不是合法 JSON 时，读取按「未暂停、预算 0」；写入直接报错，不覆盖坏文件。
- `PauseQueue` 先写控制文件，再把该行改回 `queued`，并把 `attempts` 减 1（已是 0 则保持 0），清空 `started_at` 和 `not_before`，不在这一行上写 `error_kind`。若同一 `(job_type, target_type, target_id)` 上已经有另一条 `queued`/`running`，这条 UPDATE 可能撞上 `idx_ai_jobs_active_target`。那时队列已经暂停，这一行可能仍是 `running`。测试没有造这个撞车。
- 收尾 UPDATE 都带 `status='running'`。已经被取消时返回 `Ok(false)`，不覆盖。
- 工作线程用 `catch_unwind` 接住 panic，记成 `Fatal` / `other`，消息以「任务内部出错：」开头，线程继续。`stop` 只是不再认领，并等当前任务结束。启动时先把遗留 `running` 改回 `queued` 且 `error_kind='interrupted'`；这次恢复若失败，线程会在认领前再试一次，避免把刚领走的任务又打回队列。
- `time` 只开了 `formatting`，没有解析功能，也没加依赖。库内时间用固定 24 字符 `YYYY-MM-DDTHH:MM:SS.mmmZ`，自己格式化和加减。
- 非测试代码没有 `unwrap()` / `expect()`。测试里有。
