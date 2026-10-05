---
编号: 35-followup-hardening
允许改动:
  - apps/desktop/src-tauri/src/lib.rs
  - apps/desktop/src-tauri/src/readme_follow.rs
  - apps/desktop/src-tauri/src/card_pipeline.rs
  - apps/desktop/src-tauri/src/storage.rs
---

# 卡 35 · README 跟进与理解卡 v3：只读审查员 B 找到的问题（Rust）

## 为什么有这张卡
只读审查员 B（2026-10-05）读了本轮新代码（README 跟进、场景 v2、理解卡 v3），在数据安全和解析健壮性上找到 4 处必须修和 1 处值得修的问题。本卡只改这几处，**别的一概不动**。

## Fox 能看到的效果
- 同步时新收藏**一定会排队、做规则归类**，哪怕同步到一半关了应用（现在的顺序会让它们永远排不上队）。
- 同一个仓库的理解卡重做总失败 / 被取消后，**不会每次同步都再排一遍**白花额度。
- 同步 / 手动抓 README 时，别的写库操作不再被一份奇怪的超长 README 卡住几分钟（「database is locked」）。
- 理解卡里的场景标签依据（原话）不再能用「的」「a」这种一两个字糊弄过去。
- 模型漏写一句话 / 说明时，不会写进一张空的卡（走现有的「再修一次」，修不好就失败）。

## 背景与先读什么
- `apps/desktop/src-tauri/src/lib.rs` 约 4255–4335 行（同步主流程）、约 3960–4180 行（`refresh_stale_readmes`、`finish_readme_follow`、`ReadmeRefresh`、`README_FETCH_CONCURRENCY`）：现在的顺序是 ① 发出「同步完成」→ ② `refresh_stale_readmes`（最多 150 个 README，分批联网，**没有总时限**）→ ③ `card_handler::enqueue_new_stars(created_ids)` → ④ `scenarios::classify_new_repositories(created_ids)` → ⑤ `finish_readme_follow`。`created_ids` 只在内存里。
- `apps/desktop/src-tauri/src/readme_follow.rs`：`refresh_targets`、`enqueue_changed`、`insert_job`（约 285 行，只防 `queued` / `running` 重复）、约 217–237 行（在 `IMMEDIATE` 写事务里调 `build_readme_excerpt`）。
- `apps/desktop/src-tauri/src/card_pipeline.rs`：原话核对（约 861 行一带，`scenario_evidence` 的核对）、`keyword_support`、`truncate_text`（约 2535 行）、`required_card_str`。
- 测试夹具：各文件里已有的临时库写法；README 刷新的网络部分不要在测试里联网（把「抓取」做成可注入的闭包 / trait，或只测纯函数部分）。

## 要做的事

### 1. 同步顺序：新收藏先排队，老 README 的批量刷新放最后并设上限（`lib.rs`）
目标顺序：
1. 「同步完成」事件照旧最先发（界面不要等）。
2. **先只抓 `created_ids` 里那些还没有 README 的新仓库的 README**（数量小；这样它们的理解卡能带着 README 生成，不会先出一张「只有简介」的卡、README 抓到后又因为「以前没有 README、现在有了」被再排一次白花钱）。
3. `card_handler::enqueue_new_stars(created_ids)`、`scenarios::classify_new_repositories(created_ids)`（失败只记日志，同现在）。
4. **再**刷新其余需要更新的老 README（`readme_follow::refresh_targets` 里除去第 2 步已抓的），这一步加两个保护：**总时限 120 秒**（超过就停，剩下的留给下一次同步，`MAX_REFRESH_PER_RUN` 的 150 上限保持）、**连续 5 个抓取失败就停**（网络断了 / 被限流时别把 150 个全试一遍）。
5. `finish_readme_follow` 照旧最后做（对比哈希、给明显改动的排队、记 `lastReadmeCheck`）。
- 第 2、4 步复用现有的抓取与进度事件代码，不要复制一份：把 `refresh_stale_readmes` 改成接收「要抓哪些」和一个截止时间 / 失败上限的参数，调用两次。
- 要抓的和计数（`ReadmeRefresh.refreshed`）两次合并后给 `finish_readme_follow`。

### 2. 反复失败 / 被取消的仓库不再每次都排队（`readme_follow.rs`）
`insert_job` 现在只防 `queued` / `running`。再加：**这个仓库最近 24 小时内有过 `failed` 或 `cancelled` 的理解卡任务（同一个 `job_type` / `target_id`）就不再排**，并在 `FollowReport` 里多一个计数（例如 `held_back`）、`lastReadmeCheck` 的文案不用改。24 小时的算法沿用 `jobs::` 里已有的时间工具，不要自己拼字符串比较。

