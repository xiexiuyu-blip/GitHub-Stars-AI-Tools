---
编号: 34-rerank-hardening
允许改动:
  - apps/desktop/src-tauri/src/spend_guard.rs
  - apps/desktop/src-tauri/src/ai.rs
  - apps/desktop/src-tauri/src/solve_rerank.rs
  - apps/desktop/src-tauri/src/lib.rs
  - docs/rebuild/命令契约.md
---

# 卡 34 · 找方案 AI 精选：花钱路径加固（Rust）

## 为什么有这张卡
找方案的 **AI 精选默认开启，用户每搜一句就会调用一次模型**，所以它的花钱路径任何一个小漏洞都会被放大。只读审查员（2026-10-05）按「额度是硬上限」的铁律读了这条路径，找到 5 处必须修的问题（下面逐条写成要做的事）。这张卡**只改 Rust**；对应的前端小修在卡 35。

## Fox 能看到的效果
- 把每天额度调小（比如试跑 10 万），AI 精选也跟着少花（最多每天额度的一半），不会再出现「额度只有 5 万、AI 精选却能花 30 万、后台整理被挡一整天」。
- AI 精选卡住时，最多等 90 秒就如实说「AI 精选超时了，这次按本地排序」，**后台不会再悄悄重试 3 轮、多花钱**。
- 连点「找」、把「AI 精选」开关关了又开，**同一个问题不会花两次钱**。
- 模型回了个空的 `{}` 或编号全对不上，不会再被当成「成功」并显示「收藏里没有专门做这件事的工具」，而是按失败处理（本地排序、不缓存）。
- 服务商连续出错时，10 分钟内不再每搜一次都白等、白记账。

## 背景与先读什么
- `apps/desktop/src-tauri/src/spend_guard.rs`：**先读文件头部的设计注释**（预留行、24 小时滚动窗口、`account_failed_attempt` / `release_unbilled` / `MAYBE_BILLED` 的含义）；然后读 `solve_rerank`、`refusal_for_rerank`、`reserve_with`、`SpendPermit` 的 `Drop`、`record`。这套记账规矩是前面 7 轮审查一点点磨出来的，**不要改旧功能（`Policy::Legacy`）的任何行为**。
- `apps/desktop/src-tauri/src/ai.rs`：`complete_solve_rerank`（约 593 行）、`request_openai`（约 979 行）、`request_anthropic`、`build_openai_chat_request`（约 1264 行，**现在不带 `max_tokens`**）、`execute_json_post`（约 2096 行：3 次尝试 × 120 秒，超时也重试）、常量 `AI_API_MAX_ATTEMPTS`、`AI_REQUEST_TIMEOUT_SECONDS`、`ANTHROPIC_SOLVE_RERANK_MAX_TOKENS`。
- `apps/desktop/src-tauri/src/jobs.rs` 约 79 行：`TIMEOUT_ATTEMPT_TOKEN_ESTIMATE`（等到超时才失败的调用，队列记 1 万，理由写在注释里）。
- `apps/desktop/src-tauri/src/solve_rerank.rs`：`rerank_with`（约 215 行）、`parse_reply`（约 710 行）、`read_order` / `read_fits`、`clean_phrase`、`reason_text`。
- `apps/desktop/src-tauri/src/lib.rs` 约 1271–1309 行：`solve_rerank_v2` 命令和 `solve_rerank_v2_worker`。
- 测试夹具：`spend_guard::testing::Dir`（临时库 + 控制文件，`pub(crate)`）；`ai.rs` 测试里的 `spawn_http_server` / `spawn_http_sequence_server`（本地假服务）；`solve_rerank.rs` 现有测试里「假 `ask` 闭包 + 临时库」的写法。**所有新测试都不许联网**。

## 要做的事

### 1. AI 精选的额度跟着每天额度走（`spend_guard.rs`）
`refusal_for_rerank` 现在的上限是常量 `SOLVE_RERANK_DAILY_TOKENS`（30 万）。改成**有效上限**：
- 全量整理在进行（`scan_open`）→ `SOLVE_RERANK_DAILY_TOKENS`；
- 否则每天额度 > 0 → `min(SOLVE_RERANK_DAILY_TOKENS, 每天额度 / 2)`；
- 每天额度是 0 且没有全量整理 → 照旧拒绝（文案不变）；暂停照旧拒绝。

拒绝文案里的「每天 N 的额度」写**有效上限**。`SOLVE_RERANK_DAILY_TOKENS` 仍是上限的上限，常量不改。

