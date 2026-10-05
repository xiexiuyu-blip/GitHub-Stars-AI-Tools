# 交回报告 · 卡 39-budget-ledger

## 一句话结论

第 3 轮要求的 5 处测试缺口已补齐：新增 3 个测试、增强 2 个现有测试；实现零改动，Rust 格式和编译检查通过，测试执行及变异验证待总控在沙盒外完成。

## 验收逐条

本轮按只读交接文件 `39-budget-ledger-r3.md` 执行，保留入场时上一轮的全部实现、测试和断言。

| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| 续作 1：`tools/cards/run.sh rust-fmt`、`rust-check` 通过；Rust 测试跑不了就如实写。 | 是 | 两条命令退出码均为 0；`rust-check` 完成，输出 6 条未使用代码警告。本轮未运行 Rust 测试，原因见下。 |
| 续作 2：`git diff` 相对你上一轮的工作区，只增加 `mod tests` 里的内容；实现代码零改动（总控会逐行对比）。 | 是 | 与本轮入场快照比对：`jobs.rs` 只新增 102 行、`card_handler.rs` 只新增 19 行；两文件测试模块前内容逐字一致，原有全部行按原顺序保留；`spend_guard.rs` 全文一致。 |
| 续作 3：报告里写：每条 → 新增测试名、文件:行、数据 → 推演 → 期望，以及「改坏版本为什么会失败」的一句话；没把握的地方。 | 是 | 下方逐条推演列出全部 5 条；明确区分推演与实际测试执行。 |
| 续作 4：**不要**改实现、前端、依赖、迁移、提示词、确认门槛、48 小时、刹车线、命令参数；不要碰 `refusal_for_rerank`、`rerank_*`、`rerank_tokens_used_since`。 | 是 | 本轮差异只含两文件测试模块内的新增内容；实现及 `spend_guard.rs` 与入场快照一致。 |
| 原卡 1：`tools/cards/run.sh rust-fmt`、`rust-check`、`rust-test`（可带过滤词）通过；沙盒里跑不了的测试如实写进报告，总控在沙盒外跑完整套。 | 部分 | 格式、编译检查通过；按第 3 轮交接说明不运行受 ONNX 链接限制的测试，实际测试通过数未取得。 |
| 原卡 2：`git diff --stat` 只出现「允许改动」里的文件。 | 是 | 只出现 `jobs.rs`、`card_handler.rs`、`spend_guard.rs`；最后一个文件的差异全部来自入场前，本轮未修改。 |
| 原卡 3：报告里写：条号 → 文件:行；新增 / 改动的测试名；`BudgetGate` 怎么带上时间段、是否去掉了 `Copy`；没把握的地方。 | 是 | 5 条测试位置及推演见下；`BudgetGate` 使用 `Vec<FullScanPeriod>`，已去掉 `Copy`，本轮保留原实现，见 `apps/desktop/src-tauri/src/jobs.rs:164`、`:885`。 |

### 续作第 1 条：队列状态使用每日用量

改坏版本：`queue_status` 调用 `queue_state` 时，把 `daily_tokens_used` 换为 `tokens_24h`。

新增测试：`queue_status_uses_daily_usage_after_an_expensive_full_scan`，`apps/desktop/src-tauri/src/jobs.rs:3639`。

数据 → 推演 → 期望：

1. 取当前时刻 N；自动任务在 N−9000 秒成功，100 token；整理时段为 N−7200 至 N−3600 秒，时段内 N−5400 秒有成功任务 3,000,000 token。两条任务均在最近 24 小时内，都是已结束任务；无排队、进行中任务，未暂停，未开启不限额；额度为 50,000。
2. 原始合计为 3,000,100；每日口径只含自动任务的 100。每日用量加认领预留为 100+12,000=12,100，未超过额度；余额 50,000−100=49,900，足够放下预留。断言 `state == "idle"`、`daily_tokens_used == 100`、`tokens_24h == 3_000_100`、`budget_remaining == 49_900`（`:3663`）。
3. 同一测试把自动任务输入用量改为 40,000：每日用量 40,000，原始合计 3,040,000；40,000+12,000>50,000，剩下的 10,000 不够预留。断言状态改为 `budget_reached`、余额为 0，并断言两种用量（`:3675`）。

