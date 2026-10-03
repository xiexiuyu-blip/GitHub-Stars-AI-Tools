# 交回报告 · 卡 14-card-wiring

## 一句话结论
理解卡已经接上队列：同步后的新收藏会入队，默认预算 0 不会调用 AI；工具箱可以授权每天额度、确认或重做理解卡，一键「每日预算 20 万」已删除。

## 验收逐条
| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| 1. `tools/cards/run.sh rust-test` 通过，`card_handler.rs` 新增测试至少 22 个，覆盖：处理器的成功路径（假 AI 返回合法卡 → 任务成功、token 记账、写了 `card_json`、`scenario_tags` 写进了场景归类、`JobSuccess` 各字段）；跳过路径；取配置失败 → `PauseQueue`（队列被暂停、任务回到 queued）；每一种 `CardRunError` 到 `JobFailure` 的映射（逐个断言）；用 `jobs::Worker` 加假处理器跑通一条完整链路（入队 → 预算为 0 时不被领 → 设预算后被领并成功）；`enqueue_cards` 的两个 scope（已确认的卡不入队、已有 queued 任务不重复、返回条数、优先级）；`enqueue_new_stars` 只给传入的仓库入队且优先级是 3；`confirm_card`（`review_status` 改成 confirmed、分类变 confirmed、没有卡报错、紧凑 JSON）；`request_regenerate`（`review_status` 回到 suggested、`source_hash` 清空、入队优先级 2、已有任务不重复、不存在报错）；`AutomationStatus` 各字段（含 `providerConfigured` 只看设置不读钥匙串）；预算默认 0。 | 是 | `tools/cards/run.sh rust-test`：416 passed，0 failed，5 ignored（原有），1 filtered（脚本跳过的钥匙串用例）。`card_handler` 32 个测试全过。对应：`handler_success_writes_card_tokens_scenarios_and_job_row`；`handler_skips_when_the_same_content_is_already_current`、`handler_skips_a_confirmed_card_without_calling_the_model`；`config_failure_pauses_the_queue_and_requeues_the_job`、`unusable_config_pauses_without_calling_the_model`；`rate_limit_maps_to_retryable`、`timeout_maps_to_retryable`、`network_maps_to_retryable`、`pipeline_auth_maps_to_pause_queue`、`content_too_large_maps_to_fatal`、`bad_json_maps_to_retryable`、`schema_maps_to_retryable`、`not_found_maps_to_fatal`、`database_maps_to_retryable_other`、`other_maps_to_retryable_other`；`worker_leaves_spending_jobs_queued_until_budget_is_set`；`enqueue_missing_skips_confirmed_current_and_duplicates`、`enqueue_outdated_only_old_unconfirmed_cards`；`enqueue_new_stars_only_listed_repos_at_priority_three`；`confirm_card_confirms_review_and_primary_category`、`confirm_card_rejects_a_missing_card`；`regenerate_resets_hash_and_enqueues_priority_two`、`regenerate_keeps_an_existing_active_job`、`regenerate_missing_repo_is_an_error`；`automation_status_reports_queue_budget_and_provider`、`provider_configured_reads_settings_only`；`budget_defaults_to_zero`。 |
| 2. 契约测试：`AutomationStatus` 序列化后键集合恰好是 `enabled`、`dailyBudgetTokens`、`paused`、`pauseReason`、`tokens24h`、`budgetRemaining`、`queued`、`running`、`failed`、`providerConfigured`、`model`。 | 是 | `automation_status_json_keys_match_the_contract` 通过。结构体在 `card_handler.rs` 的 `AutomationStatus`，`serde(rename_all = "camelCase")`，`pauseReason` 和 `model` 为 null 时仍写出键。 |
| 3. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多（4 条）；`tools/cards/run.sh web-build`、`web-test` 通过。 | 是 | `rust-fmt` 退出码 0。`rust-check`：`generated 4 warnings`（`MigrationReport` 的 `applied`/`backup_path`、`ensure_ready`、`repair_model_json`、`restore_database_file`），然后 Finished。`web-build` 退出码 0，vite built in 700ms。`web-test`：8 files，76 passed。 |
| 4. 给出：5 个新命令在 `generate_handler!` 和 `gsat-commands.toml` 里的行号；`.setup` 里创建并启动工作线程的行号，并逐行说明启动路径上没有读钥匙串；同步成功后调用 `enqueue_new_stars` 的行号；`toolbox.tsx` 里确认弹块、状态行、「每日预算 20 万」已删除（`grep -n "200000\|20 万" apps/desktop/src/pages/v2/toolbox.tsx` 应为空）的证据。 | 是 | 见下方「行号」。`rg -n "200000\|20 万" apps/desktop/src/pages/v2/toolbox.tsx` 无输出。 |
| 5. `git diff --stat` 里只出现「允许改动」里的路径；`card_pipeline.rs`、`scenarios.rs`、`ai.rs`、`v2_ops.rs` 没有改动。 | 是 | `git diff --stat` 只有 acl-manifests.json、gsat-commands.toml、jobs.rs、lib.rs、toolbox.tsx。新文件 `card_handler.rs` 未跟踪，所以不进 `--stat`，`git status` 能看到，路径在允许列表里。上述四个文件 `git diff --stat` 为空。`enqueue_missing_cards` 命令仍在。 |

