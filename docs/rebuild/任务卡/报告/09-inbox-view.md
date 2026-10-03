# 交回报告 · 卡 09-inbox-view

## 一句话结论
做完了：入库前预览、收集箱列表、改状态、写备注四个只作用于临时库的后端命令已接上，19 个新测试和全套 `rust-test` 都通过。

## 验收逐条
| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| 1. `tools/cards/run.sh rust-test` 通过，`inbox_view.rs` 新增测试至少 16 个，覆盖预览四种状态（含大小写不同的 `github.com/Owner/Repo`、`.git` 结尾、带 `utm_*` 的网页已在收集箱、`http://10.0.0.1/x` 被拦）；预览与 `v2_ops::commit_capture` 判定一致；`counts` 之和等于行数；超长文本报错；没有链接；`list_inbox` 默认不含归档、`all` 含归档、各状态筛选、排序与分页稳定、`counts` 不受筛选影响、非法状态报错、`limit` 夹取；来源字段映射；`set_inbox_status` 合法转换（含归档再恢复）、不许设 `failed`、不存在的 id 报错；`set_inbox_note` 恰好 500 通过、501 报错、首尾空白、不改其他字段 | 是 | `inbox_view.rs` 有 19 个 `#[test]`（802、838、850、872、890、925、939、954、969、994、1016、1044、1054、1080、1110、1120、1150、1174、1213 行）。`tools/cards/run.sh rust-test inbox_view`：19 passed，0 failed。`tools/cards/run.sh rust-test`：296 passed，0 failed，5 ignored，1 filtered out（钥匙串用例由 run.sh 跳过），21.74s。四种状态在 `preview_classifies_the_four_statuses`：`OctoCat/Hello.git` 对上 `octocat/hello` 为 starred，带 `utm_*` 的网页为 inbox，`http://10.0.0.1/x` 为 blocked，其余为 new。同一夹具先预览再 `commit_capture`：`preview_matches_commit_capture_and_does_not_write` 断言 `created/already_starred/already_in_inbox/blocked` 与预览 counts 一致，且预览前后 `inbox_items`、`capture_batches`、`repositories` 行数不变。超长文案是「这段文字太长，请分几次粘贴」。500 个「测」（1500 字节）通过，501 个拒绝且原文不改。 |
| 2. 契约测试：把 `CapturePreviewRow`、`CapturePreview`、`InboxItem`、`InboxPage` 各序列化一份，断言键集合和类型与 `contracts.ts` 一致 | 是 | `contract_serialization_matches_typescript`（`inbox_view.rs:1213`）。键为 `url/normalizedUrl/kind/platform/owner/name/blockedReason/status`，`counts.new/starred/inbox/blocked`，`InboxItem` 的 11 个 camelCase 键（含 `platform`、`sourceContext`、`repositoryId`），`InboxPage` 的 `items/totalCount/counts`。可空字段序列化为 `null`，数字是 JSON number，状态是字符串。 |
| 3. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多（4 条） | 是 | `rust-fmt` 通过。`rust-check` 退出码 0，`generated 4 warnings`，都是原有的：`db/migrations.rs:14` 的 `MigrationReport.applied/backup_path` 未读，`v2_ops.rs:81` `ensure_ready`、`v2_ops.rs:576` `repair_model_json`、`v2_ops.rs:1131` `restore_database_file` 未使用。新文件没有警告。 |
| 4. 给出 4 个命令在 `generate_handler!` 和 `gsat-commands.toml` 里的行号；证明 `capture.rs`、`v2_ops.rs` 没有改动 | 是 | `generate_handler!`：`preview_capture` 6857、`list_inbox` 6858、`set_inbox_status` 6859、`set_inbox_note` 6860。`gsat-commands.toml`：91、92、93、94。命令函数在 `lib.rs` 1072、1081、1092、1102。`mod inbox_view` 在 `lib.rs:10`。`git diff --stat -- apps/desktop/src-tauri/src/capture.rs apps/desktop/src-tauri/src/v2_ops.rs` 无输出。旧的 `extract_links_from_text`、`commit_capture_batch` 函数体未改（`lib.rs:1057` 起仍原样）。 |
| 5. `git diff --stat` 里只出现「允许改动」里的路径 | 是 | `git diff --stat`：`acl-manifests.json`、`gsat-commands.toml`、`lib.rs`。新建的 `apps/desktop/src-tauri/src/inbox_view.rs` 还没进索引，所以不出现在 `git diff --stat`，`git status --short` 里是 `??`。这四个路径都在允许改动里。 |

