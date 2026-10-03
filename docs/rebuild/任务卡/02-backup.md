---
编号: 02-backup
允许改动:
  - apps/desktop/src-tauri/src/backup.rs
  - apps/desktop/src-tauri/src/lib.rs
  - apps/desktop/src-tauri/permissions/gsat-commands.toml
  - apps/desktop/src-tauri/gen/schemas/acl-manifests.json
  - apps/desktop/src/pages/v2/toolbox.tsx
---

# 卡 02 · 每日自动备份 + 一键恢复

## Fox 能看到的效果
打开开发版，进「工具箱」页：能看到「自动备份」状态（上次备份时间、下次什么时候备份）和一张备份列表（时间、种类、大小、里面有多少个仓库）；点「现在备份」几秒内多出一行；在某一行点「恢复到这一份…」，确认后提示「需要重启」，重启后资料库就换成了那一份，且恢复前的那个库另存成了一份「恢复前」备份（可以再恢复回去）。App 开着时每天自动备份一次，不用 Fox 做任何事。**这是切换到正式库之前必须有的安全网。**

## 背景与先读什么
- `apps/desktop/src-tauri/src/v2_ops.rs` 里现有的 `backup_database_file`（`VACUUM INTO` 加完整性检查）和 `restore_database_file`（**有缺陷：直接把文件拷到活动库上，不处理 `-wal` / `-shm`，库还开着时会损坏**，本卡要换成安全的两阶段恢复）；`apps/desktop/src-tauri/src/lib.rs` 里的 `create_dev_backup` 命令（第 1105 行附近）和 `run()` 里的 `.setup(...)`（第 6619 行附近）；`apps/desktop/src-tauri/src/db/migrations.rs` 里的 `backup_database`（迁移前备份，看它的文件名和存放位置，列表要认得它）。
- `apps/desktop/src-tauri/src/dev_guard.rs`（正式库保护，**不要改，也不要绕**）；`storage.rs` 里的 `AppStorage::from_app_handle`、`database_path()`。
- 前端：`apps/desktop/src/pages/v2/toolbox.tsx`（现在只有一个「创建备份」按钮，约 55 行）。重启用已安装的 `@tauri-apps/plugin-process` 的 `relaunch()`。
- 可用的库：`rusqlite`（已开 `backup` 特性）、`time`（已开 `formatting`）、`serde`、`serde_json`、`sha2`、`tokio`（只开了 `time`）。**不允许加新依赖**；测试用的临时目录自己建（参考现有测试里的做法）。

## 要做的事

