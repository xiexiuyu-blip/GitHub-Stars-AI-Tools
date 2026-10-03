---
编号: 09-inbox-view
允许改动:
  - apps/desktop/src-tauri/src/inbox_view.rs
  - apps/desktop/src-tauri/src/lib.rs
  - apps/desktop/src-tauri/permissions/gsat-commands.toml
  - apps/desktop/src-tauri/gen/schemas/acl-manifests.json
---

# 卡 09 · 收集：入库前预览状态、收集箱列表（后端）

## Fox 能看到的效果
这是后台积木，**这张卡 Fox 暂时看不到界面变化**。做完后，「收集」页粘贴一段群聊，**入库之前**就能看到每条链接会怎么处理——**新收 / 已在你的 Stars 里 / 已在收集箱 / 将被拦下（内网地址等）**；入库后收集箱里能看到所有条目、按状态筛选、写备注、归档。这就是 Fox 要的「微信联动」要落地的那个收集箱。

## 背景与先读什么
- **契约（不许改名、不许改形状）**：`apps/desktop/src/lib/contracts.ts` 里的 `CapturePreviewRow`、`CapturePreview`、`InboxItem`、`InboxPage`、`SetInboxStatusRequest`。命令返回的 JSON 必须逐字段按它走（serde 的 `camelCase`）。
- **链接抽取**：`apps/desktop/src-tauri/src/capture.rs` 的 `pub fn extract_links_from_text(text) -> Vec<ExtractedLink>`（字段全是 `pub`：`url`、`normalized_url`、`kind`、`platform`、`owner`、`name`、`blocked_reason`），已经去重、已经带「被拦原因」。**直接调用它，不要改它。**
- **现有的入库**：`apps/desktop/src-tauri/src/v2_ops.rs` 的 `commit_capture`（约第 621 行起）和 `CaptureCommit`——它在一个事务里决定每条链接是「已收藏 / 已在收集箱 / 被拦 / 新收」，规则是：`kind` 是 `github_repo` 且在 `repositories` 里（有效、`lower(full_name)` 相等、同账号）→ 已收藏；`inbox_items` 里已有同账号同 `normalized_url` → 已在收集箱；`blocked_reason` 非空 → 被拦；否则新收。**预览必须用和它完全相同的判定顺序和条件**（先看被拦，再看已收藏，再看收集箱，最后是新收——读 `commit_capture` 的实际顺序，照它来；顺序不一致会让预览和真实入库对不上）。**不要改 `commit_capture`。**
- **表**：`inbox_items`（迁移 005 + 021：`id`、`account_id`、`original_url`、`normalized_url`、`title`、`item_type`、`user_note`、`status`（`pending`/`ready`/`failed`/`archived`）、`repository_id`、`created_at`、`updated_at`，以及 `source_platform`、`source_context`、`captured_via`、`capture_batch_id`、`raw_excerpt`）；`repositories`；账号取法：库里最近更新的 `github_accounts` 行。
- 测试怎么造库：看 `db/migrations.rs` 里 `empty_database_applies_every_migration`，再用 SQL 插 `github_accounts`、`repositories`、`inbox_items`。**所有测试只用临时库。**

## 要做的事

### 新模块 `src-tauri/src/inbox_view.rs`（不依赖 Tauri）
1. **`preview_capture(db_path, text) -> CapturePreview`**：`text` 超过 64KB 返回中文错误「这段文字太长，请分几次粘贴」；用 `extract_links_from_text` 抽链接；对每条链接按上面的判定得到 `status`（`blocked`/`starred`/`inbox`/`new`）；`rows` 保持抽取顺序；`counts` 四项之和等于 `rows` 的长度。**只读，不写库。** 没有链接返回空的 `rows` 和全 0 的 `counts`，不报错。
2. **`list_inbox(db_path, status: Option<&str>, limit, offset) -> InboxPage`**：`status` 为 `None` 或空串时返回**除 `archived` 外**的全部（待处理、已整理、失败），为 `"all"` 时包含归档，为某个具体状态时只返回该状态；排序 `created_at` 新到旧，并列按 `id`；`limit` 默认 100、夹在 1 到 200；字段按契约：`platform` 取 `source_platform`，`sourceContext` 取 `source_context`；`counts` 是该账号下四种状态各自的总数（不受 `status` 筛选影响）。`status` 不是合法值返回中文错误。
3. **`set_inbox_status(db_path, id, status) -> Result<(), String>`**：`status` 只能是 `pending`、`ready`、`archived`（`failed` 是系统写的，用户不能设）；只改 `status` 和 `updated_at`；`id` 不存在返回中文错误「找不到这条收集」。
4. **`set_inbox_note(db_path, id, note) -> Result<(), String>`**：备注最多 500 个字符（按字符计，超出返回中文错误，**不要悄悄截断**）；去掉首尾空白；只改 `user_note` 和 `updated_at`。

### 接入（`lib.rs`）
- 登记 `mod inbox_view;`；新增 4 个命令并登记到 `generate_handler!` 和 `permissions/gsat-commands.toml`：`preview_capture(text)`、`list_inbox(status, limit, offset)`、`set_inbox_status(id, status)`、`set_inbox_note(id, note)`。参数名按契约。数据库路径一律从 `AppStorage` 取；只读命令用只读方式打开。**旧的 `extract_links_from_text`、`commit_capture_batch` 命令保持不动。**

## 不要做
- 不改迁移、不改 `capture.rs`、不改 `v2_ops.rs`、不改前端、不新增依赖；不联网（取 X 帖子正文是另一张卡的事）。
- 不要读写真实的数据目录；测试只用临时库。

## 验收
1. `tools/cards/run.sh rust-test` 通过，`inbox_view.rs` 新增测试**至少 16 个**，覆盖：预览的四种状态各一个例子（含 `github.com/Owner/Repo` 大小写不同也算已收藏、`.git` 结尾、带 `utm_*` 的网页链接已在收集箱、`http://10.0.0.1/x` 被拦）；预览与 `v2_ops::commit_capture` **判定一致**（用同一份夹具文本和同一个库，先预览，再真的入库，断言入库后 `CaptureCommit` 的 `created`/`already_starred`/`already_in_inbox`/`blocked` 数量和预览的 `counts` 对得上——**入库放在测试里的临时库，不要影响别的测试**）；`counts` 之和等于行数；超长文本报错；没有链接；`list_inbox` 的默认不含归档、`all` 含归档、各状态筛选、排序与分页稳定、`counts` 不受筛选影响、非法状态报错、`limit` 夹取；来源字段映射；`set_inbox_status` 的合法转换（含归档再恢复）、不许设 `failed`、不存在的 id 报错；`set_inbox_note` 的 500 字上限（恰好 500 通过、501 报错）、首尾空白处理、不改其他字段。
2. **契约测试**：把 `CapturePreviewRow`、`CapturePreview`、`InboxItem`、`InboxPage` 各序列化一份，断言键集合和类型与 `contracts.ts` 一致。
3. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多（4 条）。
4. 给出 4 个命令在 `generate_handler!` 和 `gsat-commands.toml` 里的行号；证明 `capture.rs`、`v2_ops.rs` 没有改动。
5. `git diff --stat` 里只出现「允许改动」里的路径。

## 停止条件
- 要改迁移或卡外文件、要新依赖、`commit_capture` 的判定和本卡描述有冲突——停手并写进报告，不要自己拍板。
