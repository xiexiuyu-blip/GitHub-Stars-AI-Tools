# 交回报告 · 卡 34-rerank-hardening

## 一句话结论
做完了：卡里 7 件事全部落地并有测试；另外加固了 3 处卡外但同源的问题（reqwest blocking 的时限不是合计、慢失败记账、断路器试探租约）。`rust-fmt`、`rust-check`（警告数不变：库 6、测试构建 1）、`rust-test`（1200 通过，基线 1135，新增 65 个）全绿。**3 处现有测试的夹具因新规则必须调整（断言一个没改）、1 处语义取舍，需要总控过目（见「需要决定的事」）。**

## 验收逐条
| 验收项（照抄任务卡） | 做到了吗 | 证据 |
|---|---|---|
| 1. `rust-test` 全部通过；新增 Rust 测试至少 26 个（临时库 + 本地假服务 + 假 `ask`，不联网，不 sleep 超过 1 秒） | 是 | 1200 通过 / 0 失败 / 5 ignored（基线 1135 / 0 / 5）。新增 65 个：`spend_guard.rs` 12、`ai.rs` 19、`solve_rerank.rs` 34。全部本机假服务 / 临时库 / 假 `ask`，没有联网。有 3 条 `ai.rs` 测试因「最终超时」本来就要等时限而用 1 秒时限（每条约 1–1.5 秒，并行跑），其余没有超过 1 秒的 sleep（线程对齐用通道和带 5 秒上限的轮询，不靠睡） |
| 　额度联动：0+无全量→拒绝；0+全量→30 万；10 万→5 万；60 万→30 万；1000 万→30 万；暂停→拒绝；已用+预计==上限放行、+1 拒绝；文案里是有效上限；经过 `reserve_with` 的集成用例 | 是 | `spend_guard::tests`：`the_rerank_cap_is_half_the_daily_budget_but_never_above_the_ceiling`（含 1→0、u64::MAX→30 万、全量整理下 0/1/10 万/1000 万都是 30 万）、`rerank_edges_follow_the_effective_cap`（5 组每天额度 × 边界 ==放行 / +1 拒绝）、`rerank_zero_budget_and_pause_still_refuse_with_the_same_wording`（两句拒绝文案逐字不变，全量整理也越不过暂停）、`the_refusal_names_the_effective_cap_not_the_ceiling`、`a_small_daily_budget_shrinks_rerank_through_reserve_with`（10 万额度、AI 精选已用 4.5 万、预计 1 万→拒绝，文案「会超过它每天 5.0 万 token 的额度」；4.5+0.5 万放行、再 +1 拒绝）、`a_full_scan_lifts_rerank_to_the_ceiling_even_with_a_small_daily_budget` |
| 　单次调用：200 空内容 / 429 / 500 / 400 / 响应体读到一半断开，请求数恰好 1；`max_tokens == 4096`（OpenAI 兼容和 Anthropic 各一个）；旧入口无 `max_tokens`、500 仍重试 | 是 | `ai::tests`：`a_rerank_call_is_exactly_one_request_whatever_comes_back`（6 种：200 空内容 / 429 / 500 / 400 / 响应体半截 / 连接被直接断开，每种 `CountingServer` 数到恰好 1 个请求，并核对账本）、`the_anthropic_rerank_path_is_one_request_too`、`a_rerank_request_to_an_openai_compatible_service_carries_max_tokens_and_books_real_usage`、`a_rerank_request_to_anthropic_carries_max_tokens_4096_up_from_1200`、`the_old_entry_points_still_send_no_output_limit_to_openai_compatible_services`（旧入口请求体键集合恰好是 messages / model / stream / temperature）、`the_old_entry_point_still_retries_a_server_error`（500 后重试，共 2 个请求）、`a_rerank_call_does_not_follow_a_redirect` + `the_old_entry_point_still_follows_a_redirect`、`zero_attempts_still_sends_one_request` |
| 　记账：4xx/429→0；最终超时（`timeout_secs=1`）≥ 1 万、状态不是 running；200 带真实用量→记真实用量 | 是 | 4xx/429→`("cancelled", 0, 0)`、200 带用量→`("succeeded", 700, 50)`：同上两条；最终超时：`a_final_timeout_books_at_least_what_the_queue_books_for_one`（假服务收完整请求后不回应，`timeout_secs = 1`，请求数 1，文案「已等待 1 秒」，账本 `("cancelled", 10000, 0)`）、`a_final_timeout_while_the_body_never_finishes_is_booked_the_same_way`、`a_final_timeout_with_a_long_prompt_is_booked_at_the_worst_case`、`a_failure_after_a_long_wait_is_booked_like_a_timeout_but_a_rejection_never_is`、spend_guard 里 6 条 `a_slow_failure_*` |
| 　命令等待时限：常量大于请求超时（`assert!`） | 是 | `solve_rerank::tests::the_command_waits_a_little_longer_than_one_model_request`（`WAIT_SECONDS 95 > ai::SOLVE_RERANK_TIMEOUT_SECONDS 90`）；另有 `the_command_waits_for_the_shared_constant_and_keeps_its_messages`（读 `lib.rs` 源码，确认命令用的是 `solve_rerank::WAIT_SECONDS`、不是写死的 60，超时 / 出事两句文案没变） |
| 　`parse_reply`：`{}`、`{"order":["x"],"fit":{"foo":"best"}}`、`{"fit":{"A1":"great"}}`→`None`；`{"fit":{"A2":"good"}}`→`Some`；上层 `rerank_with` 验证 failed、不写缓存、`ask` 只 1 次 | 是 | `a_reply_without_a_single_recognized_fit_does_not_parse`（10 种无有效判断的回复）、`a_reply_with_one_recognized_fit_still_parses_and_fills_in_maybe`、`a_reply_with_no_recognized_fit_is_a_failure_and_not_cached`（三种回复各：failed、文案不变、`none_fit=false`、`ask` 1 次、`cache_count == 0`） |
| 　单飞：两线程同一问题 + 慢 `ask`→1 次、第二个 `cached`；先到的失败→两个都 failed、`ask` 1 次；失败后第三次重新调；不同问题各调各的；守卫 panic 也清登记 | 是 | `two_identical_requests_make_one_model_call_and_the_second_is_cached`（用 `Arc::strong_count` 确认后来者真的在等，不是碰巧读到缓存）、`when_the_first_call_fails_the_waiting_one_gets_the_same_failure_without_asking`（含第三次重新调且不被断路器拦：证明后来者那份失败没有重复记账）、`different_questions_do_not_wait_for_each_other`、`a_panicking_first_call_still_clears_the_registration`（`catch_unwind`）、`a_panic_in_the_first_call_releases_a_waiting_one_with_a_failure`、另有 `a_burst_of_identical_requests_makes_exactly_one_model_call`（25 轮 × 8 线程同时来）、`a_result_stored_just_before_registering_is_found_by_the_second_cache_read`（用测试挂钩确定性地复现「读缓存没读到 → 别人刚写完撤销 → 我登记」的窗口）、`the_same_question_in_two_databases_does_not_wait`、`a_follower_gives_up_after_the_wait_limit_without_asking`、`a_refused_first_call_is_shared_as_refused`、`a_poisoned_state_lock_does_not_break_later_calls` |
| 　断路器：连续 2 次 failed→第 3 次 skipped 且不调 `ask`；refused/skipped/cached 不计数；成功清零；10 分钟后允许一次、再失败又停；打开时缓存命中仍返回 cached；不同模型名互不影响 | 是 | `two_failures_in_a_row_pause_the_model_and_ask_is_not_called`、`refused_skipped_and_cached_results_neither_count_nor_reset`、`a_success_clears_the_failure_count`、`after_ten_minutes_one_attempt_is_allowed_and_a_failure_pauses_again`（注入时钟，T0+599 拦、+600 放、再失败 +1199 拦、+1200 放）、`a_cache_hit_still_returns_while_the_model_is_paused`、`a_paused_model_does_not_affect_another_model`；另有 `a_failed_attempt_pauses_for_ten_minutes_from_the_failure_not_from_the_start`、`only_one_search_gets_through_when_the_pause_ends`、`a_probe_that_gets_no_verdict_does_not_pause_the_model_for_another_ten_minutes`、`a_clock_set_far_back_does_not_pause_the_model_for_hours`、`local_failures_before_asking_do_not_count_toward_the_breaker`、`unreadable_replies_and_replies_without_a_judgment_count_as_failures`。全部不 sleep（时钟是注入的闭包） |
| 　清洗：`https://x.y`、`www.x.com`、`foo.com/bar`、`evil.xyz`→兜底；`Next.js`、`Node.js`、`ffmpeg.wasm`、`v1.2` 不受影响；建议同样处理 | 是（http(s) 的语义见「偏离」） | `a_reason_that_contains_a_web_address_is_blanked_whole`（17 种）、`tool_names_and_versions_with_dots_are_not_mistaken_for_addresses`（含 `.NET`、`foo.community`、`evil.xyzzy`）、`invisible_characters_cannot_hide_a_web_address`（13 种零宽 / 方向 / 异体字选择符夹在网址里）、`address_detection_is_linear_on_hostile_input`（6 万字符的恶意输入，各 < 500 ms）、`a_web_address_in_a_reason_or_the_note_falls_back_to_the_default_text`（走 `rerank_with`：理由→「最对口 / 不太合适」，建议→无 / 兜底建议） |
| 2. 现有测试一个断言都不许放宽或删除；`rust-fmt`、`rust-check` 通过，警告数不比卡开始前多 | 是 | `git diff` 里被删的行只有下面 3 处夹具和 1 条注释（逐条列在「偏离」里），没有一个断言被放宽 / 删除。`rust-fmt` 无输出；`rust-check` 6 个警告（基线同样 6 个，同一批）；测试构建 1 个警告（基线同样 1 个） |
| 3. 旧功能没被改动：`Policy::Legacy` 分支、`request_openai_streaming`、`execute_json_post` 默认路径无行为变化（指出新增分支的行） | 是 | `git diff -U0` 的 hunk 没有碰 `reserve_with` / `Policy::Legacy` / `record` / `SpendPermit::drop` / `request_openai_streaming` / `request_anthropic_streaming` / `execute_sse_post` / `execute_json_get`。`execute_json_post(url, headers, body)` 现在只是 `execute_json_post_with(.., &AiCallOptions::default())`；`Default` = 不带 `max_tokens`、120 秒、3 次、`single_request=false`、`bill_slow_failures=false`。新增分支全在 `ai.rs`：2227–2231（只在 `single_request` 时关协议层重试 / 重定向）、2241–2245（同条件的请求级合计时限）、2256–2263 / 2272–2275（`bill_slow_failures` 才有的 `waited_out` 判断和 `account_slow_failure`）、2288–2295（响应体读不完，同条件）、2314–2322（慢 5xx，同条件）；两个开关都关着时这些行不执行，其余逐行和原来一样。旧入口 / 旧行为另有测试钉住：`the_old_entry_point_still_retries_a_server_error`、`the_old_entry_point_still_follows_a_redirect`、`the_old_options_give_the_body_its_own_fresh_window`、`without_the_rerank_flags_a_short_total_timeout_is_still_refunded_like_before`，以及全部原有的记账测试 |
| 4. `git diff --stat` 只出现允许改动的文件 | 是 | `ai.rs`、`lib.rs`、`solve_rerank.rs`、`spend_guard.rs`、`docs/rebuild/命令契约.md`（只改 §10）。没碰 `jobs.rs`、迁移、前端、`solve.rs`、权限文件；没新增依赖、没加 Tauri 命令 |
| 5. 报告里写：代码位置、常量及理由、最终超时那一笔记多少、状态放哪怎么清理、与审查结论不一致、没把握 | 是 | 见下面各节 |