### 2. 一次搜索最多一次模型调用，超时和界面等待对齐（`ai.rs`、`lib.rs`）
现在 `complete_solve_rerank` 走旧功能的 `request_openai` / `request_anthropic`，里面的 `execute_json_post` 最多 3 次尝试、每次 120 秒，超时还会重试；而命令 `solve_rerank_v2` 只等 60 秒就返回「超时」，后台线程却继续跑（钱照花、结果进缓存、界面只看到失败）。改成：
- 新增调用选项（名字自定，例如 `AiCallOptions { max_tokens: Option<u32>, timeout_secs: u64, max_attempts: u32 }`），**`Default` / 旧入口的行为和现在完全一样**（旧功能的函数签名和行为不变，现有测试里除了为适应新签名必须改的调用点，一个断言都不许改）。线程这个选项穿过 `request_openai` / `request_anthropic` / `build_openai_chat_request` / `execute_json_post`。
- `complete_solve_rerank` 用：`max_tokens = 4096`（OpenAI 兼容路径**也要带上**，Anthropic 路径从 1200 提到 4096——思考型模型的思考 token 算在输出额度里，太小会「只有思考、没有正文」）、`timeout_secs = 90`、`max_attempts = 1`。常量放在 `ai.rs` 里并写注释说明为什么。
- **一次调用恰好一个 HTTP 请求**，无论响应是 200 空内容、429、500、网络错误还是超时，都不重试、不回退到别的请求。
- 记账：
  - 服务商直接拒绝（4xx、429）、根本没连上、几秒内的 5xx：照现有规矩 `release_unbilled`（这几种现在已经退回，保持）。
  - **最终超时**（请求发出去、等到 `timeout_secs` 才失败）：账本里这一次要**不少于 `jobs::TIMEOUT_ATTEMPT_TOKEN_ESTIMATE`**（1 万）。做法建议：在 `spend_guard.rs` 加一个小函数（例如 `account_slow_timeout()`），把「提示词估计之外还要补的部分」加到 `EXTRA_TOKENS`，凭证放掉时一起结算；不要改 `record` / `SpendPermit::drop` 的既有逻辑。
  - 200 但读不出内容 / 内容为空：现有逻辑已经带用量记账，保持。
- `lib.rs`：命令的等待时限从 60 秒改成 **95 秒**（略大于请求的 90 秒，这样工作线程总是先自己结束；这个等待只是兜底），超时文案不变。

### 3. 回复里一个有效判断都没有，不算成功（`solve_rerank.rs`）
`parse_reply` 现在对 `{}`、`{"fit":{"foo":"bad"}}`、编号全对不上的回复也返回 `Some`（全 `maybe`、`none_fit = true`、建议「收藏里没有专门做这件事的工具」），还会被写进缓存，之后同一问题永远命中这份空结果。改成：**回复里至少有一个候选的 `fit` 是认得的合法值（`best` / `good` / `maybe` / `poor`，按现有 `Fit::parse` 认的写法），否则 `parse_reply` 返回 `None`**（上层已有的路径会给 `failed`、不写缓存）。`Fit::parse` 现在对不认识的写法默认 `maybe`，你需要一个能区分「认得 / 不认得」的版本（例如 `Fit::try_parse`），旧入口行为不变。部分有效的回复（只给了 `A2: good`）仍然成功，缺的补 `maybe`，这是现有行为，保持。

### 4. 同一问题单飞（`solve_rerank.rs`）
现在缓存要等调用结束才写，两个相同的请求（连点「找」、AI 开关关了又开、两个窗口）会各调一次模型。改成**进程内按缓存键单飞**：
- 同一个缓存键已经有一次在飞：后来的调用**不发模型请求**，等先到的那次结束（等待上限 95 秒），然后：先到的成功 → 读缓存返回 `cached`、`tokens == Some(0)`；先到的失败 / 拒绝 / 跳过 → 返回和先到的同样的结果（`failed` 等），**不再自己发请求**。
- 不同缓存键互不等待。
- 先到的那次结束（成功、失败、panic、提前 return）都要把「在飞」登记清掉（用 RAII 守卫）；清掉之后再来的调用（比如失败后用户重试）是新的一次，会重新调用模型。
- 实现可以用 `Mutex<HashMap<String, Arc<…>>>` + `Condvar`；不引入新依赖。

### 5. 简单的断路器（`solve_rerank.rs`）
- 进程内、按模型名记「连续失败次数」。只有 `failed`（调用出错、超时、回复看不懂）算失败；`refused`（额度）、`skipped`（没配置、问题为空、候选不足）、`cached` 都**不算**，也不清零；一次真正的成功（`ok`）清零。
- 连续失败达到 **2 次**后，接下来 **10 分钟**内，对这个模型的 `rerank_with` 直接返回 `skipped`、`message = "AI 精选连续出错，先停 10 分钟，这次按本地排序"`，**不调用 `ask`**。10 分钟过后允许一次尝试：失败就再停 10 分钟（计数保持 ≥ 2），成功就清零。
- 时间用可注入的时钟（例如 `rerank_with_clock(…, now_secs: u64)` 由 `rerank_with` 调用并传真实时间），单元测试不许 sleep。
- 命中缓存、额度拒绝发生在断路器检查之前还是之后由你定，但必须保证：断路器打开时**缓存命中仍然正常返回**（读缓存不花钱）。

### 6. 理由里的网址（`solve_rerank.rs`）
`clean_phrase`（理由和建议都走它）现在只拦以 `http` 开头的。补：片段里含 `://`、`www.`，或像域名的写法（字母数字加 `.` 加 `com` / `cn` / `io` / `ai` / `app` / `dev` / `org` / `net` / `co` / `me` / `top` / `xyz`，不区分大小写）时，**整条理由当作空**（走现有的「理由为空」兜底文案）。`Next.js`、`Node.js`、`ffmpeg.wasm` 这类带点的工具名不许被误伤（测试里要有这几个）。

