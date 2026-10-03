# 交回报告 · 卡 07-library-view

## 一句话结论
做完了：资料库列表、详情、批量状态和今日视图四个只读/写入查询已接上，26 个新测试和整套 Rust 测试通过，3000 条默认查询实测 97 毫秒。

## 验收逐条
| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| 1. `tools/cards/run.sh rust-test` 通过，`library_view.rs` 新增测试**至少 24 个**，覆盖：状态组映射的全部 8 个原始值与「没有 annotations 行」；各个筛选单独和组合（场景、状态组、卡片状态、形态、关键词含 `%` 和 `_` 的转义、大小写不敏感）；`cardState` 默认是 `done`；三种排序和并列稳定、分页没有重复也没有遗漏；`oneLiner` 不回退到摘要；**坏 JSON、空串、字段类型不对的 `card_json` 不会让查询失败**，且这些行 `hasCard` 的判定正确；`LibraryCounts` 的联动语义（选了场景后，`scenarios` 计数不变而 `reading` 计数随之变；选了状态后反过来）、`total` 不受筛选影响、没有归类的场景也有键且为 0；`scenarioIds` 的排序和 `rejected` 不计入；详情的各字段回退（`how_to_try` 是数组时的处理、`capabilities` 不是数组时为空数组）和 `readmeExcerpt` 的截断（用全中文长文本验证不切坏字符）；`set_library_reading` 的映射、不动 `note_md`/`rating`、不存在的 id 和已移除的仓库被忽略、上限 200；`get_today_view` 的挑选（核心方向优先、同方向最多一个、不够再放宽、只含 unseen 且有卡的）、`weekNew` 的 7 天边界和 8 个上限、各个 `counts`、`syncedAt`。 | 是 | `tools/cards/run.sh rust-test`：303 passed，0 failed，5 ignored，1 filtered（run.sh 跳过的钥匙串用例）。其中 `library_view` 26 个测试全部 ok。函数在 `library_view.rs`：状态组 1425、默认 done 1489、一句话不回退 1525、关键词各列与大小写 1549、`%`/`_` 1596、场景跳过 rejected 1621、状态组筛选 1649、形态 1688、组合 AND 1713、三种排序与分页 1764、limit/offset 1822、坏 JSON 1864、计数联动 1931、total 与空场景 2024、scenarioIds 顺序 2060、详情回退 2085、README 1500 字 2195、找不到仓库 2220、写入映射且不动笔记 2227、忽略无效 id 与 200 上限 2278、今日不同方向 2348、不够再放宽 2404、7 天边界与 8 条 2431、今日计数与 syncedAt 2475。 |
| 2. **契约测试**：把 `LibraryItem`、`LibraryCounts`、`LibraryPage`、`LibraryDetail`、`TodayView` 各序列化一份，断言键集合和类型和 `contracts.ts` 一致（键名、`null` 与数组的位置都要对）。 | 是 | `library_view::tests::contract_json_keys_and_types_match_typescript` 通过，`library_view.rs:2539`。`reading` 的键是 `in_use`，序列化结果不含 `inUse`。 |
| 3. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多（4 条）。 | 是 | `rust-fmt` 退出码 0。`rust-check`：`generated 4 warnings`，都是原有死代码：`migrations.rs:14` 的 `applied`/`backup_path`，`v2_ops.rs:81` `ensure_ready`，`v2_ops.rs:576` `repair_model_json`，`v2_ops.rs:1131` `restore_database_file`。新代码没有新增警告。 |
| 4. 给出 4 个命令在 `generate_handler!` 和 `gsat-commands.toml` 里的行号；证明 `v2_ops.rs` 没有改动。 | 是 | `generate_handler!`：`list_library_view` 6879、`get_library_detail` 6880、`set_library_reading` 6881、`get_today_view` 6882。命令函数：920、929、938、948。`gsat-commands.toml`：110、111、112、113。`git diff --stat -- apps/desktop/src-tauri/src/v2_ops.rs` 无输出。 |
| 5. 性能：用脚本在测试里造 3000 个仓库的临时库，`list_library_view` 的一次默认查询（含计数）在本机应当不超过 1 秒；把实测耗时写进报告。超过就优化查询（比如把 JSON 解析放到只对当前页的行做）。 | 是 | `default_query_on_3000_repos_stays_under_one_second` 通过。测试把毫秒数写到 `/tmp/fsl-07-library-view-bench.txt`，全套测试那次的值是 **97**。断言要求小于 1 秒。默认页 80 条，`limit=10000` 被夹到 200，`totalCount` 仍是 3000。 |
| 6. `git diff --stat` 里只出现「允许改动」里的路径。 | 是 | `git diff --stat` 只有 `acl-manifests.json`、`gsat-commands.toml`、`lib.rs`。新文件 `apps/desktop/src-tauri/src/library_view.rs` 还未跟踪，`git status` 显示为 `??`，路径在允许改动里。没有其他路径。 |

## 改了哪些文件
- `apps/desktop/src-tauri/src/library_view.rs`：新增查询模块和 26 个测试。不依赖 Tauri。列表、详情、今日用只读打开；批量状态一次事务写入。
- `apps/desktop/src-tauri/src/lib.rs`：`mod library_view;`，登记四个命令。数据库路径来自 `AppStorage`。`get_today_view()` 不收时间参数，自己取当前 UTC 再传进模块。
- `apps/desktop/src-tauri/permissions/gsat-commands.toml`：允许这四个命令。
- `apps/desktop/src-tauri/gen/schemas/acl-manifests.json`：在 `allow-gsat-commands` 的 allow 列表末尾加上同名四条，否则主窗口调不到。