## 要做的事 ↔ 代码位置
1. **额度跟着每天额度走**：`spend_guard.rs:478` `rerank_cap`（全量整理→`SOLVE_RERANK_DAILY_TOKENS`；否则 `min(30 万, 每天额度 / 2)`）；`:488` `refusal_for_rerank`，`:504` 用有效上限比较，拒绝文案里的数字也是它；暂停、额度 0 的两句拒绝文案没动。`SOLVE_RERANK_DAILY_TOKENS`（`:61`）仍是 30 万。
2. **一次搜索最多一次请求、超时对齐**：`ai.rs:48` `AiCallOptions`（`max_tokens` / `timeout_secs` / `max_attempts` / `single_request` / `bill_slow_failures`），`:66` `Default` = 旧行为，`:81` `SOLVE_RERANK`；`:658` `complete_solve_rerank`、`:663` `complete_solve_rerank_with`；`:1058` `request_openai_with`、`:1178` `request_anthropic_with`、`:1369` `build_openai_chat_request_with`、`:2218` `execute_json_post_with`、`:2550` `waited_for_response`、`:2537` `format_ai_request_error_after`（超时文案写实际等了多久）、`:2568` / `:2580` 重试判断带 `max_attempts`。旧函数（`request_openai` `:1053`、`request_anthropic`、`build_openai_chat_request`、`execute_json_post` `:2206`）保留原签名，只是转调 `_with(.., default)`，所以现有测试一个调用点都不用改。`spend_guard.rs:304` `account_slow_failure`。`lib.rs:1281` 命令等 `solve_rerank::WAIT_SECONDS`（95 秒）。
3. **没有有效判断不算成功**：`solve_rerank.rs:152` `Fit::try_parse`（旧 `Fit::parse` 行为不变，仍被 `read_fits` 用）、`:1104` `read_fits` 多返回「认得的个数」、`:1029` `if recognized == 0 { return None }`。
4. **单飞**：`:258` `rerank_with` / `:278` `rerank_with_clock`（顺序：本地检查 → 读缓存 → 单飞 → 登记后再读一次缓存 → 断路器 → `call_model` → 写缓存 → 记断路器）、`:424` `flights`、`:448` `join_flight`、`:473` `FlightGuard` + `:486` `Drop`、`:504` `wait_for_flight`、`:519` `follower_view`、`:348` `call_model`。
5. **断路器**：`:539` `breakers`、`:553` `breaker_admit`、`:571` `breaker_record`，常量 `:42`–`:52`。
6. **理由里的网址**：`:1206` `clean_phrase`（先去 http 开头的片段 = 旧行为，再用 `:1285` `looks_like_link` 检查剩下的）、`:1261` `LINK_ENDINGS`、`:1267` `is_invisible`、`:1306` `domain_ending`。
7. **文档**：`docs/rebuild/命令契约.md` §10：status 表里 `skipped` / `failed` 两处；「额度」一条改写；新增「一次调用一个请求」「同一问题单飞」「断路器」「回复校验」四条（卡写的是补「一条」，我分成四条写清楚）。别处没动。