## 改了哪些文件
- `apps/desktop/src-tauri/src/inbox_view.rs`：新增预览、列表、改状态、写备注，以及 19 个临时库测试。
- `apps/desktop/src-tauri/src/lib.rs`：登记模块和 4 个命令，数据库路径从 `AppStorage` 取。
- `apps/desktop/src-tauri/permissions/gsat-commands.toml`：允许主窗口调用这 4 个命令。
- `apps/desktop/src-tauri/gen/schemas/acl-manifests.json`：同一份允许列表，插在 `commit_capture_batch` 后面（整文件仍是一行 JSON）。

## 验证
- `tools/cards/run.sh rust-test inbox_view` 第一次编译失败：`list_inbox_rejects_invalid_status` 误用了 `unwrap_or_else`，错误类型对不上。改成 `expect_err` 并 `rust-fmt-fix` 后重跑：19 passed，0 failed，2.88s。
- `tools/cards/run.sh rust-test`：296 passed，0 failed，5 ignored，1 filtered out，21.74s，退出码 0。
- `tools/cards/run.sh rust-fmt`：通过。
- `tools/cards/run.sh rust-check`：通过，4 条原有警告，见上表。
- `git diff --name-only` 只有上述三个已跟踪文件；`capture.rs`、`v2_ops.rs` 无差异。

## 没做到的 / 偏离任务卡的地方
- 无功能缺口。下面几处任务卡没写死，按可回退的做法定了，并写进测试：
- 64KiB 按字节计（`text.len()`）。刚好 65536 字节通过，再多 1 字节返回「这段文字太长，请分几次粘贴」。没有链接时不打开数据库。
- 备注先去掉首尾空白，再按 Unicode 字符数（`chars()`）判断。超过 500 返回「备注最多 500 个字」，不截断。只含空白时存空串。
- 列表排序是 `created_at DESC, id ASC`。`totalCount` 是当前筛选下的总数，用来分页；`counts` 是该账号四种状态的总数，不随筛选和分页变化。
- `limit` 缺省 100，夹在 1 到 200；`offset` 缺省 0，负数当成 0。状态字符串区分大小写，不去掉空白。`None` 和空串都不含归档。
- 账号取最近更新的 `github_accounts` 行，SQL 与 `v2_ops` 相同。列表和改写都只动这个账号；别的账号的 id 也返回「找不到这条收集」。
- 有链接但没有账号时，预览、列表、改写都返回「开发库里还没有 GitHub 账号，无法写入收集或分类。」，和 `commit_capture` 同一句。
- 只读命令用 `SQLITE_OPEN_READ_ONLY`，并设 `PRAGMA query_only = ON`。写入走 Immediate 事务，只改声明的列和 `updated_at`（`strftime('%Y-%m-%dT%H:%M:%fZ','now')`）。

## 需要总控或 Fox 决定的事
- 无。

## 给审核者的提示
- 预览判定顺序和 SQL 是照着 `commit_capture` 抄的：先 `blocked_reason`，再 `github_repo` 且 `sync_status = 'active'` 且 `lower(full_name)` 相同，再同一账号的 `normalized_url`，最后才是新收。已收藏压过已在收集箱，被拦压过已在收集箱。一致性测试用的是临时库。
- `id` 并列时用升序。如果界面希望同一秒里后写入的排在前面，要改成 `id DESC`，测试 `list_inbox_sort_and_pagination_are_stable` 会跟着变。
- 全套测试里的 5 个 ignored 是原来就有的；run.sh 另外跳过了钥匙串那一个。
