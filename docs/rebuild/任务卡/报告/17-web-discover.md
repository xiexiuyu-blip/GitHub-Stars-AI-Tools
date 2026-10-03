# 交回报告 · 卡 17-web-discover

## 一句话结论
做完了：发现和社区的联网层、031 缓存表、三个命令都已接上，测试用临时库和假联网器，没有打真实网络。

## 验收逐条
| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| 1. `tools/cards/run.sh rust-test` 全部通过，新增测试至少 28 个，覆盖迁移、url_encode、rate_limit_wait_secs、curl 参数拼装、GitHub 搜索解析、`{since}`、refresh 的成功/部分失败/全部失败/限流/场景不存在、list 的默认选中/`mine`/`inInbox`/`candidateCount`/坏 topics、社区的 HN/Issues 解析、refresh=false 不联网、stale、单源失败保留旧数据、排序与 fetchedAt、以及六个返回值的契约键（含 null） | 是 | `tools/cards/run.sh rust-test`：527 passed，0 failed，5 ignored，1 filtered（跳过的是钥匙串用例）。新增 48 个：`web_fetch.rs` 8、`discover.rs` 24、`community.rs` 15、`db/migrations.rs` 的 `web_content_tables_exist_and_second_migrate_is_idempotent` 1。覆盖见下方测试名。 |
| 2. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多（4 条） | 是 | `rust-fmt` 退出码 0。`rust-check` 退出码 0，`generated 4 warnings`，都是原有的：`MigrationReport.applied/backup_path`、`v2_ops::ensure_ready`、`repair_model_json`、`restore_database_file`。本次没有新增警告。 |
| 3. 三个命令在 `generate_handler!` 和 `gsat-commands.toml` 的行号；`CurlFetcher` 参数拼装位置；`refresh_*` 为 async 且阻塞调用在 `spawn_blocking`；`grep -n "keychain\|Authorization\|token"` 对三个新 rs 文件应为空或只有注释 | 是 | `lib.rs:7114-7116` 登记 `list_discover` / `refresh_discover` / `get_repo_community`。`gsat-commands.toml:127-129` 同样三条。参数拼装：`web_fetch.rs:119` `curl_arguments`，`web_fetch.rs:65` 把结果传给 `Command::new("curl")`。`lib.rs:1520` `async fn refresh_discover`，`lib.rs:1524` `spawn_blocking`。`lib.rs:1540` `async fn get_repo_community`，`lib.rs:1545` `spawn_blocking`（不刷新也走这条，闭包里 `refresh.unwrap_or(false)`）。上述 grep 无输出。 |
| 4. `git diff --stat` 只出现「允许改动」里的路径；`001`–`030` 迁移文件没有改动 | 是 | `git diff --stat` 只有 `acl-manifests.json`、`gsat-commands.toml`、`db/migrations.rs`、`lib.rs`。未跟踪的新文件也在允许清单内：`web_fetch.rs`、`discover.rs`、`community.rs`、`discover_queries.json`、`031_web_content.sql`。`git diff --stat -- packages/storage/migrations/fox/` 为空，001–030 未改。 |
| 5. 报告写清哪些逻辑因不能联网而没有真实验证，以及假数据覆盖了什么；搜索词里的 `OR`、中文按文档语法写，需总控联网验证 | 是 | 见「没做到的」和「给审核者的提示」。搜索词与任务卡原文一致，由 `query_file_locks_thirteen_scenarios` 锁住 13×2 条。 |

新增测试名（均已在上面的全量 `rust-test` 里通过）：

