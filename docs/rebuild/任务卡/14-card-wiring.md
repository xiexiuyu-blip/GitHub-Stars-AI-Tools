---
编号: 14-card-wiring
允许改动:
  - apps/desktop/src-tauri/src/card_handler.rs
  - apps/desktop/src-tauri/src/jobs.rs
  - apps/desktop/src-tauri/src/lib.rs
  - apps/desktop/src-tauri/permissions/gsat-commands.toml
  - apps/desktop/src-tauri/gen/schemas/acl-manifests.json
  - apps/desktop/src/pages/v2/toolbox.tsx
---

# 卡 14 · 理解卡接线：处理器、自动出卡、预算授权、确认与重做

## Fox 能看到的效果
前面几张卡把零件都做好了（队列消费者、理解卡流水线、场景），这张卡把它们**接成一条能用的线**：①同步 Stars 之后，新收藏的仓库会自动排队出理解卡（只在 Fox 授权了每天额度之后才真的调用 AI，**没授权就一分钱不花**）；②「工具箱」页有一个明确的「AI 自动整理」开关，要填「每天最多用多少 token」并点确认才生效，旁边能看到今天用了多少、还剩多少、排队几个、有没有因为密钥出错而暂停；③详情页里「说得对吗？」的两个按钮有后端了：「对」把卡标为已确认，「不对，重新整理」把它排进队列重做；④旧的「每日预算 20 万」一键按钮删掉，再也不会因为误点而授权花钱。

## 背景与先读什么（本卡依赖的前面几张卡，都已合并在基线里）
- **队列消费者**：`apps/desktop/src-tauri/src/jobs.rs`（卡 04）：`JobHandler` trait、`JobRecord`、`JobSuccess`、`JobFailure`（`Retryable`/`Fatal`/`PauseQueue`）、`ErrorKind`、`Worker::start`、`WorkerConfig`、`queue_status`、`start_default_worker`（目前固定返回 `None`）、控制文件读写（`read_control`、`set_paused`、`set_daily_budget`）。**预算 0 = 不花钱，是默认值，不许改这个默认。**
- **理解卡流水线**：`apps/desktop/src-tauri/src/card_pipeline.rs`（卡 05）：`run_repo_card(conn, client, repo_id, options)`、`RunOptions`（含 `force`、`now`）、`RealLlmClient { config: AiRequestConfig }`、`CardRunReport`、`CardRunError`（带 `kind` 和 `retry_after_secs`）、`PROMPT_VERSION`。
- **场景**：`apps/desktop/src-tauri/src/scenarios.rs`（卡 06，**如果基线里还没有这个文件，先停手写进报告**）：`apply_ai_scenarios(conn, account_id, repo_id, slugs, confidence)`、`seed_scenarios`。理解卡 JSON 里的 `scenario_tags` 要在出卡成功后写进场景归类。
- **AI 配置与密钥**：`lib.rs` 里的 `ai_request_config_from_settings_value`（从设置读出提供商、地址、模型，`api_key` 留空）、`read_ai_api_key_for_provider`（从系统钥匙串读密钥）、`merge_ai_request_config`；设置文件路径怎么取，看 `lib.rs` 里读设置的那几处（搜 `fox-stars-lab.settings.json` 或 `settings_path`）。**钥匙串只在处理器真的要调用 AI 时才读，不要在应用启动时读**（启动时读会让 macOS 弹授权窗，卡在加载界面）。
- **入队**：现有的 `enqueue_missing_cards`（`v2_ops.rs` 约第 1123 行，命令在 `lib.rs`）用 `'card-' || repositories.id` 当任务 id，所以一个仓库只能入队一次，之后永远插不进去——**不要改它**，在新模块里另写入队函数，任务 id 用随机的（`lower(hex(randomblob(8)))` 之类）。活动任务有唯一索引（同一仓库同一类型同一时刻只能有一条 queued/running），`INSERT OR IGNORE` 就能避免重复。
- 同步命令：`lib.rs` 里 `sync_github_stars`（搜它），同步成功之后要触发入队。读懂它的返回值和线程模型再动手，别破坏它现有的行为。
- 现有的工具箱页：`apps/desktop/src/pages/v2/toolbox.tsx`（卡 02 加了备份区块；里面还有一个「每日预算 20 万」按钮要删掉）。
- 测试怎么造库：看 `db/migrations.rs` 里 `empty_database_applies_every_migration`；假的 AI 用 `card_pipeline.rs` 测试里的 `ScriptedClient` 那种写法（在本模块的测试里自己写一个实现 `LlmClient` 的替身）。**所有测试只用临时库和假 AI，不读钥匙串，不联网。**

## 要做的事

