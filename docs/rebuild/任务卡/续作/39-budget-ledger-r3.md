# 卡 39 续作（第 3 轮）：补 5 处测试漏洞，只补测试（总控，2026-10-05）

背景：你上一轮的实现，总控已在沙盒外跑完整 Rust 套件：**1360 通过、0 失败、5 忽略**。总控又对新逻辑做了 41 种「改坏实现」检查（每种把一处实现故意改坏、再跑你写的测试，看它会不会发现）：**33 种被测试抓住**；另有 8 种改坏之后测试仍然全绿，说明测试没守住。其中 **5 处是真漏洞**（下面逐条写），3 处是「改了也不影响任何结果」的等价改动（见末尾，不用管）。

这一轮**只补测试，不改实现**：

- 你上一轮的实现和测试**全部保留**，不要重做、不要改写已有断言和数字。
- 允许改动：`apps/desktop/src-tauri/src/jobs.rs`、`card_handler.rs`，而且**只能改各文件 `#[cfg(test)] mod tests` 里面**（新增测试；或在已有测试里追加断言）。`mod tests` 之外的代码（实现、常量、注释）一个字都不许动。`spend_guard.rs` 这一轮用不到。
- 如果补测试时发现实现本身有错（不是测试没写），**停手**，写进报告，不要改实现。

你这个沙盒里跑不了 Rust 测试（`ort-sys` 在沙盒里重新编译、又没有网络下载 ONNX，链接失败），所以这一轮改完只能跑 `rust-fmt`、`rust-check`；**每个新增或改动的测试，请你自己一步步推演**：数据 → 每日用量 / 结果 → 期望，并且写清「实现被改坏时它为什么会失败」。总控会把改坏版本放回去逐个验证，没抓住的会再退回给你。

## 要补的 5 条（每条先写「改坏版本」，方便你推演）

### 1. 队列状态要跟「每日用量」走，不能跟「原始 24 小时合计」走

- **改坏版本**：`jobs.rs::queue_status` 里调用 `queue_state(...)` 的那个参数，把 `daily_tokens_used` 换成 `tokens_24h`。现在所有测试仍然全绿。
- **为什么没抓住**：断言 `state == "idle"` 的测试里，整理用的 token 都很小（5000），原始合计加上认领预留也没超过额度；真正超过的那个测试（`automatic_usage_still_limits_claims_after_an_expensive_full_scan`）两种算法都是 `budget_reached`。**这恰好是 Fox 看到「今天的额度用完了」的那个 bug 场景，必须有测试守住。**
- **要做**：`jobs.rs` 的 `mod tests` 里新增一个测试。数据：额度 50,000；`recent_full_scans` 里写一轮已结束的整理，时段内有一个已成功任务 3,000,000 token，时段外（时段开始前）有一个已成功任务 100 token（`open_job_row` 的用法照 `daily_budget_excludes_full_scan_usage_and_preserves_raw_24h_usage`），没有排队 / 进行中的任务、没有暂停。断言 `queue_status` 的：`state == "idle"`、`daily_tokens_used == 100`、`tokens_24h == 3_000_100`、`budget_remaining == 49_900`。再往下走一步（同一个测试里）：把时段外那个自动任务改成 `input_tokens = 40_000`（和上面已有的 `budget_reached` 测试同量级），断言 `state` 变成 `"budget_reached"`、`budget_remaining == 0`，说明「每天额度确实还在管自动用量」。

### 2. 历史去重（`finish_full_scan` 里的 `periods.dedup()`）

- **改坏版本**：`finish_full_scan` 里删掉 `periods.dedup();`。
- **为什么没抓住**：现有「幂等」测试靠的是别的机制（`fullScan` 已经不在了就什么也不做），文件里本来就有重复项的情况没人测。
- **要做**：新增测试：手写控制文件，`recentFullScans` 里放三项——`P1`、**和 `P1` 完全相同的另一项**、`P2`（都在 25 小时内、起止合法、互不重叠）；然后 `begin_full_scan` + `stop_full_scan`（沿用 `recent_scan_history_prunes_older_than_25_hours_and_keeps_latest_16` 的写法）。**读回原始 JSON 文件**（`serde_json::from_slice` 读 `queue_control_path(...)`，不要用 `read_control`），断言 `recentFullScans` 数组恰好是 `[P1, P2, 新的一轮]`，每项一次、按结束时间从早到晚。

### 3. 写入端也只留最近 16 项（`finish_full_scan` 里的 `periods.drain(..excess)`）