- 迁移：`web_content_tables_exist_and_second_migrate_is_idempotent`（三张表、索引、非法 source 被 CHECK 拒绝、再迁移不重复、行还在）。
- 联网层：`url_encode_encodes_cjk_space_plus_and_colon`；`rate_limit_wait_secs_on_403_remaining_zero_uses_reset`、`rate_limit_wait_secs_on_429_uses_retry_after`、`rate_limit_wait_secs_none_when_not_limited`；`curl_arguments_are_separate_and_include_limits`（不执行 curl）；另有头文件只留最后一跳、退出码分类、时间往返。
- 发现解析与刷新：`parse_search_skips_incomplete_and_keeps_nulls`、`select_drops_flags_dedupes_sorts_and_limits`、`missing_topics_become_empty`、`since_placeholder_is_encoded_in_the_search_url`（注入 2026-10-03，得到 2026-06-05）；`refresh_replaces_old_rows_and_counts_skipped_mine`、`one_query_failure_still_writes_the_rest`、`all_queries_fail_keep_old_cache`、`rate_limit_on_first_query_keeps_old_cache`、`rate_limit_after_success_stops_and_writes_partial`、`unknown_or_archived_scenario_does_not_fetch`（会 panic 的假联网器）。
- 列表：`list_defaults_to_first_thin`（`scn-data`）、`list_defaults_to_first_when_none_thin`（`scn-xhs`）、`list_uses_explicit_id_and_falls_back`、`mine_is_case_insensitive_and_ignores_removed`、`inbox_counts_open_items_not_archived`、`candidate_count_excludes_mine`、`bad_topics_json_becomes_empty`、`discover_contract_keys_include_nulls`。
- 社区：`parse_hn_builds_item_url_and_drops_blanks`、`parse_issues_keeps_state_and_drops_incomplete`、`refresh_false_does_not_fetch`、`stale_when_missing_expired_or_unparseable`、`exactly_one_day_old_is_fresh`、`failure_keeps_old_payload_and_timestamp`、`items_are_sorted_and_fetched_at_is_earlier`、`community_contract_keys_include_nulls`。

## 改了哪些文件
- `packages/storage/migrations/fox/031_web_content.sql`：新增 `discover_candidates`、`discover_runs`、`repo_community_cache`，并写入 `schema_migrations`。
- `apps/desktop/src-tauri/src/db/migrations.rs`：登记 031；空库幂等测试；被忽略的 dev 库断言从 29 改为 31；测试账号插入放在这里。
- `docs/rebuild/spec/discover_queries.json`：13 个场景各 2 条搜索词，`since_days` 120，`per_scenario_limit` 20。
- `apps/desktop/src-tauri/src/web_fetch.rs`：curl 参数、错误分类、百分号编码、限流等待、ISO 时间。
- `apps/desktop/src-tauri/src/discover.rs`：搜索、替换缓存、只读列表。
- `apps/desktop/src-tauri/src/community.rs`：HN 与 Issues，按来源各自写入。
- `apps/desktop/src-tauri/src/lib.rs`：三个模块和三个命令；刷新与社区都是 `async` + `spawn_blocking`。
- `apps/desktop/src-tauri/permissions/gsat-commands.toml`：允许这三条命令。
- `apps/desktop/src-tauri/gen/schemas/acl-manifests.json`：`rust-check` / `rust-test` 重新生成，只这一份生成文件变了。

## 验证
- `tools/cards/run.sh rust-fmt-fix` → 退出码 0。
- `tools/cards/run.sh rust-fmt` → 退出码 0。
- `tools/cards/run.sh rust-check` → 退出码 0，4 条原有警告。
- `tools/cards/run.sh rust-test discover::` → 修完命令登记断言后，24 passed。
- `tools/cards/run.sh rust-test community::` → 15 passed。
- `tools/cards/run.sh rust-test` → 527 passed，0 failed，5 ignored，1 filtered。
- `grep -n "keychain\|Authorization\|token" web_fetch.rs discover.rs community.rs` → 无输出。
- `git diff --stat` 与 `git status --short` → 只触及允许改动的路径。`packages/storage/migrations/fox/` 下 001–030 无差异。
- 没有启动 App，没有打开界面。这张卡禁止 `pnpm dev:desktop` / `tauri dev`，也没有界面可点。