### 1. 新模块 `src-tauri/src/backup.rs`（纯逻辑 + 文件操作，不依赖 Tauri，便于测试）
- `BackupKind`：`Auto`（自动）、`Manual`（手动）、`PreRestore`（恢复前）、`PreMigrate`（迁移前，认得现有迁移备份的文件名）。
- `BackupInfo`：文件名、种类、创建时间（RFC3339 字符串，取文件名里的时间戳，没有就取文件修改时间）、字节数、仓库数（只数 `repositories` 里 `sync_status='active'` 的；读不出就是 `None`）、迁移版本号（`schema_migrations` 里最大的 version，读不出就是 `None`）。
- `create_backup(db_path, backups_dir, kind) -> Result<BackupInfo, String>`：先写临时文件（`.partial`），用 `VACUUM INTO` 或 SQLite 在线备份接口生成，`PRAGMA integrity_check` 必须是 `ok`，再**原子改名**成最终文件名（`auto-YYYYMMDD-HHMMSS.sqlite3`、`manual-…`、`pre-restore-…`）。任何一步失败都不能留下半个文件。同一秒内重复创建要自动避开重名。
- `list_backups(backups_dir) -> Vec<BackupInfo>`：新的在前；认得现有的 `manual-<毫秒时间戳>.sqlite3` 和迁移前备份的命名；忽略 `.partial` 和不认识的文件；单个文件读失败不影响其他行。
- 保留策略 `plan_retention(entries, now) -> Vec<String>`（返回要删除的文件名，纯函数）：自动备份保留最近 14 份，再加最近 3 个月每个月最新的一份；`Manual`、`PreRestore`、`PreMigrate` **永远不自动删除**。
- 到期判断 `auto_backup_due(last_auto: Option<time>, now) -> bool`：距上次自动备份超过 20 小时（或从没有过）就该备份；时间由参数传入，方便测试。
- 恢复分两阶段：
  1. `request_restore(data_dir, backups_dir, file_name, app_max_schema_version) -> Result<(), String>`：只接受**纯文件名**（含 `/`、`\`、`..`、绝对路径一律拒绝）；文件必须在 `backups_dir` 里、完整性是 `ok`、迁移版本号不能大于本 App 认识的最大版本（否则拒绝并说明「这份备份来自更新的版本」）；通过后在 `data_dir` 写标记文件 `pending-restore.json`（记录文件名和请求时间）。**这一步不动活动库。**
  2. `apply_pending_restore(data_dir, backups_dir) -> Option<RestoreReport>`：App 启动时、**打开任何数据库连接之前**调用。有标记才做事：先把当前库（连同 `-wal`、`-shm` 里的内容，用文件拷贝就行，此时没有连接）另存为 `pre-restore-…` 备份；删除活动库的 `-wal` 和 `-shm`；把备份拷到临时文件再原子改名成活动库；删除标记；返回报告（恢复到哪一份、恢复前备份的文件名）。任何一步失败：**活动库保持原样**，删除标记，把失败原因写进 `restore-result.json`，返回带错误的报告。没有标记就什么都不做，返回 `None`。
- 状态 `backup_status(...)`：上次自动备份时间、下次到期时间、上次恢复结果（读 `restore-result.json`）。

### 2. 接入应用（`lib.rs`）
- `.setup` 里：**已核实**——`AppStorage::from_app_handle` 会 `migrate()`（即打开并升级数据库），`start_embedding_maintenance` 也可能开库，而 `enforce_dev_data_isolation` 只解析数据目录、不开库。所以 `apply_pending_restore` 要插在 `enforce_dev_data_isolation` **之后**、`start_embedding_maintenance` 和 `AppStorage::from_app_handle` **之前**；数据目录用 `app.path().app_data_dir()`，并照 `AppStorage::database_path_from_app_handle` 那样先过一遍 `dev_guard::refuse_if_production_data_dir`（正式库目录必须拒绝）。恢复完成后再让后面的代码正常开库、升级。然后开一个后台线程：启动 1 分钟后检查一次到期就备份，之后每 6 小时检查一次；备份失败只记日志，不弹窗、不崩溃、不影响应用。备份目录用现有的 `backups/`（在 `AppStorage` 的数据目录下）。
- 新增 4 个命令并登记到 `generate_handler!` 和 `permissions/gsat-commands.toml`：`list_backups`、`create_backup_now`（种类 Manual）、`request_restore_backup(file_name)`、`get_backup_status`。命令里的路径一律由后端从 `AppStorage` 取，**不接受前端传来的路径**。
- 旧的 `create_dev_backup` 命令保持不动（别处可能在用），但工具箱页改用 `create_backup_now`。

### 3. 前端（`pages/v2/toolbox.tsx`，只改备份相关部分）
- 「自动备份」状态行：上次时间（没有就写「还没有」）、下次到期时间。
- 备份列表：时间（本地时间，到分钟）、种类（自动/手动/恢复前/迁移前）、大小（MB，一位小数）、仓库数；每行一个「恢复到这一份…」。
- 「现在备份」按钮（页面里**唯一**的主按钮样式；其他按钮用次要样式），成功后刷新列表并提示「已备份」。
- 点「恢复到这一份…」弹确认：「恢复会把当前资料库换成这一份。当前的库会先另存一份「恢复前」备份。需要重启应用才会生效。继续吗？」；确认后调用 `request_restore_backup`，成功则显示「已记录，重启后生效」并给一个「立即重启」按钮（调用 `relaunch()`）和一个「稍后」。
- 重启后若 `get_backup_status` 里有上次恢复结果，在页面上方显示一行：成功「已恢复到 <时间> 的备份」，失败「恢复失败：<原因>（资料库没有被改动）」。
- 所有界面文字用简体中文；错误显示后端返回的中文原因，不要吞掉。

## 不要做
- 不动迁移（`db/migrations.rs`）、不动 `dev_guard.rs`、不动 `v2_ops.rs`；不新增依赖；不改其他页面。
- 不要在 `apply_pending_restore` 之前的任何地方打开数据库；不要在库开着时替换库文件。
- 不要读写真实的数据目录：所有测试只用临时目录里造出来的 SQLite 文件。
- 不要做云备份、加密、定时器 UI 等没写的东西。

## 验收
1. `tools/cards/run.sh rust-test` 通过，新增测试**至少 14 个**，覆盖：创建备份通过完整性检查且不留 `.partial`；人为让备份中途失败后不留半个文件；同一秒重复创建不覆盖；保留策略的表驱动用例（恰好 14 份、超过 14 份、跨月每月留最新一份、手动/恢复前/迁移前永不删除、空列表）；`auto_backup_due` 的边界（19 小时 59 分 = 否，20 小时 = 是，从没有 = 是）；`list_backups` 排序与种类识别（含现有 `manual-<毫秒>` 命名、忽略 `.partial` 和未知文件）；`request_restore` 拒绝路径穿越名（`../x`、`a/b`、绝对路径）、拒绝损坏文件、拒绝版本更高的备份、接受正常备份；`apply_pending_restore` 成功时活动库内容变成备份内容、`-wal`/`-shm` 被删、生成了 `pre-restore-…` 且内容是恢复前的库；备份损坏时活动库字节不变且 `restore-result.json` 有失败原因；没有标记时返回 `None` 且什么都没动。
2. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，且 `rust-check` 的警告数不比卡开始前多（卡开始前是 4 条，都在旧代码里）。
3. 给出 `lib.rs` 里 `apply_pending_restore` 调用的行号，并说明它在 `start_embedding_maintenance` 和 `AppStorage::from_app_handle` 之前、在 `enforce_dev_data_isolation` 之后；说明为什么前面的行不会开库。
4. 给出 4 个命令在 `generate_handler!` 和 `gsat-commands.toml` 里登记的行号；`backup.rs` 里没有任何接受前端传入路径的公开函数（给出证明：公开函数签名清单）。
5. `tools/cards/run.sh web-build` 通过；给出 `toolbox.tsx` 里「现在备份」「恢复到这一份…」「立即重启」「自动备份状态行」「上次恢复结果」各自的行号。
6. `git diff --stat` 里只出现「允许改动」里的路径。

## 停止条件
- 需要改卡外的文件、需要新依赖、发现除 `start_embedding_maintenance` 和 `AppStorage::from_app_handle` 之外还有代码会在恢复前打开数据库、或者拿不准某个文件名格式——停手并写进报告。