### 7. 文档（`docs/rebuild/命令契约.md` §10）
把 §10 里关于额度的那条改成：AI 精选的额度是「每 24 小时最多 30 万，且不超过每天额度的一半（一次性全部整理进行时按 30 万）」；补一条：一次调用只发一个请求、`max_tokens = 4096`、请求超时 90 秒、命令最多等 95 秒；同一问题单飞；连续失败 2 次停 10 分钟；回复里没有任何合法判断算失败。只改 §10，别处不动。

## 不要做
- 不改旧功能（`Policy::Legacy`、`request_openai_streaming`、其它 `complete_*`、翻译、队列认领）的任何行为或常量。
- 不改 `jobs.rs`、迁移、前端、`solve.rs`；不新增依赖；不加新的 Tauri 命令。
- 不把重试加回来；不改 `SOLVE_RERANK_TAG` 和账本行的格式。
- 不动 `permissions` / `acl-manifests.json`（没有新命令）。

## 验收
1. `tools/cards/run.sh rust-test` 全部通过；新增 Rust 测试**至少 26 个**（临时库 + 本地假服务 + 假 `ask`，不联网，不 sleep 超过 1 秒），覆盖：
   - 额度联动：每天额度 0 + 无全量 → 拒绝；0 + 全量 → 上限 30 万；10 万 → 5 万；60 万 → 30 万；1000 万 → 30 万；暂停 → 拒绝；`已用 + 预计 == 上限` 放行、`+1` 拒绝；拒绝文案里的数字是有效上限；经过 `reserve_with` 的集成用例（10 万额度、AI 精选已用 4.5 万，预计 1 万 → 拒绝）。
   - 单次调用：本地假服务计数——200 空内容 / 429 / 500 / 400 / 响应体读到一半断开，每种情况**请求数都恰好是 1**；请求体里 `max_tokens == 4096`（OpenAI 兼容和 Anthropic 各一个）；旧入口 `request_openai` 的请求体**仍然没有** `max_tokens`、500 仍然会重试（现有测试照常通过）。
   - 记账：4xx / 429 → 账本里这一次是 0（预留已退回）；最终超时（假服务不回应，测试里把 `timeout_secs` 设成 1）→ 账本里这一行 `input_tokens + output_tokens >= jobs::TIMEOUT_ATTEMPT_TOKEN_ESTIMATE`、状态不是 `running`；200 带真实用量 → 记真实用量。
   - 命令等待时限：常量大于请求超时（用 `assert!` 写成测试，免得以后有人改一个忘了另一个）。
   - `parse_reply`：`{}`、`{"order":["x"],"fit":{"foo":"best"}}`、`{"fit":{"A1":"great"}}` → `None`；`{"fit":{"A2":"good"}}` → `Some`，其余补 `maybe`；上层用 `rerank_with` 验证：这三种回复 → `failed`、**不写缓存**、`ask` 只被调用 1 次。
   - 单飞：两个线程同一问题 + 一个慢的假 `ask` → `ask` 只调 1 次，第二个返回 `cached`；先到的失败 → 两个都是 `failed`、`ask` 1 次；失败后第三次调用重新调 `ask`；不同问题并发时各调各的（`ask` 2 次）；守卫在 panic 时也清登记（用 `catch_unwind` 或线程 join 验证）。
   - 断路器：连续 2 次 `failed` → 第 3 次 `skipped` 且 `ask` 不被调用；`refused` / `skipped` / `cached` 不计数；成功清零；10 分钟后允许一次，再失败又停；断路器打开时缓存命中仍返回 `cached`；不同模型名互不影响。
   - 清洗：含 `https://x.y`、`www.x.com`、`foo.com/bar`、`evil.xyz` 的理由 → 兜底文案；`Next.js`、`Node.js`、`ffmpeg.wasm`、`v1.2` 不受影响；建议（note）同样处理。
2. 现有测试**一个断言都不许放宽或删除**（为适应新签名而改的调用点除外，报告里列出来）；`tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多。
3. 旧功能没被改动：`git diff` 里 `Policy::Legacy` 分支、`request_openai_streaming`、`execute_json_post` 的默认路径没有行为变化（报告里指出新增的分支在哪几行）。
4. `git diff --stat` 里只出现「允许改动」里的文件。
5. 报告里写：每个要做的事对应的代码位置（文件:行）；新增的常量及取值理由；最终超时那一笔具体记多少、怎么算出来的；单飞和断路器的状态放在哪、怎么清理；任何你觉得和审查结论不一致的地方；没把握的地方。

## 停止条件
- 需要改旧功能的行为、`jobs.rs`、迁移、前端，或需要新依赖——停手并写进报告。
- 发现「一次调用一个请求」必须改 `execute_json_post` 的默认路径才能做到——停手并写进报告（不要改默认路径）。