## 没做到的 / 偏离任务卡的地方
- 没有真的执行 curl，没有请求 GitHub 或 Hacker News。`CurlFetcher::get` 只在生产路径里组好参数；测试只调用 `curl_arguments` 和退出码分类。搜索 JSON、HN `hits`、Issues `items`、403/429 头都是夹具。
- 没有启动 App，没有做界面验收。
- `migrate_live_dev_database_when_explicitly_requested` 仍是 `#[ignore]`，断言已改为版本 31，没有运行。它会写 dev 库。
- 031 的 SQL 比任务卡多了一行 `INSERT INTO schema_migrations(version, name) VALUES('031', 'web_content')`。现有 `apply_one` 要求迁移文件自己写入版本，030 也是这样；不加的话启动会拒绝。
- curl 参数在任务卡列出的标志之外，最前面加了 `-q`（不读用户的 curl 配置，避免配置里夹带请求头），网址前加了 `--`。都在 `curl_arguments` 里，测试锁住了完整参数列表，但没有真跑 curl。
- 列表传入不存在或已归档的场景 id 时，回退到默认选中，不报错。刷新才返回「找不到这个场景」。已归档场景按找不到处理。场景 id 用 `scenarios.id`（如 `scn-xhs`），不是 slug。
- 一条搜索都没成功时（含第一下就限流）不删旧缓存，也不改 `discover_runs`；限流时刻只放在命令返回值。已有成功再限流，则停掉后面的搜索词，并把已拿到的部分写入。`queries` 记实际发出的次数。限流的那一次算 `failedQueries`，不算成功。
- 响应里没有 `items` / `hits` 键算这条失败，不用空结果覆盖旧缓存。`{"items":[]}` / `{"hits":[]}` 算成功。
- 状态是 403/429，但算不出等待秒数时，不当成「停止后续」，记一条失败后继续。能算出秒数才停，并写「约 N 秒后再试」。
- 描述、语言、推送时间是空字符串时写成 null。`archived` / `fork` / `disabled` 只有 JSON `true` 才丢弃，缺字段或 null 当成 false。
- 社区某一来源失败且已有缓存行时，只更新 `error`，保留原来的 `payload_json` 和 `fetched_at`，避免把过期数据标成刚拉过。从来没有行时，插入 `[]`、`fetched_at` 为本次时间，并写下错误。一个来源失败不挡住另一个。
- 恰好 24 小时（`fetched_at == now - 86400`）仍算新鲜；更早、缺行、时间解析不了，算 stale。stale 时仍返回已有条目。
- `inInbox` 用 `capture::extract_links_from_text` 的仓库级 `normalized_url`，再要求收集箱状态不是 `archived`。`pending` / `ready` / `failed` 都算在收集箱里。`mine` 只看同一账号、`sync_status='active'`、名字 ASCII 大小写不敏感。已移除的仓库和其他账号的仓库不算。
- 列表不调用 `seed_scenarios`。没有配置搜索词的用户场景返回「这个场景还没有配置搜索词」，不清缓存。
- 没有 GitHub 账号时，列表和刷新都返回「还没有 GitHub 账号」。
- 测试账号的插入放在 `db/migrations.rs`（`insert_test_account`、`insert_named_test_account`），这样三个新文件里不会出现凭据列名。第二个测试账号的 `updated_at` 固定在 2000 年，避免它变成「最近更新」的账号。

## 需要总控或 Fox 决定的事
- 搜索词按任务卡原文写入，没有改 `OR` 和中文的写法。GitHub 是否按这个语法解释（尤其电商、SEO 那两句），需要总控联网抽查。实现侧不能查。

## 给审核者的提示
- 假联网器在 GitHub 请求上断言只有 `Accept: application/vnd.github+json` 和 `X-GitHub-Api-Version: 2022-11-28`。HN 不带请求头。测试没有出现 `Authorization`。
- `refresh_discover` 和 `get_repo_community` 直接用 `tauri::async_runtime::spawn_blocking`，没有排进全局后台队列，避免被 README 拉取堵住。
- 候选按星数从高到低、名字 ASCII 小写升序。同一 `full_name`（忽略 ASCII 大小写）保留星数更高的一条。社区是 HN 在前（按分数，空分最后），Issues 在后（按评论数），同分再按链接升序。`fetchedAt` 取两个来源里较早的那个原文字符串；只有一侧有缓存时用那一侧。
- `candidateCount` 不含已收藏，列表本身仍包含这些行，并标 `mine`。
- 契约测试比较键的集合，并单独断言该为 null 的键存在且为 null。没有使用 `skip_serializing_if`。
- `acl-manifests.json` 由构建重新生成。生成目录里另外三份 schema 的校验和没有变。
- 全量测试里的 5 个 ignored 是原来就有的，其中包括会写 dev 库的迁移测试，这次没有跑。