### 1. 新模块 `src-tauri/src/card_handler.rs`
1. **`RepoCardHandler`**：实现 `jobs::JobHandler`：`job_type() = "repo_card"`，`spends_tokens() = true`。构造时传入一个「取 AI 配置」的闭包 `Arc<dyn Fn() -> Result<AiRequestConfig, String> + Send + Sync>`（闭包由 `lib.rs` 提供：读设置文件、读钥匙串；**闭包在 `run` 里才被调用**）。同时提供一个可注入假 AI 的构造函数 `with_client_factory(...)`，测试用。
2. **`run`**：从 `job.target_id` 取仓库 id（`target_type` 必须是 `repository`，否则 `Fatal(Other, …)`）；取到 AI 配置失败（没配置、没密钥）→ `PauseQueue { kind: Auth, message: "还没有配置 AI 服务或密钥，已暂停整理队列" }`；调用 `card_pipeline::run_repo_card`（`force=false`）；成功后读出该仓库 `card_json` 里的 `scenario_tags`，调用 `scenarios::apply_ai_scenarios(…, 卡片 confidence)`；返回 `JobSuccess`（真实 token、`provider`、`model`、`prompt_version`、`input_hash = source_hash`，跳过的 `skipped=true`）。
3. **错误映射**（`CardRunError` → `JobFailure`）：`RateLimit` → `Retryable { RateLimit, retry_after_secs }`；`Timeout`/`Network` → `Retryable`；`Auth` → `PauseQueue { Auth }`；`ContentTooLarge` → `Fatal`；`BadJson`/`Schema` → `Retryable`；`NotFound` → `Fatal`；`Database` → `Retryable { Other }`；其他 → `Retryable { Other }`。
4. **入队函数**：
   - `enqueue_cards(conn, scope, priority) -> usize`：`scope` 取 `Missing`（有效仓库里**没有理解卡**的，即 `card_json` 为空）、`Outdated`（有卡但 `card_schema_version < 2` 且卡**未被确认**）；用 `INSERT OR IGNORE INTO ai_jobs(id, account_id, job_type, target_type, target_id, priority, status)`，状态 `queued`；返回新入队条数；整批一个事务；已被确认的卡永不入队。
   - `enqueue_new_stars(db_path, repo_ids: &[String])`：同步后调用，只给**这批新仓库**里还没有理解卡的入队，优先级 3（比补旧账的 6 高）。
5. **确认与重做**：
   - `confirm_card(db_path, repo_id)`：把 `card_json` 里的 `review_status` 改成 `confirmed`（解析 JSON 再写回，写成紧凑 JSON；`card_json` 为空或不是 JSON 返回中文错误「这个仓库还没有理解卡」）；同时把这个仓库未被拒绝的主分类（`entity_categories`，`is_primary=1`）的 `status` 改成 `confirmed`、`origin` 改成 `user`。
   - `request_regenerate(db_path, repo_id)`：把 `review_status` 改回 `suggested`，把 `repo_ai_documents.source_hash` 清成空串（这样流水线不会因为"内容没变"而跳过），并入队一条优先级 2 的 `repo_card` 任务；已有 queued/running 的同一任务时直接返回成功（不重复）。仓库不存在返回中文错误。

### 2. `jobs.rs`
- 去掉模块顶部的 `#![allow(dead_code)]`；`start_default_worker` 改成 `start_worker(db_path, handlers)`（保留旧函数名也行，但不能再固定返回 `None`）。如果去掉 `allow` 之后 `cargo check` 出现新的未使用警告，就**只**给真正暂时没用的项单独加 `#[allow(dead_code)]` 并写明原因，警告总数不能比卡开始前多。

### 3. `lib.rs`
- `.setup` 里：创建 `RepoCardHandler`（AI 配置闭包：读设置、读钥匙串，`merge_ai_request_config`），用 `jobs::Worker::start` 起工作线程，放进 Tauri 的托管状态（`app.manage`），应用退出时不用特别处理。**启动时不读钥匙串**；预算为 0 时工作线程不会领花钱任务，所以也就不会读。
- 同步成功之后：从同步结果里拿到这次新增的仓库 id（读 `sync_github_stars` 的实现，看有没有"新增列表"；没有就用「同步前后差集」或「`created_at` 在这次同步开始之后」的办法），调用 `enqueue_new_stars`；**入队失败只记日志，不影响同步的返回值**。
- 新增命令并登记到 `generate_handler!` 和 `permissions/gsat-commands.toml`：
  - `confirm_library_card(id)`、`regenerate_library_card(id)`（契约见 `apps/desktop/src/lib/contracts.ts` 末尾，成功返回 `null`）；
  - `get_ai_automation() -> AutomationStatus`：`{ enabled（预算大于 0 且未暂停）, dailyBudgetTokens, paused, pauseReason: {kind, message, at} | null, tokens24h, budgetRemaining, queued, running, failed, providerConfigured（设置里提供商不是 none 且有模型名；**不要为此读钥匙串**，只看设置文件）, model: string | null }`，JSON 用 `camelCase`；
  - `set_ai_automation(dailyBudgetTokens)`：只写预算（0 = 关闭），返回新的 `AutomationStatus`；上限沿用 `jobs` 的 2_000_000；
  - `enqueue_cards_now(scope)`：`scope` 是 `"missing"` 或 `"outdated"`，返回入队条数；**只入队，不授权花钱**（预算为 0 时任务会一直排着，这是对的）。
