# 交回报告 · 卡 22-local-api-capture

## 一句话结论
做完了：本地 API 增加了收集专用令牌和 `POST /v1/capture`，旧的粘贴收集行为不变。

## 验收逐条
| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| `tools/cards/run.sh rust-test` 全部通过，新增测试至少 24 个 | 是 | 全量：`673 passed; 0 failed; 5 ignored; 1 filtered out`（过滤的是契约里要跳过的钥匙串用例）。新增 38 个 `#[test]`：`local_api.rs` 35 个（原有 9 个，现 44 个）+ `v2_ops.rs` 3 个。 |
| 认证矩阵：无令牌 401；错误令牌 401；完整令牌访问 `/v1/capture` 200；收集专用令牌访问 `/v1/capture` 200；收集专用令牌访问 `/v1/search`、`/v1/usage` → 403；完整令牌访问其它路由行为不变；控制文件没有 `captureToken` 时空 Bearer 不能通过；两个令牌分别重置后旧的失效、另一个不受影响；带 `Origin` 头 → 拒绝；错误的 `Host` → 拒绝 | 是 | `capture_without_a_token_is_unauthorized`、`capture_with_a_wrong_token_is_unauthorized`、`full_token_can_capture`、`capture_token_can_capture`、`capture_token_cannot_search`、`capture_token_cannot_record_usage`、`full_token_still_opens_the_other_routes`、`empty_bearer_is_rejected_when_capture_token_is_absent`、`rotating_either_token_leaves_the_other_alone`、`capture_rejects_requests_that_carry_origin`、`capture_rejects_a_foreign_host`。认证分支在 `local_api.rs:397-427`。 |
| 请求校验：缺 `Content-Type` 415；非 JSON 400；`text` 和 `urls` 都空 400；`urls` 101 条 400；单条 URL 2049 字符 400；`text` 50001 字符 400；`sourceContext` 超长被截断而不是报错；字段类型错误 400 | 是 | `capture_requires_json_content_type`、`capture_rejects_a_body_that_is_not_json`、`capture_rejects_empty_text_and_urls`、`capture_rejects_more_than_one_hundred_urls`（100 条仍 200）、`capture_rejects_a_url_over_2048_chars`（2048 仍 200）、`capture_rejects_text_over_50000_chars`（50000 个 ASCII、以及 20000 个汉字仍 200）、`capture_truncates_source_context_instead_of_failing`（2001 个「群」截成 2000）、`capture_rejects_mistyped_fields`。 |
| 行为：只传 `urls`；只传 `text`；两者都传（合成顺序）；`sourcePlatform` 合法值写进 `capture_batches.source_platform`、非法值变 `other`；条目的 `captured_via` 是 `local_api`；条目的 `source_platform` 仍是链接平台（GitHub 链接是 `github`）；`raw_excerpt` 有原话且被截到 2000 字符；同一请求重发 `duplicateBatch: true`；已 Star 的仓库计入 `alreadyStarred`；内网地址计入 `blocked`；没有链接 200 且 `linksFound: 0`；响应 JSON 键集合 | 是 | `capture_with_only_urls_creates_items`、`capture_with_only_text_creates_items`、`capture_puts_urls_before_text`（正向合成与 `commit_capture` 撞成重复批，反过来不是）、`capture_stores_a_known_source_platform_on_the_batch`、`capture_turns_an_unknown_platform_into_other`、`capture_marks_via_local_api`、`capture_keeps_the_link_platform_on_the_item`（GitHub 是 `github`，X 是 `x`，批次仍是 `wechat`）、`capture_stores_the_original_words_trimmed_to_2000_chars`、`sending_the_same_capture_again_is_a_duplicate_batch`、`capture_counts_repositories_already_starred`、`capture_counts_blocked_private_addresses`、`capture_with_no_link_is_still_ok`、`capture_response_json_has_only_the_commit_keys`。键是 `alreadyInInbox`、`alreadyStarred`、`blocked`、`created`、`duplicateBatch`、`linksFound`、`ok`、`skipped`。 |
| 旧入口：`commit_capture` 旧测试全部原样通过，且新增一个断言它仍写 `paste_batch` / `other` | 是 | 旧测试 `paste_twice_does_not_duplicate_and_solve_quotes_summary`、`capture_skips_starred_and_blocked_links_without_aborting_the_batch`、`text_without_links_leaves_no_batch_behind`、`failed_insert_rolls_the_whole_batch_back` 在全量里通过。新断言：`commit_capture_still_writes_paste_batch_and_other`（批次 `other`、条目 `paste_batch`、`raw_excerpt` 空、链接平台仍是 `github`）。 |
| 控制文件：新生成的文件权限是 0600；写 `captureToken` 不丢 `enabled`、`token` 等已有键 | 是 | `a_new_capture_token_file_is_private`（`mode & 0o777 == 0o600`，第二次 `ensure` 不轮换）。`saving_the_capture_token_keeps_the_other_control_keys`（`enabled`、`host`、`port`、`token`、额外的 `note` 都还在；两把令牌分别轮换互不影响）。 |
| `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多 | 是 | `rust-fmt` 退出码 0。`rust-check` 通过，4 条 `dead_code`，都在原有代码：`db/migrations.rs:14` 的 `MigrationReport.applied` / `backup_path`，`v2_ops.rs:81` `ensure_ready`，`v2_ops.rs:576` `repair_model_json`，`v2_ops.rs:1201` `restore_database_file`。新增代码没有警告。改前没有单独跑过，不能附上改前的数字；这 4 个符号本次都没改。 |
| `git diff --stat` 里只出现「允许改动」里的路径 | 是 | 已跟踪的 5 个文件都在允许清单里（见下）。新建的 `docs/rebuild/微信联动接入.md` 还没进 git，`git status` 显示为未跟踪，路径也在允许清单里。 |
| 报告里给出：路由表里认证分支的文件:行号；证明收集专用令牌访问非 `/v1/capture` 路由返回 403 的测试名；命令在 `generate_handler!` 和 `gsat-commands.toml` 里的行号 | 是 | 认证分支 `local_api.rs:397-427`（`POST /v1/capture` 在 409-414 接受两把令牌；其它路由在 416-418 对收集专用令牌返回 403）。测试名：`capture_token_cannot_search`（`local_api.rs:1034`，另测了 `/v1/nothing` 和 `GET /v1/capture`）、`capture_token_cannot_record_usage`（`local_api.rs:1046`）。`generate_handler!`：`lib.rs:7094-7095`。`gsat-commands.toml:88-89`。 |

## 改了哪些文件
- `apps/desktop/src-tauri/src/v2_ops.rs`：新增 `CaptureSource` 和 `commit_capture_with`；旧 `commit_capture` 改为调用它，仍写 `other` / `paste_batch`、不写原话。
- `apps/desktop/src-tauri/src/local_api.rs`：控制文件增加独立的 `captureToken`；`POST /v1/capture`；收集专用令牌不能走其它路由。
- `apps/desktop/src-tauri/src/lib.rs`：工具箱命令 `get_local_api_capture_token`、`rotate_local_api_capture_token`，并登记进 `generate_handler!`。
- `apps/desktop/src-tauri/permissions/gsat-commands.toml`：同一对命令加入权限清单。
- `apps/desktop/src-tauri/gen/schemas/acl-manifests.json`：按现有格式把这两个命令插进 `allow` 列表。`rust-check` 没有再改这个文件。
- `docs/rebuild/微信联动接入.md`：给微信工作台开发者的一页说明（约 70 行）。

## 验证
- `tools/cards/run.sh rust-fmt-fix` → 退出码 0。
- `tools/cards/run.sh rust-fmt` → 退出码 0。
- `tools/cards/run.sh rust-check` → 通过；4 条原有 `dead_code` 警告，见上表。
- `tools/cards/run.sh rust-test capture` → 48 passed（含新测试和原有链接抽取测试）。
- `tools/cards/run.sh rust-test full_token_still` → `full_token_still_opens_the_other_routes` passed。
- `tools/cards/run.sh rust-test rotating_either` → `rotating_either_token_leaves_the_other_alone` passed。
- `tools/cards/run.sh rust-test` → `673 passed; 0 failed; 5 ignored; 1 filtered out`，54.20s。
- `git diff --stat` → 只列出上面 5 个已跟踪文件，`1186 insertions, 16 deletions`。

## 没做到的 / 偏离任务卡的地方
- 没有。界面、CORS、监听地址、`Origin` 规则、`MAX_BODY_BYTES`、迁移和卡外文件都没动。

## 需要总控或 Fox 决定的事
- 无。

## 给审核者的提示
- 完整令牌的 `get_local_api_token`（`lib.rs:1112` 一带）不会自动生成，没有令牌就返回「还没有令牌」。收集专用令牌的 `get_local_api_capture_token`（`lib.rs:1126`）按任务卡调用 `ensure_capture_token`，没有就生成。这是两套命令的差别，不是把完整令牌的行为改了。
- `GET /v1/health` 仍在认证之前放行（`local_api.rs:390`）。带收集专用令牌访问它也是 200，因为健康检查本来就不看令牌。`GET /v1/capture` 不是收集接口：收集专用令牌得到 403，完整令牌得到 404。
- 平台按原样匹配允许列表，大小写不对（例如 `WeChat`）写成 `other`，不报错。`via` 只保留 `local_api` 和 `paste_batch`，别的当 `paste_batch`。
- 「没有可收集的内容」看的是字符串或数组元素是否为空，不先去掉空白。原话写入 `raw_excerpt` 时才去掉首尾空白，再按字符截到 2000。截断用 `.chars()`，和原来的 `source_context` 一样，按 Unicode 标量，不按字节。
- 字数上限和 64KB 请求体上限同时生效，64KB 没有放宽。纯汉字大约两万字就会先被读请求的 413 拦住，到不了 50000 字的校验。微信一条消息远小于这个，文档里写了 413。`route()` 的测试直接喂正文，所以 50000 / 50001 的字符校验是测到的。
- 写库失败一律 500，正文固定为 `{"error":"收集写入失败"}`，不带 SQLite 原文。见 `capture_database_errors_stay_generic`。
- 批次去重仍是合成后正文的哈希。`sourceContext` 不参与。同一段 `text` 换了上下文，第二次仍是 `duplicateBatch: true`。
- 测试令牌都是 `generate_token()` 现造的，库在临时目录。没有读真实数据目录，代码和文档里没有真实令牌。