## 新增常量及取值理由
- `SOLVE_RERANK_MAX_TOKENS = 4096`（`ai.rs:32`）：回复只有一段 JSON（约 1k token），但思考型模型的思考 token 算在输出额度里，太小会「只有思考没有正文」；不设上限则失控的模型写到超时。Anthropic 路径原来 1200。OpenAI 兼容路径用的是 `max_tokens`（项目里理解卡流水线 `card_chat` 一直在用的写法，LongCat 这类 OpenAI 形状的服务认它）。
- `SOLVE_RERANK_TIMEOUT_SECONDS = 90`（`:37`）：思考型模型约 60 token/s，写满 4096 约 70 秒，加读提示词的时间，90 秒够用；比命令的 95 秒小。
- `SOLVE_RERANK_MAX_ATTEMPTS = 1`（`:40`）：卡的要求；原来 3 次 × 120 秒、超时也重试。
- `WAIT_SECONDS = 95`（`solve_rerank.rs:39`）：命令的兜底等待，也是后来者等先到那次的上限；一个常量两处用，所以 `lib.rs` 不再有写死的数字。
- 断路器：`BREAKER_FAILURES = 2`、`BREAKER_PAUSE_SECONDS = 600`（卡的要求）；`BREAKER_PROBE_LEASE_SECONDS = WAIT_SECONDS`（95 秒，见「偏离」）；`BREAKER_CLOCK_SLACK_SECONDS = 3600`（时钟往回拨一小时以内照常算暂停，再远当作时钟坏了）。
- `LINK_ENDINGS` 12 个后缀：照卡抄（com cn io ai app dev org net co me top xyz）。