- **旧的 `enqueue_missing_cards` 命令保持不动**（卡 04 已经去掉了它的预算副作用）。

### 4. `pages/v2/toolbox.tsx`
- **删掉**「每日预算 20 万」按钮。
- 新增区块「AI 自动整理」：一个数字输入框（每天最多用多少 token，默认显示当前值，0 表示关闭）和「保存」按钮；当输入值大于 0 且和当前值不同时，点保存先弹一个页内确认块：「这会让应用在后台调用你配置的 AI 服务整理仓库，并按你的服务商计费。每天最多使用 {N} token（约相当于整理 {N/4000 取整} 个仓库）。确认开启吗？」，确认后才调用 `set_ai_automation`；设成 0 不需要确认。显示状态一行：「已开启 / 未开启」「今天已用 {tokens24h}，还剩 {budgetRemaining}」「排队 {queued}，进行中 {running}，失败 {failed}」；`providerConfigured` 为假时提示「还没配置 AI 服务，先去设置里配置」；有 `pauseReason` 时红字显示原因，并给一个「继续队列」按钮（调用已有的 `resume_ai_queue`）。另有两个次要按钮：「给没有理解卡的仓库排队」（`enqueue_cards_now('missing')`）和「把旧版理解卡排队重做」（`enqueue_cards_now('outdated')`），点后用一行字提示「已排队 N 个；是否开始整理取决于上面的每天额度」。
- 这个区块里的主按钮样式只给「保存」；页面其余按钮保持次要样式。所有文字简体中文；错误显示后端返回的中文原因，不要吞掉。

## 不要做
- 不改迁移、不改 `card_pipeline.rs`、`scenarios.rs`、`ai.rs`、`v2_ops.rs`；不新增依赖；不改工具箱以外的前端。
- **不调用任何真实的 AI 接口，不读钥匙串（测试里），不读写真实数据目录。**
- 不要在应用启动或同步时让任何东西自动花钱：没有授权预算，就只排队。

## 验收
1. `tools/cards/run.sh rust-test` 通过，`card_handler.rs` 新增测试**至少 22 个**，覆盖：处理器的成功路径（假 AI 返回合法卡 → 任务成功、token 记账、写了 `card_json`、`scenario_tags` 写进了场景归类、`JobSuccess` 各字段）；跳过路径；取配置失败 → `PauseQueue`（队列被暂停、任务回到 queued）；每一种 `CardRunError` 到 `JobFailure` 的映射（逐个断言）；**用 `jobs::Worker` 加假处理器跑通一条完整链路**（入队 → 预算为 0 时不被领 → 设预算后被领并成功）；`enqueue_cards` 的两个 scope（已确认的卡不入队、已有 queued 任务不重复、返回条数、优先级）；`enqueue_new_stars` 只给传入的仓库入队且优先级是 3；`confirm_card`（`review_status` 改成 confirmed、分类变 confirmed、没有卡报错、紧凑 JSON）；`request_regenerate`（`review_status` 回到 suggested、`source_hash` 清空、入队优先级 2、已有任务不重复、不存在报错）；`AutomationStatus` 各字段（含 `providerConfigured` 只看设置不读钥匙串）；预算默认 0。
2. 契约测试：`AutomationStatus` 序列化后键集合恰好是 `enabled`、`dailyBudgetTokens`、`paused`、`pauseReason`、`tokens24h`、`budgetRemaining`、`queued`、`running`、`failed`、`providerConfigured`、`model`。
3. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多（4 条）；`tools/cards/run.sh web-build`、`web-test` 通过。
4. 给出：5 个新命令（`confirm_library_card`、`regenerate_library_card`、`get_ai_automation`、`set_ai_automation`、`enqueue_cards_now`）在 `generate_handler!` 和 `gsat-commands.toml` 里的行号；`.setup` 里创建并启动工作线程的行号，并**逐行说明启动路径上没有读钥匙串**；同步成功后调用 `enqueue_new_stars` 的行号；`toolbox.tsx` 里确认弹块、状态行、「每日预算 20 万」已删除（给出 `grep -n "200000\|20 万" apps/desktop/src/pages/v2/toolbox.tsx` 的输出，应为空）的证据。
5. `git diff --stat` 里只出现「允许改动」里的路径；`card_pipeline.rs`、`scenarios.rs`、`ai.rs`、`v2_ops.rs` 没有改动。

## 停止条件
- 基线里没有 `scenarios.rs`、要改迁移或卡外文件、要新依赖、同步命令的结构让「拿到新增仓库」做不到——停手并写进报告。