改坏版本为什么会失败：第一阶段会用 3,000,100+12,000 判断为 `budget_reached`，直接违反 `idle` 断言；第二阶段同时守住自动用量仍受额度限制。

### 续作第 2 条：历史写入去重

改坏版本：删除 `finish_full_scan` 中的 `periods.dedup()`。

新增测试：`finishing_a_full_scan_deduplicates_existing_history_in_the_raw_file`，`apps/desktop/src-tauri/src/jobs.rs:3994`。

数据 → 推演 → 期望：

1. 固定 N=`2026-10-03T12:00:00.000Z`。原始控制文件写入 `[P1, P1, P2]`；P1 为 N−7200 至 N−6600 秒，P2 为 N−3600 至 N−3000 秒。重复的 P1 完全相同，两个有效时段互不重叠，均在 25 小时内。
2. `begin_full_scan` 在 N−60 秒开始新一轮，`stop_full_scan` 在 N 收尾；追加的新时段为 `[N−60, N]`。所有时段均晚于清理线，不会因过期被删除，也不到 16 项上限。
3. 正常实现按结束时间排序、去重，直接用 `serde_json::from_slice(fs::read(...))` 读取原始文件，断言数组长度恰为 3、内容恰为 `[P1, P2, 新的一轮]`（`:4006`）；没有通过 `read_control` 检查结果。

改坏版本为什么会失败：读入三项后再追加一项，若没有去重，文件会写入 `[P1, P1, P2, 新的一轮]`，长度 4 和完整数组均不符合断言。

### 续作第 3 条：历史写入只保留最新 16 项

改坏版本：把 `finish_full_scan` 的 `periods.drain(..excess)` 改成 `let _ = excess`。

增强测试：`recent_scan_history_prunes_older_than_25_hours_and_keeps_latest_16`，`apps/desktop/src-tauri/src/jobs.rs:4017`；新增原始文件断言从 `:4049` 开始。原有夹具、清理断言、长度及第 5～20 轮的断言全部保留。

数据 → 推演 → 期望：

1. 沿用原测试：先造清理线前一秒结束的旧段、恰在清理线结束的段，再开始并停止一轮；原有断言验证早于 25 小时的旧段被删除，恰在边界的段保留。
2. 再按 index=1～20 依次开始、停止整理，每轮开始 N+index×60 秒，结束再加 10 秒。这些轮次彼此不同且按结束时间递增；最新 16 项应为第 5～20 轮。
3. 原断言读取端仍保留；新增断言直接解析原始 JSON 文件，断言数组长度等于 `MAX_RECENT_FULL_SCANS`（16），且完整内容等于第 5～20 轮的起止时刻（`:4050`）。

改坏版本为什么会失败：写入端不截断时，读取端虽会取最后 16 项，但最终收尾会把它们加上第 20 轮后写成 17 项（第 4～20 轮）；原始文件长度与内容断言会失败。

### 续作第 4 条：待整理计数排除归类任务

改坏版本：`organizing_counts` 的待整理查询去掉 `job_type = 'repo_card'` 限制。

增强测试：`automation_status_splits_missing_outdated_and_deduplicates_awaiting_repos`，`apps/desktop/src-tauri/src/card_handler.rs:2253`；新增夹具在 `:2281`、新增进行中断言在 `:2304`。

数据 → 推演 → 期望：

1. 原 A～G 夹具及所有断言保持不变：有效仓库 a、b、c、d、f、g 共 6；a、g 缺卡，b 旧格式；c、f 现行格式；d 旧格式但已确认；e 已移除。f、g 有理解卡任务排队。
2. 新增 `('c-categorize', 'acct', 'categorize', 'repository', 'c', 'queued')`。c 已有现行理解卡，归类任务不属于整理理解卡；缺卡仍为 2，旧格式仍为 1，合计仍为 3，待整理集合仍是 `{a,b,f,g}`，g 在集合内只算一次。原 `awaiting == 4` 断言直接覆盖新增归类任务处于 queued 时的结果（`:2303`）。
3. 只把 `c-categorize` 改为 running，再调用状态函数，新增断言仍为 4，并说明归类进行中也不增加待整理数（`:2309`）。继续运行原 f 任务改为 running 的断言和 `start_full_scan` 总数为 4 的断言。