## 最终超时那一笔具体记多少、怎么算
`spend_guard::account_slow_failure(max_output_tokens)`（`spend_guard.rs:304`）：这一笔 = **`max(1 万, 提示词估计 + 4096)`**。做法按卡的建议：算出「提示词估计之外还要补的部分」`top_up = 那个数 − 提示词估计`，加进 `EXTRA_TOKENS`，凭证放掉时（`SpendPermit::drop`，没改）和提示词那部分一起结算成 `cancelled` 行（`input_tokens = 提示词估计 + top_up`、`output_tokens = 0`）；同时把这次标成「可能已计费」，之后 `release_unbilled` 退不掉；`record` / `drop` / `release_unbilled` 都没改。
- 1 万 = `jobs::TIMEOUT_ATTEMPT_TOKEN_ESTIMATE`（队列对超时的记法）；`提示词估计 + 4096` 是「把输出额度写满」的最坏情形。AI 精选提示词上限 6500 字，估计最多 6500，所以通常（提示词估计 ≤ 5904）账本里正好是 **10 000**（例：1000 的提示词 = 1000 + 补 9000）；估计 6500 的最长提示词 = 10 596。测试：`a_final_timeout_books_at_least_what_the_queue_books_for_one`（10000）、`a_slow_failure_with_a_long_prompt_is_booked_at_the_worst_case`（6500→10596）。
- 退回与保留照旧：服务商直接拒绝（4xx、429）、没连上、几秒内的 5xx / 断线→整笔退回（0）；200 带用量→真实用量；200 读不出内容 / 空内容→照旧带用量记账。
- **范围比卡宽一点（要总控认）**：卡只写了「最终超时」。我把同一条规则用到另外三种「等了很久才失败、没拿到可用回复、服务商多半在算」的情形（只对 AI 精选，由 `bill_slow_failures` 开关控制）：①等了一阵（≥ `FAST_FAILURE_SECONDS` = 10 秒）才回的 5xx / 408（常见的网关 504）；②等了一阵才断开的连接；③200 的状态行收到了、响应体却读不完（超时或被断）。理由：这几种旧规矩只留提示词那部分，而 AI 精选的输出最多 4096，网关 504 常常恰好发生在模型写了一大半的时候；按「宁多勿少」记 1 万更保守。想只保留卡的原样（只有最终超时）：把 `ai.rs:2272` 的 `may_be_billed && options.bill_slow_failures` 改回 `waited_out`，并去掉 `:2291` 和 `:2318` 两处调用，对应改 `a_failure_after_a_long_wait_is_booked_like_a_timeout_but_a_rejection_never_is` 和 `a_rerank_call_is_exactly_one_request_whatever_comes_back` 里响应体半截那一行的期望。