### 3. 写事务外算摘录（`readme_follow.rs`）
`build_readme_excerpt` 现在在 `IMMEDIATE` 写事务里跑，里面的清洗函数对超长单行 + 未闭合标记（`![`、`[](`、`&lt;span`）是 O(行长 × 次数)，1 MB 的一行能跑分钟级，期间一直占着写锁。改成：
- 先在**事务外**读出 README、截到前 **200 KB**（按字符边界）、算出摘录和哈希；
- 开 `IMMEDIATE` 事务后只**复核**这个仓库的 `content_hash` 还是刚才读到的那个（变了就跳过这一个，下次再说），再写；
- 事务里不许再调用任何按文本长度成比例变慢的函数。

### 4. 场景依据的最小长度（`card_pipeline.rs`）
`scenario_evidence` 的原话核对现在没有最小长度，写「的」「a」也能通过；没有 `evidence` 词的场景（图片、视频、PPT、写作、调研、数据、知识库、电商这几类）等于没核。改成：**原话去掉空白后不足 4 个字符（按字符数）不算依据**，这时退到现有的 `keyword_support`（场景关键词字面出现）判断；两者都不满足就去掉这个标签（现有行为）。不要改 `keyword_support` 本身。

### 5. 必填字段不许是空的；截断不许把非空文字截成空（`card_pipeline.rs`）
- `required_card_str`（以及同类的必填字符串检查，如 `one_liner_zh`、`what_it_is`）现在不查非空：模型回 `""` 或只有空白会写出一张空的卡。改成：**trim 后为空算「缺字段」**，走现有的校验失败 → 修复调用路径。
- `truncate_text` 在补括号 / 找句末断点后，若输入非空而结果为空（例如以「「」开头的超限句），**退回按字符硬截到上限（再补括号）**，保证返回非空。

## 不要做
- 不改额度、`spend_guard.rs`、`ai.rs`、`solve*.rs`、`scenarios.rs`、迁移、前端、权限文件。
- 不新增依赖；不加新的 Tauri 命令；不改 `lastReadmeCheck` 的 JSON 键名（只许往里加可选键，旧文件仍能读）。
- 不改变「已确认的卡不碰」「小改动只更新哈希」「明显改动才排队」这些既有规则。

## 验收
1. `tools/cards/run.sh rust-test` 全部通过；新增 Rust 测试**至少 18 个**，覆盖：
   - 同步顺序（可注入的抓取闭包 + 临时库 + 假的 `created_ids`）：新仓库的 README 在入队**之前**被抓；入队与归类发生在老 README 刷新**之前**（用调用顺序记录断言）；老 README 刷新超过总时限就停（注入时钟）、连续 5 个失败就停、成功后计数清零；中途 panic / 提前返回时，新仓库已经排了队（把「刷新老 README」那一步做成会失败的闭包验证）。
   - 反复失败：最近 24 小时有 `failed` / `cancelled` → 不排、`held_back` 加 1；25 小时前的 → 照排；`queued` / `running` 仍不重复排；别的仓库的失败不影响这个仓库。
   - 事务外算摘录：超长（1 MB）单行 + 大量未闭合标记的 README，`enqueue_changed` 在合理时间内完成（用 `Instant` 断言 < 3 秒，且摘录函数不在事务闭包里被调用——用结构上的断言或注入计数）；复核发现 `content_hash` 在中间变了 → 这个仓库被跳过、不写。
   - 最小长度：「的」「a」「ok」+ 无关键词 → 去掉标签；4 个字符以上且字面在材料里 → 保留；3 个字符的原话但场景关键词字面出现 → 靠 `keyword_support` 保留；全角空格 / 换行在长度计算时被去掉。
   - 必填非空：`one_liner_zh` / `what_it_is` 为 `""`、`"  "`、缺失 → 校验失败（走修复路径，修不好就失败）；`truncate_text("「" + 80 个字 …)` 这类输入返回非空。
2. 现有测试一个断言都不许放宽或删除；`tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多。
3. `git diff --stat` 里只出现「允许改动」里的文件。
4. 报告里写：每一条对应的代码位置（文件:行）；同步新顺序的调用图（五步各在哪一行）；新增常量及取值；没把握的地方。

## 停止条件
- 需要改额度闸门、迁移、前端，或需要新依赖——停手并写进报告。
- 发现必须改「同步完成」事件的时机才能做到——停手并写进报告（不要改）。