## 验证
- `tools/cards/run.sh rust-fmt-fix` → 只改了本卡文件的格式，随后 `rust-fmt` 通过。
- `tools/cards/run.sh rust-test library_view` → 26 passed。
- `tools/cards/run.sh rust-test` → 303 passed，0 failed，5 ignored，1 filtered，16.84s。
- `tools/cards/run.sh rust-check` → 通过，4 条原有警告。
- `git diff --stat` → 见验收 6。`v2_ops.rs` 无差异。
- 性能夹具写在临时库，测完删除；没有读正式库或开发库。没有启动 App。

## 没做到的 / 偏离任务卡的地方
- 无。这张卡没有界面，按红线没有启动 App，也没有改契约。

## 需要总控或 Fox 决定的事
- 无。

## 给审核者的提示
- 模块函数是 `get_today_view(path, now)`。`now` 给测试钉住时间。Tauri 命令 `get_today_view()` 无参数，用 `OffsetDateTime::now_utc()` 格式化成 RFC 3339 再传入。近 7 天在 SQL 里用 `julianday(starred_at) >= julianday(now) - 7`，整 7 天那一秒算在内，再早一秒不算。`time` 只有 `formatting`，没有加解析功能。
- `how_to_try` 是字符串就原样用；是数组时把非空字符串用换行接起来，其他类型当没有。测试断言了 `先安装\n再运行`。
- `set_library_reading` 返回的是这次实际写入的有效仓库个数（去重后）。状态本来就是目标值时也会改 `updated_at`，仍算 1。超过 200 个（按调用方给的原始长度，含重复）直接报错，不写任何一行。已有标注只改 `read_status` 和 `updated_at`。
- 列表和批量写入只处理 `sync_status='active'`。按 id 取详情时，行在就返回，包括已移除；找不到才是「找不到这个仓库」。`syncedAt` 是全部仓库 `updated_at` 的最大值，含已移除。测试把已移除行的时间做成了最大值。
- 方向排名用 `sort_order <= 7` 的最小排序，不是写死 slug。任务没说跳过已归档场景，所以未拒绝的归档场景仍会计入 `scenarioIds` 和方向排名。被拒绝的归类两边都不计。没有核心方向的排名是 99，先每个排名最多一个，不够 3 个再按原排序补，最后按原排序展示。
- `forms` 只放数量大于 0 的形态。未归档场景、五个状态组、`done`/`pending` 都会给出键，没有就是 0。
- `oneLiner` 只看 `one_liner_zh`，空白只去掉普通空格，和 SQLite `TRIM` 一致。只有空白或没有文档时 `hasCard` 为假，`cardReview` 为 null；`form` 和 `confidence` 仍从读得懂的 `card_json` 取。`cardsSuggested` 把「有一句话，且 `review_status` 不是字符串 `confirmed`」都算上，坏 JSON 也算。
- `json_extract` 都放在 `json_valid` 为真的分支里。关键词转义 `\`、`%`、`_`，用 `ESCAPE '\'`，不预先转小写。
- 只读连接是 `SQLITE_OPEN_READ_ONLY` 加 5 秒 `busy_timeout`。测试在交回连接前做了 `wal_checkpoint(TRUNCATE)`，避免只读打开缺 `-shm`。

## 第 2 轮修改

总控只要求补 `LibraryItem.highlight`。只改了 `apps/desktop/src-tauri/src/library_view.rs`，没有动 `lib.rs`、权限文件和契约。

### 意见怎么处理的
- `LibraryItem` 增加 `highlight: Option<String>`（`library_view.rs:114`）。没有 `skip_serializing_if`，`None` 序列化成 `"highlight": null`。
- 在 `into_item` 里赋值（`library_view.rs:760`），计算函数是 `highlight_of`（`library_view.rs:996`）。顺序：`problems_solved` 里第一个去掉普通空格后仍非空的字符串；没有就用 `capabilities_of` 的第一条（字符串或对象的 `name_zh`）；再没有、没有卡、坏 JSON、字段不是数组，都是 `null`。不和 `oneLiner` 比较。
- 列表、详情、今日的 `picks` / `weekNew` 都走 `into_item`。`LibraryDetail` 的 `flatten` 会带上这个字段。

### 新增测试（5 个）
- `highlight_prefers_first_problem`（2563）：有能力名时仍取第一条问题；和一句话相同也照样返回。详情同一条。
- `highlight_uses_capability_string_when_problems_are_missing`（2584）：没有问题时取第一条字符串能力，并去掉首尾空格。
- `highlight_uses_capability_name_when_problems_are_missing`（2598）：空的 `name_zh` 跳过，取下一条能力名。
- `highlight_skips_blank_or_non_string_problems`（2615）：空串、数字、纯空格都跳过，取后面的问题，不用能力名。
- `highlight_is_null_without_a_usable_card`（2632）：没有卡、坏 JSON、字段类型不对、元素全空，都是 `null`。
- `contract_json_keys_and_types_match_typescript` 的键集合已加上 `highlight`，并断言 `null` 时键仍在。`LibraryItem` 和 `LibraryDetail` 都查了。旧断言没有改含义。

### 验证
- `tools/cards/run.sh rust-fmt` → 通过。
- `tools/cards/run.sh rust-check` → 通过，仍是 4 条原有警告（`migrations.rs:14`，`v2_ops.rs:81`、`576`、`1131`）。
- `tools/cards/run.sh rust-test library_view` → 31 passed（原 26 个加上这 5 个）。
- `tools/cards/run.sh rust-test` → 415 passed，0 failed，5 ignored，1 filtered（钥匙串用例），26.02s。
- `git diff --stat` 本轮只有 `apps/desktop/src-tauri/src/library_view.rs`（+124）。