## 单飞和断路器的状态：放哪、怎么清理
- **单飞**：`solve_rerank.rs` 里 `flights()` 函数内的 `static FLIGHTS: OnceLock<Mutex<HashMap<String, Arc<Flight>>>>`。键 = 数据库路径 + `\n` + 缓存键（缓存是每个库自己的；生产里只有一个库，等于「按缓存键」；带上路径是为了测试里各自的临时库互不牵连）。清理：先到的那次拿着 `FlightGuard`（RAII），`Drop` 先撤销自己的登记（用 `Arc::ptr_eq` 确认是自己的），再把结果（`None` = 它 panic 了）交给 `Flight.state` 并 `notify_all`；成功、失败、panic、提前返回都走同一条。后来者只在等待期间拿着 `Arc`。撤销在交结果之前，所以撤销之后再来的调用是新的一次，并且登记之后会再读一次缓存（先到的先写缓存再撤销登记）。不新增依赖，用 `Mutex` + `Condvar`。
- **断路器**：`breakers()` 里的 `static BREAKERS: OnceLock<Mutex<HashMap<String, BreakerState>>>`，键 = 数据库路径 + `\n` + 模型名，值 = `{ 连续失败次数, 暂停到的时刻 }`。清理：一次成功就 `remove` 这一项；所以只会留下「一直在出错」的模型，至多寥寥几项；不持久化，重启 App 清零。
- 两处的锁都不用 `unwrap`：遇到被毒化的锁就接着用里面的数据（`locked()`，有测试）；`ask` 在任何锁外面执行。

## 没做到的 / 偏离任务卡的地方
1. **三处现有测试的夹具必须改（断言一个没动）**——它们的预设和卡的新规则直接冲突：
   - `spend_guard::tests::refusal_for_rerank_is_a_plain_table`：表里「有额度」写的是 `control(1, false)`，新规则下每天额度 1 的有效上限是 0。4 行 `control(1, false)` 改成 `control(AMPLE, false)`（`AMPLE = 600_000`，上限正好 30 万），每一行的期望结果（`is_ok` / `is_err`）都没改。
   - `spend_guard::tests::rerank_stops_at_its_own_daily_allowance_and_says_so`：测的是「30 万」这个上限本身，原来用 10 万额度，新规则下上限只有 5 万。`BUDGET_100K` 换成新增的 `BUDGET_600K`，断言（含 29.5 万 + 5000 放行）一字没改。
   - `solve_rerank::tests::bad_fit_becomes_maybe`：它的回复 `great` / `BEST` 没有一个认得的判断，新规则下该是 failed。加了第三个候选 `c`（判断 `poor`，认得），请求改成 a、b、c；`great`→maybe、`BEST`→maybe 的原三条断言（`items[0]`、`items[1]` 是 maybe、`none_fit`）一字没改，只多加了 `status == "ok"` 和 `items[2] == poor` 两条。
   - 另有 `spend_guard::tests::rerank_usage_older_than_a_day_is_forgotten` 里一句注释（「超过 30 万」）改成了「超过上限」，断言没动。
   - 测试模块里新加的 `use`（`mpsc` / `thread` / `AtomicBool` / `Ordering` / `Arc` / `Mutex`）不算调用点。