改坏版本为什么会失败：去掉类型限制后，queued/running 的 c 会作为第五个仓库并入集合，`awaiting == 4` 的断言得到 5 而失败。

### 续作第 5 条：无法解析的时间不生成免计时段

改坏版本：删除 `scan_period_end` 开头的 `parse_db_time(now).ok()?`。

新增测试：`scan_period_end_rejects_invalid_times_and_caps_valid_times_at_48_hours`，`apps/desktop/src-tauri/src/jobs.rs:4064`。

数据 → 推演 → 期望：

1. 用 `full_scan_control(NOW, 1)` 生成正常开始时间的整理记录，48 小时线为 `2026-10-05T12:00:00.000Z`。取 `now="z-not-a-time"`，先断言它的字典序大于该截止时间；解析应失败，函数应返回 `None`（`:4067`）。
2. 相同记录，正常 now 为开始后 1 小时，应返回 `Some(now)`；开始后 49 小时，应封顶返回 `Some(48 小时线)`（`:4074`）。
3. 用 `full_scan_control("z-not-a-time", 1)` 生成开始时间不可解析的记录，now 使用正常的开始后 1 小时时刻；计算截止时间失败，应返回 `None`（`:4083`）。

改坏版本为什么会失败：跳过 now 的解析后，`"z-not-a-time"` 因字典序晚于截止时间而返回 `Some(48 小时线)`，违反第一个 `None` 断言。

## 改了哪些文件

- `apps/desktop/src-tauri/src/jobs.rs`：仅在测试模块新增 3 个测试，并给现有历史上限测试追加原始 JSON 断言，共增加 102 行。
- `apps/desktop/src-tauri/src/card_handler.rs`：仅在现有拆分计数测试内追加归类任务夹具和进行中断言，共增加 19 行。
- `.card-report.md`：本轮交回报告。
- `spend_guard.rs` 的 git 差异为上一轮遗留，本轮全文保留；`lib.rs` 未修改。没有越界草稿需要还原。

## 验证

- `tools/cards/run.sh rust-fmt-fix` → 退出码 0；随后与入场快照核对，确认只整理了本轮新增测试内容。
- `tools/cards/run.sh rust-fmt` → 退出码 0，格式检查通过。
- `tools/cards/run.sh rust-check` → 退出码 0，`Finished dev profile`，耗时 4.21 秒；6 条未使用代码警告，涉及 `MigrationReport`、`README_RULE_CHARS`、`reserve_at`、`ensure_ready`、`repair_model_json`、`restore_database_file`。本轮没有修改这些实现。
- `python3 -c '<文本快照比对脚本>'` → 退出码 0，3 个文件比对全部通过；两文件测试模块前逐字不变、原有全部行按序保留，第三个文件全文不变。比对对象是在 `$TMPDIR/39-r3-baseline/` 保存的入场快照，本轮差异也保存到同目录 `r3-only.diff`（运行结束会清理，报告不依赖该临时文件）。
- 文本比对脚本首次使用 here-document → zsh 在创建隐式临时文件时被沙盒拒绝，脚本未执行；改用不需要隐式临时文件的 `python3 -c` 后完成上述检查。
- `git diff --check` → 退出码 0，无空白错误。
- `git diff --stat` → 仅 3 个原卡允许的源码文件：`card_handler.rs` 186 行差异、`jobs.rs` 765 行差异、`spend_guard.rs` 120 行差异；这些数字包括上一轮草稿，不能当作本轮修改量。本轮只有 121 行新增测试内容。
- Rust 测试实际执行数：0；本轮未取得测试通过数，也未执行改坏实现验证。交接文件所述上一轮 1360 通过、0 失败、5 忽略为总控此前结果，不是本轮运行结果。

## 没做到的 / 偏离任务卡的地方

- 未执行 `rust-test`：第 3 轮交接说明明确记载该沙盒在 `ort-sys` 重编译后因缺少 ONNX、无网络而无法链接测试，因此本轮按其要求只跑格式和编译检查，没有重试受限测试或下载依赖。
- `rust-check` 不编译 `#[cfg(test)]` 模块，本轮新增测试还没有经过测试目标的类型检查和运行；上面的数据推演是人工核对，不能替代运行证据。
- 未实际运行 5 个改坏版本；预测的失败断言均已逐条写明，需总控在沙盒外验证。未发现需要修改实现的错误。
- 无其他偏离；未改旧断言、旧数字、实现或任务卡外文件。