### 行号

`generate_handler!`（`apps/desktop/src-tauri/src/lib.rs`）：`confirm_library_card` 6995，`regenerate_library_card` 6996，`get_ai_automation` 6997，`set_ai_automation` 6998，`enqueue_cards_now` 6999。

`permissions/gsat-commands.toml`：同上五个名字在 118–122 行。`acl-manifests.json` 的 allow 列表在 `set_repository_scenario` 后追加了这五个名字（文件仍是一行）。

`.setup` 创建并启动工作线程在 `lib.rs` 6936–6945。逐行：

- 6936 取出存储。只解析数据库路径，不读钥匙串。
- 6937 按库里的开关启动本地 API。不读 AI 钥匙串。
- 6938 把数据库路径复制出来。不读钥匙串。
- 6939 克隆 `AppHandle`。不读设置文件，不读钥匙串。
- 6940 注释，说明闭包此时不调用。
- 6941–6943 `RepoCardHandler::new` 把 `load_card_ai_config` 放进闭包。这一行不调用闭包。读钥匙串的 `read_ai_api_key_for_provider` 只写在 `load_card_ai_config`（1995 行）里，只有处理器 `run` 被调用时才会走到。
- 6944 `jobs::start_worker`。该函数不写预算、不调用处理器。预算只看控制文件，缺省是 0，花钱任务不会被领走。
- 6945 `app.manage` 托管工作线程。不读钥匙串。

同步成功后入队在 `lib.rs` 3800 行：`card_handler::enqueue_new_stars`。失败只 `eprintln!`，后面仍返回原来的 `StarSyncSummary`。新增 id 用同步前的 `existing_states` 和这次拉下来的 id 做差集（3776–3781），和原来的 `created_count` 是同一批。

工具箱：确认块在 `toolbox.tsx` 278 行；状态行在 248 行；暂停原因红字在 255 行（`text-error`），旁边「继续队列」走已有的 `resume_ai_queue`。区块标题「AI 自动整理」在 246 行。

## 改了哪些文件
- `apps/desktop/src-tauri/src/card_handler.rs`：新模块。理解卡处理器、两种补队、新收藏入队、确认、重做、自动整理状态，以及 32 个测试。
- `apps/desktop/src-tauri/src/jobs.rs`：去掉模块级 `allow(dead_code)`；`start_default_worker` 改成 `start_worker`；`JoinHandle` 放进 `Mutex`，工作线程才能被 Tauri 托管。
- `apps/desktop/src-tauri/src/lib.rs`：登记模块和 5 个命令；启动工作线程；同步成功后给新收藏入队；读设置文件的状态函数不碰钥匙串。
- `apps/desktop/src-tauri/permissions/gsat-commands.toml`：允许这 5 个命令。
- `apps/desktop/src-tauri/gen/schemas/acl-manifests.json`：allow 列表追加同样 5 个命令。
- `apps/desktop/src/pages/v2/toolbox.tsx`：删掉「每日预算 20 万」；加上「AI 自动整理」。