2. **理由里 `http(s)://` 的语义**：卡写「片段里含 `://` → 整条理由当作空」，但现有测试 `reason_is_cleaned_truncated_and_defaulted` 要求「好用 … http://evil.example/a 真好」去掉链接后 `好用`、`真好` 还在（断言不许改）。所以我保留旧步骤（先去掉以 `http` 开头的片段，旧行为），**再**检查剩下的文字：还含 `://`（任何别的协议，如 `hxxps://`、`ftp://`）、`www.`、`字母数字.com` 这类域名写法，就整条清空。结果：被去掉的 http 链接不会显示；别的网址写法整条回落到兜底文案；`https://x.y` 单独成理由时回落兜底，「详见 https://x.y 了解」会变成「详见 了解」（旧行为）。想让 http 链接也整条清空，必须同时改那条旧测试的两句断言，这是总控的决定。
3. **后来者不读缓存而用先到者的内存结果**：卡写「先到的成功 → 读缓存返回 cached」。可观察结果相同（`cached`、`tokens == Some(0)`、items 一致）；好处是缓存写失败（数据库忙）时后来者仍拿得到结果。
4. **旧函数的「线程这个选项」**：卡写选项要穿过 `request_openai` / `request_anthropic` / `build_openai_chat_request` / `execute_json_post`。我保留这四个函数的原签名（转调新的 `_with` 版本，选项在 `_with` 里穿过去），这样旧功能和现有测试一个字不动。
5. **卡外的加固（都只对 AI 精选生效，由 `AiCallOptions` 的字段控制，旧功能 `Default` 全关）**：
   - `single_request`（`ai.rs:2227`–`2245`）：**reqwest 的 blocking 客户端 `.timeout()` 不是合计时限**——只管「等响应头」，`Response::text()` 又给响应体一个同样长的新时限（我读了 `reqwest-0.13.4` 的 `blocking/client.rs:1487`、`blocking/response.rs:296`、`async_impl/client.rs:2653`、`body.rs:400` 确认）。所以响应头 85 秒到、响应体再慢慢写，工作线程能跑到约 175 秒，命令 95 秒报超时、后台还在跑、结果进缓存，正是这张卡要消灭的情形（上面的 Stall 测试看不出来，因为响应头立刻到）。修法：AI 精选用请求级 `.timeout(90 秒)`（从开始连接到读完响应体的合计），有测试 `the_rerank_deadline_covers_slow_headers_and_the_body_together`；旧功能的「各一个时限」另有测试钉住（`the_old_options_give_the_body_its_own_fresh_window`）。同一开关还做了：关掉 reqwest 自带的协议层重试（HTTP/2 被拒绝的流 / GOAWAY 会自动再发，最多 2 次，默认开）；不跟随重定向（3xx 当失败报告：307/308 会把提示词再 POST 到 `Location`）。「一次调用恰好一个请求」才是字面成立。
   - `bill_slow_failures` 把最终超时的记法扩到慢 5xx / 慢断线 / 响应体读不完，并用最坏情形 `提示词估计 + 4096` 兜底（见上一节）。
   - 断路器：放行试探时只记 95 秒的租约（`BREAKER_PROBE_LEASE_SECONDS`），失败才从失败那一刻停满 10 分钟。原来放行就把整整 10 分钟记上，试探那次被额度拒绝或 panic 就白白停 10 分钟还说「连续出错」。时钟往回拨的容忍从「必须不早于记下的那一刻」放宽到 1 小时（原来拨回 1 秒就会提前放行一个）。
   - 理由检测的「看不见的字符」从 7 种扩到零宽 / 方向隔离 / 异体字选择符 / 标签字符等整段区间（只用于判断，不改显示的文字）。
6. **没有联网验证**：reqwest 的行为只在本机 HTTP/1.1 假服务上验证；HTTP/2 的协议层重试被关掉这一条没法在本机测（没有 h2 假服务），靠读 `reqwest::retry::never()` 的文档和源码。