## 需要总控或 Fox 决定的事

无。

## 给审核者的提示

- 请按本轮入场状态比较增量，不要把相对 HEAD 的上一轮实现差异误认为本轮修改；本轮只在两文件测试模块内插入内容，已有全部行及 `spend_guard.rs` 保留。
- 历史去重和数量上限的新增断言都直接读原始 JSON，不能改回 `read_control`，否则读取端截断会再次掩盖写入错误。
- 队列状态测试的第一个阶段原始用量超过每日额度、每日用量低于额度，第二个阶段自动用量又不足以容纳认领预留，分别验证恢复可用和继续限额。
- `BudgetGate` 原实现已去掉 `Copy`，通过 `full_scan_periods: Vec<FullScanPeriod>` 携带时段；`claim_for_worker` 在 `jobs.rs:885` 构造该字段，认领事务内按时段现算用量。本轮未修改这一设计。
- 没把握的地方仅为未运行测试目标和变异检查；需总控用现有 `tools/cards/run.sh rust-test` 在沙盒外确认新增测试通过、5 个改坏版本分别失败。


---

## 总控审核（2026-10-05，Claude）

> 说明：上面的报告是 Codex **第 3 轮**（只补测试）的交回报告；第 1、2 轮的报告被后一轮覆盖了，它们的内容和结果都写在下面这份审核里。