- **改坏版本**：把 `periods.drain(..excess);` 改成什么也不做（`let _ = excess;`）。
- **为什么没抓住**：现有测试 `recent_scan_history_prunes_older_than_25_hours_and_keeps_latest_16` 是用 `read_control` 读的，而读取端本来就只取最近 16 项，所以文件里存了 20 项也看不出来。
- **要做**：在这个测试的末尾追加（或新写一个测试）：直接读**原始 JSON 文件**，断言 `recentFullScans` 数组长度等于 `MAX_RECENT_FULL_SCANS`（16），并且内容正是第 5～20 轮（和已有断言同一批数据）。

### 4. `awaiting` 只算「理解卡」任务（`organizing_counts` 里的 `job_type = 'repo_card'`）

- **改坏版本**：`card_handler.rs::organizing_counts` 里 `awaiting` 那条查询的第二段，把 `WHERE job_type = 'repo_card' AND status IN ('queued', 'running')` 改成 `WHERE status IN ('queued', 'running')`（去掉类型限制）。
- **为什么没抓住**：现有测试里排队的任务全是 `repo_card`。但库里还有别的任务类型（比如 `categorize`，同步后给新收藏归类），它们的 `target_id` 同样是仓库 id，不能算进「在等整理的理解卡」。
- **要做**：在 `automation_status_splits_missing_outdated_and_deduplicates_awaiting_repos`（`card_handler.rs`，约 2253 行）的数据里，再排一个别的类型的任务：`('c-categorize', 'acct', 'categorize', 'repository', 'c', 'queued')`——`c` 是新版说明、本来不在待整理里；断言 `awaiting` 仍然是 4（注释里写清为什么），把它改成 `running` 后仍然是 4。**不要改这个测试已有的断言和数字**；追加断言，或者另写一个测试都行。

### 5. 「时间读不懂就不生成免计时段」（`scan_period_end` 先解析 `now`）

- **改坏版本**：删掉 `scan_period_end` 里的 `parse_db_time(now).ok()?;` 这一行。
- **为什么没抓住**：函数的注释写明了「时间读不懂就不生成免计时段」，但只有 `started_at` 读不懂的情况可能被测到，`now` 读不懂没人测。（生产里 `now` 永远来自时钟、不会读不懂；这条是把文档里写的约定用测试钉住。）
- **要做**：`jobs.rs` 的 `mod tests` 里新增一个测试，直接调 `scan_period_end`：
  - 正常的 `FullScan`（用 `full_scan_control` / `begin_full_scan` 之类现成的夹具造一个）+ 一个读不懂、而且**按字典序排在 48 小时线后面**的 `now`（比如 `"z-not-a-time"`）→ 断言返回 `None`（改坏版本会因为 `now >= deadline` 返回 `Some(48 小时线)`）；
  - 同一个 `FullScan` + 正常的 `now`（开始后 1 小时）→ `Some(now)`；`now` 在 48 小时线之后 → `Some(48 小时线)`（这两条别的测试已经覆盖，写上是为了让这个测试自己能说明「读不懂」和「正常」的区别，可以很短）；
  - `started_at` 读不懂的 `FullScan` + 正常 `now` → `None`。

## 不用做的 3 处（总控已确认是「等价改动」：改了之后任何输入的结果都一样，不要改实现、也不要为它们写测试）

- `parse_db_time` 里的 `!bytes.is_ascii()`：所有切片边界都落在固定位置的 ASCII 分隔符两侧，非 ASCII 字符只会让 `parse` 失败（结果同样是 `Err`），不可能 panic。
- `append_last_scan_fallback` 里的「历史里已有同一轮」判断：没有 `recentFullScans` 数组的文件，`recent_full_scans` 一定是空的，这个判断永远为真。
- `organizing_counts` 里「没有说明」那一项的 `NOT_CONFIRMED_SQL`：没有说明的行 `card_json` 是空的，`NOT_CONFIRMED_SQL` 必然为真。

## 验收

1. `tools/cards/run.sh rust-fmt`、`rust-check` 通过；Rust 测试跑不了就如实写。
2. `git diff` 相对你上一轮的工作区，只增加 `mod tests` 里的内容；实现代码零改动（总控会逐行对比）。
3. 报告里写：每条 → 新增测试名、文件:行、数据 → 推演 → 期望，以及「改坏版本为什么会失败」的一句话；没把握的地方。
4. **不要**改实现、前端、依赖、迁移、提示词、确认门槛、48 小时、刹车线、命令参数；不要碰 `refusal_for_rerank`、`rerank_*`、`rerank_tokens_used_since`。