## 需要总控或 Fox 决定的事
1. 上面「偏离 2」：http(s) 链接是「去掉链接留下别的话」（现状，旧测试要求）还是「整条清空」（卡字面，要改旧测试断言）。
2. 上面「最终超时一笔」的范围：现状是把慢 5xx / 慢断线 / 响应体读不完也记到 `max(1 万, 估计 + 4096)`（更保守）；要退回只记最终超时，按那一节末尾的三处改。
3. **修复前已经写进缓存的「空结果」不会自动清**（全 maybe + 兜底建议的行，来自 `{}` / 编号对不上的回复）。迁移不许加，所以代码里没清；它们会在候选 / 理解卡变化或 300 条滚动里消失。要立刻清，可由总控在 dev 库副本上跑一条 `DELETE`（特征：每条理由都是「只是沾边」且 `noneFit` 为真），或我可以在 `read_cache` 里按这个特征当作未命中——没做，因为合法的「全 maybe」回复也可能长这样（极少）。
4. **断路器按模型名**（卡的设计）：改了密钥 / 地址但没换模型名之后，AI 精选最多还要等 10 分钟（或重启 App）才试。若要改善，可以把服务商、地址、密钥指纹带进键（`lib.rs` 传一个范围字符串），或保存设置时清零；没做。配置校验错误（`validate_request_config` 的各种「请填写…」）和 401 / 404 也算失败，会触发断路器。
5. 3 处现有测试的夹具调整（见「偏离 1」）。

## 没把握的地方
- **`max_tokens` 对 OpenAI 官方的推理模型**（o 系列 / GPT-5 要 `max_completion_tokens`）会 400 `Unsupported parameter`：AI 精选退回本地排序，断路器 2 次后停 10 分钟（400 不计费，预留退回）。卡明确要求 OpenAI 兼容路径带 `max_tokens`，LongCat 等 OpenAI 形状的服务不受影响；项目里 `card_chat` 早就认这个参数。没有联网验证。
- 预留额度的输出余量仍是 `max(3000, 提示词)`（没动 `begin`，现有断言是 4000），而 `max_tokens = 4096`：一次调用的真实用量最多比预留多约 1.1k；结算按真实用量，所以只影响「预留当下」的判断，硬上限最多被一次调用的这点余量超过。
- 命令 95 秒报超时之后，工作线程若还卡在钥匙串授权 / 数据库等待（总控的记忆里有等 Fox 点钥匙串的情形），之后仍会调一次模型。单飞限制了重复；没做「已放弃」标志——放弃的先到者会把假的失败结果发给仍在等的后来者，反而更糟。
- `store_cache` 失败（数据库忙超过 5 秒）会被忽略（原有行为）：下一次同问题会再调一次模型。
- 慢 5xx 的阈值沿用旧的 `FAST_FAILURE_SECONDS = 10`；`ASP.NET` / `Socket.IO` / `Fly.io` 这类带点的工具名按卡的规则会被当网址、理由回落成兜底文案（卡只要求 `Next.js` / `Node.js` / `ffmpeg.wasm` / `v1.2` 不误伤）。
- 重定向：旧功能照常跟随（没动）；AI 精选不跟随，所以 base URL 若会 30x 跳转，AI 精选会按失败处理（其它功能不受影响）。

## 改了哪些文件
- `apps/desktop/src-tauri/src/spend_guard.rs`：`rerank_cap` / `refusal_for_rerank`（有效上限）、`account_slow_failure`（慢失败补账）、头部注释、12 个新测试、2 处现有测试夹具。
- `apps/desktop/src-tauri/src/ai.rs`：`AiCallOptions` + `SOLVE_RERANK_*` 常量、`complete_solve_rerank(_with)`、`request_openai_with` / `request_anthropic_with` / `build_openai_chat_request_with` / `execute_json_post_with`、`waited_for_response`、`format_ai_request_error_after`、`should_retry_*_within`、19 个新测试（含一个数请求的假服务 `CountingServer`）。旧的 `ANTHROPIC_SOLVE_RERANK_MAX_TOKENS`（1200）删了，换成共用的 `SOLVE_RERANK_MAX_TOKENS`（4096）。
- `apps/desktop/src-tauri/src/solve_rerank.rs`：`rerank_with` / `rerank_with_clock` / `call_model`、单飞、断路器、`Fit::try_parse`、`parse_reply` / `read_fits`、`clean_phrase` / `looks_like_link`、34 个新测试、1 处现有测试夹具。
- `apps/desktop/src-tauri/src/lib.rs`：`solve_rerank_v2` 的等待改用 `solve_rerank::WAIT_SECONDS`（95 秒），注释更新；文案没变。
- `docs/rebuild/命令契约.md`：只改 §10。