## 验证
- `tools/cards/run.sh rust-fmt-fix` → 退出码 0；随后 `rust-fmt` → 退出码 0。
- `tools/cards/run.sh rust-check` → Finished，4 条警告，都是改动前就有的。
- `tools/cards/run.sh rust-test card_handler` → 32 passed，0 failed。
- `tools/cards/run.sh rust-test` → 416 passed，0 failed，5 ignored，1 filtered，25.79s。
- `tools/cards/run.sh web-build` → 退出码 0，vite built in 700ms。
- `tools/cards/run.sh web-test` → 8 files，76 passed。
- `rg -n "200000|20 万" apps/desktop/src/pages/v2/toolbox.tsx` → 无匹配。
- `git diff --stat` 与 `git status --short` 见验收第 5 条。没有用 git 写命令。

## 没做到的 / 偏离任务卡的地方
- 没有启动桌面应用去点工具箱。红线不许跑 `pnpm dev:desktop`、`tauri dev` 或 `cargo run`。确认块和状态行只经过 `web-build`（tsc + vite）和 `web-test`，没有在窗口里点过。
- `run_repo_card` 要求客户端是固定大小的类型。测试注入的是 trait 对象，所以在 `card_handler.rs` 里包了一层 `SizedClient`，没有改 `card_pipeline.rs`。
- 空的或坏掉的 `card_json` 上直接 `json_extract` 会让整句 SQL 报 malformed JSON。入队条件改成 `CASE WHEN json_valid(...)` 再 `json_extract`。确认状态仍然用 `json_extract`，不用 `LIKE`。空卡和坏 JSON 不算已确认。
- `with_client_factory` 只在 `#[cfg(test)]` 下编译，避免 `cargo check` 把它算成未使用。测试里仍能注入假 AI。
- `WorkerStatus`、`Worker::stop`、`Worker::status` 加了带原因的 `#[allow(dead_code)]`。它们给测试停线程、看状态；应用退出走 `Drop`。`cargo check` 不编译测试，不加的话警告会超过 4 条。

## 需要总控或 Fox 决定的事
无

## 给审核者的提示
- 启动这几行不读钥匙串。如果控制文件里已经有大于 0 的每天额度，工作线程领到 `repo_card` 之后才会调用 `load_card_ai_config`，那时才会读钥匙串。预算缺省是 0，这种情况下不会领花钱任务。
- `get_ai_automation` / `set_ai_automation` 只读设置文件（`read_app_settings_value`）。`providerConfigured` 看提供商不是 `none` 且模型名非空，不看 `apiKey`，不读钥匙串。
- 本机 `openai-compatible` 且地址是 loopback 时，空密钥算已配好（沿用 `can_use_ai_without_api_key`），不会因此暂停队列。别的空密钥会暂停，文案固定为「还没有配置 AI 服务或密钥，已暂停整理队列」，底层错误不拼进去。
- 同步只把「这次之前库里没有的 id」入队。以前取消星标、这次又星上的仓库算更新，不会自动入队；工具箱的「给没有理解卡的仓库排队」能补上。
- 重做时如果已经有 queued 或 running 的同一任务，不改它的优先级，只把卡改回 suggested 并清空 `source_hash`，然后返回成功。
- 保存每天额度：输入和当前值相同则不下发命令；大于 0 且不同才出页内确认；0 不确认。空输入报「请先填写每天额度」，不当成 0。上限仍是 `jobs` 的 2_000_000，保存后输入框显示服务端压过的数字。设预算不会顺便取消暂停。
- `card_handler.rs` 是新文件，`git diff --stat` 看不到它，请用 `git status` 一起看。