- **执行者与过程**：Codex（`gpt-6.1-sol`，xhigh），三轮。第 1 轮 13:57–14:14（80 条命令）交出完整实现；总控在沙盒外跑完整 Rust 套件，**1354 通过、2 失败、5 忽略**，两个失败都在 `spend_guard.rs`：① 开始于 49 小时前、还没收尾的一轮，时段一直算到「现在」，把「现在」记的账也当整理用量排除了（**代码问题**，晚收尾的一轮必须封顶在 48 小时线）；② `a_full_scan_far_beyond_its_estimate_no_longer_lifts_the_cap` 的前提在新口径下不成立（整理自己花的钱本来就不占每天额度，**测试问题**）。第 2 轮 14:22–14:41（62 条命令，`续作/39-budget-ledger-r2.md`）修完：新增 `scan_period_end`（`min(现在, 开始 + 48 小时)`，进行中和收尾两处共用，`lastFullScan.finishedAt` 仍写真实收尾时间）、内存里的 `recent_full_scans_recorded` 标记（防止「历史被清空」后又退回宽时段）、测试改成两段、三处 Rust 提示改叫「现在整理」。第 3 轮 15:02–15:11（`续作/39-budget-ledger-r3.md`，只补测试）。
- **沙盒限制**：`ort-sys` 在沙盒里要重新编译、没有网络下载 ONNX Runtime，链接失败，Codex 只能跑 `rust-fmt` / `rust-check`，测试断言靠推演；完整测试由总控在沙盒外跑。第 2 轮后 **1360 通过、0 失败、5 忽略**（那次我跳过了 1 个测试）；第 3 轮后不带任何过滤再跑一遍：**1364 通过、0 失败、5 忽略**。
- **范围**：改动 3 个文件（`jobs.rs`、`card_handler.rs`、`spend_guard.rs`；`lib.rs` 没动），都在任务卡「允许改动」里，`check-scope.sh` 通过。diff 相对 HEAD：+1030 / −41；新增 21 个测试（`jobs` 17、`card_handler` 2、`spend_guard` 2），没有删测试、没有新增 `#[ignore]`。`refusal_for_rerank`、`rerank_*`、`rerank_tokens_used_since`、`REFUSAL_PREFIX`、`is_refusal`、48 小时、刹车线、迁移、提示词、确认门槛一个字没动（逐行对过）。第 3 轮只加测试：我把三个文件的「测试模块之前」部分和第 2 轮的快照逐字比对，**完全一致**。
- **日志审计**：三轮日志里没有碰正式 / 开发数据目录和钥匙串（`com.foxwork` 只出现在读文档的输出里，另有一次只读查看主工作区的 ONNX 编译缓存目录），没有 git 写命令，没有联网。
- **读代码**：我把花钱相关的几处逐行读过——`claim_next` 的认领事务（用量在原 IMMEDIATE 事务里算，`BudgetGate` 去掉 `Copy` 改带 `Vec<FullScanPeriod>`）、`spend_guard::reserve_with`（旧功能通道改用 `jobs::daily_tokens_used`，被拒时不留预留）、`full_scan_periods` / `scan_period_end` / `finish_full_scan` / `control_from_map`（坏项逐项丢弃、倒置区间拒绝、十万项也不慢）、`organizing_counts`（拆分计数与 `awaiting` 去重）。对外接口：`lib.rs` 没动，命令名和参数都不变；返回 `AutomationStatus` 的四条命令（`get_ai_automation` / `set_ai_automation` / `start_full_scan` / `stop_full_scan`）的返回值多五个字段，`QueueStatus` 多一个 `daily_tokens_used`，前端对缺失字段有回退（卡 40）。
- **设计决定（我定的）**：整理这一轮自己花的钱**永远不占每天额度**，包括超过刹车线被自动停下的那一轮——刹车只管「这一轮停下来、不再不限额」，停下以后队列和旧功能回到每天额度，每天额度只算自动用量。理由：Fox 亲手确认过这一轮的花费上限，它已经有自己的预估、刹车线和失败刹车线，不该再让它挤占后台自动整理。
- **「改坏实现」检查**（我自己在草稿目录里写的脚本，不进仓库，逐种改一处实现、跑 `jobs / card_handler / spend_guard` 的测试）：共 41 种。第一次 **33 种被抓住**，8 种没被抓住：
  - **5 处真漏洞**（第 3 轮补测试）：J08 队列状态仍用原始 24 小时合计（这恰好是 Fox 看到「额度用完」的场景，之前没有测试守）、J17 历史去重、J23 写入端截断到 16 项（读取端本来就会截，所以看不出）、C07 `awaiting` 把别的任务类型（`categorize` 等）也算进去、J28 `scan_period_end` 对读不懂的 `now` 的约定。
  - **3 处等价改动**（改了之后任何输入的结果都一样，不补测试）：J15 `parse_db_time` 的 `!is_ascii()`（所有切片边界都落在固定位置的 ASCII 分隔符两侧，非 ASCII 只会让 `parse` 失败）、J26 `append_last_scan_fallback` 里「历史里已有同一轮」（没有历史数组的文件 `recent_full_scans` 一定是空的，判断永远为真）、C09 「没有说明」数里的 `NOT_CONFIRMED_SQL`（没有说明的行 `card_json` 为空，必然为真）。
  - **第 3 轮后复验（15:14–15:16）**：5 种改法重新放回去，**全部被抓住，各由 1 个测试单独报失败**——J08 → `queue_status_uses_daily_usage_after_an_expensive_full_scan`、J17 → `finishing_a_full_scan_deduplicates_existing_history_in_the_raw_file`、J23 → `recent_scan_history_prunes_older_than_25_hours_and_keeps_latest_16`、J28 → `scan_period_end_rejects_invalid_times_and_caps_valid_times_at_48_hours`、C07 → `automation_status_splits_missing_outdated_and_deduplicates_awaiting_repos`；三个文件脚本自己还原后确认 `RESTORE … OK`，工作区只剩那 3 个改动文件。41 种合计：38 种被抓住、3 种等价、0 种漏网。
- **没做到 / 遗留**：① 实际界面和真数据红线不让 Codex 启动 App；我在发布后用真数据复验（每日用量应是 0、额度剩余 60 万、页面「第一步：把现有收藏整理一遍 / 现在整理全部 500 个」、5 个排队的理解卡任务开始认领）。② 额度账本的口径只改了用量计算；`FULL_SCAN_TOKENS_PER_REPO` 之类估算没动。
- **合并后要做的**：和卡 40、41 一起发布（卡 41 的小字「不占每天额度」依赖本卡的口径，单发会说错话）。含 Rust，开发版会重新编译，之后会弹一次钥匙串授权，只有 Fox 能点；发布前确认没有一轮整理正在进行。