## 验证
- `tools/cards/run.sh rust-fmt-fix` → 然后 `rust-fmt`：无输出（通过）。
- `tools/cards/run.sh rust-check` → 通过，6 个警告（基线 6 个，同一批：`MigrationReport` 字段、`README_RULE_CHARS`、`reserve_at`、`ensure_ready`、`repair_model_json`、`restore_database_file`）。
- `tools/cards/run.sh rust-test`（整套，跑了两次）→ 1200 通过 / 0 失败 / 5 ignored / 1 跳过（钥匙串用例）；基线 1135 / 0 / 5 / 1。测试构建警告 1 个（基线同样 1 个）。
- 新增的并发 / 联网类测试单独重复跑：`solve_rerank::tests` 连跑 6 次、`ai::tests` 连跑 3+ 次，全过，没有抖动。
- **变异测试**（总控的习惯）：对新代码逐个「改坏实现」约 90 次（有效上限不除以 2 / 不封顶 / 全量整理不放开 / 文案写成封顶值、`>` 变 `>=`、补账不补 / 补整数 / 不标已计费、选项里的次数 / 超时 / `max_tokens` 改值、旧入口误用新选项、请求级时限去掉、重定向放开、慢失败漏记 / 误记 4xx、单飞不去重 / 不撤销登记 / 后来者照样调 / 不转成 cached / 不管等待上限、断路器阈值 / 时长 / 租约 / 时钟容忍 / 成功不清零 / 后来者重复计数 / 本地失败计数 / 缓存在断路器之后、`recognized` 要求去掉 / 大小写不敏感、链接检测的各个后缀 / 边界 / 看不见字符 / `www.` / `://`、锁毒化时 panic、命令写死 60 秒……）。全部被测试杀掉；一开始有 2 个存活（失败后再停 10 分钟是从放行还是从失败算起、点前面不是字母数字的 `.NET` / `.com`），补了测试 `a_failed_attempt_pauses_for_ten_minutes_from_the_failure_not_from_the_start` 和 `tool_names_and_versions_with_dots_...` 里的用例后也杀掉了。唯一杀不了的是「关掉 reqwest 协议层重试」（需要 HTTP/2 假服务）。每次变异后原文件都已还原（最终 `git diff` 即为交付内容）。
- **独立复核**：派了两个只读的复核者（一个看记账和请求路径，一个看单飞 / 断路器 / 解析 / 网址检测），没有 BLOCKER。他们指出并已修的：reqwest 合计时限（MAJOR）、慢失败只记提示词、最终超时下限对长提示词偏低、断路器试探被「额度拒绝 / panic」白白占 10 分钟、时钟往回拨 1 秒就提前放行、看不见字符不全、`local_failures_*` 测试证明力不够、一处过期注释。没改、写进上面「没把握 / 需要决定」的：配置变化后的断路器、缓存写失败、「已放弃」标志、`max_tokens` 与 o 系列。

## 给审核者的提示
- 先看 `git diff -- apps/desktop/src-tauri/src/ai.rs` 的 `execute_json_post_with`（`:2218`–`:2327`）：默认选项下逐行等于旧的 `execute_json_post`；新增的都在 `options.single_request` / `options.bill_slow_failures` 后面。
- `solve_rerank.rs` 里有一个只在测试里存在的挂钩 `BEFORE_JOIN`（`#[cfg(test)]` 的 thread_local，`join_flight` 开头一行），用来确定性地复现「登记前别人刚写完缓存」的窗口；正式构建里没有。
- 新的 AI 精选状态是进程内的：`FLIGHTS` / `BREAKERS` 的键都带数据库路径，所以测试之间互不牵连；生产里只有一个库。
- 过程披露：我误把一个空文件写到了 `/tmp/dummy_check.txt`（工作区之外，里面没有任何内容），随即删除，已确认不存在；用 `python3` 脚本改过工作区里的源码 / 文档和 `.tmp-card/`（用完已删）；读过 `~/.cargo/registry` 里 `reqwest-0.13.4` 的源码（只读，用来核对超时 / 重试 / 重定向的真实行为）；变异测试脚本通过 `tools/cards/run.sh rust-test` 跑，没有直接运行 cargo；没有任何 git 写命令；没有碰 `~/Library/Application Support`、钥匙串、网络。
