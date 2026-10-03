# 交回报告 · 卡 23-inbox-fetch

## 一句话结论
做完了：收集箱里的公开链接可以在后台读成标题、作者和摘要，读不到就写下原因；`rust-test`、`rust-fmt`、`rust-check` 都通过。

## 验收逐条
| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| 1. `tools/cards/run.sh rust-test` 全部通过，新增测试**至少 28 个**（假 `PageFetcher`） | 是 | 全量：`646 passed; 0 failed; 5 ignored; 0 measured; 1 filtered out`（约 60 秒）。5 个 ignored 都是原来的 dev 库 / 本机模型用例，本卡没有加 `#[ignore]`。被滤掉的 1 个是 `run.sh` 固定跳过的钥匙串用例。新增测试函数 44 个：`inbox_fetch.rs` 42 个，外加迁移 `upgrade_from_032_adds_inbox_fetch_and_skips_existing_github`、列表 `list_inbox_clips_excerpt_by_chars_not_bytes`。过滤跑 `rust-test inbox_fetch`：43 passed（42 + 名字里带 inbox_fetch 的迁移测试）。 |
| 提取：X oEmbed（普通、带 `<br>`、带实体、中文、超长截断）、YouTube oEmbed、普通网页（`og:*` 齐全、只有 `<title>`、标题里有实体、`<script>`/`<style>` 里的字不进摘要、`<article>` 优先、没有正文容器时用 body）、公众号文章夹具（`js_content`、`nickname`）、公众号拦截页、空页面、非 UTF-8 乱码不崩；实体解码（命名、十进制、十六进制）。 | 是 | `x_oembed_plain_text`、`x_oembed_br_becomes_newline_and_keeps_pic_link`、`x_oembed_decodes_entities`、`x_oembed_chinese_title_counts_chars`、`x_oembed_truncates_title_at_40_chars_and_excerpt_at_2000`、`youtube_oembed_keeps_title_and_author_and_empty_excerpt`、`youtube_shorts_and_youtu_be_are_videos`（shorts / youtu.be / m.youtube.com 的 oEmbed 地址含百分号编码）、`html_og_fields_win_and_article_beats_main`、`html_title_only_is_success_with_empty_excerpt`、`html_title_entities_collapse_to_one_line`、`html_script_style_comment_and_svg_stay_out_of_excerpt`、`html_uses_body_when_no_container`、`wechat_reads_js_content_and_nickname_not_the_script`、`wechat_profile_nickname_element_is_author_when_js_missing`、`wechat_block_page_and_too_short_page_fail`、`empty_page_fails_and_short_non_wechat_page_does_not`、`non_utf8_body_decodes_without_panic`、`entities_cover_named_decimal_and_hex_once`。 |
| 安全：`http` 跳过、非 443 端口跳过、解析到私有/回环/元数据 IP 的域名被拒（给 `PageFetcher` 之前的解析函数写成可注入的，`localhost`、`10.0.0.1`、`169.254.169.254`、`[::1]`、`::ffff:127.0.0.1`、带账号密码的链接都要有用例）；重定向到内网地址被拒；重定向超过 3 跳被拒；非网页类型（`image/png`、`application/pdf`）跳过；响应体超过 1 MB 的处理。 | 是 | `http_is_skipped_before_fetch_and_resolve`、`http_row_is_skipped_without_calling_fetcher`、`non_443_port_is_skipped_and_443_is_allowed`、`literal_private_loopback_metadata_and_credentials_are_rejected`、`resolver_private_loopback_metadata_and_mapped_are_rejected`（含「一个公网 IP 混一个 10.0.0.1」）、`redirect_to_intranet_is_rejected_without_a_second_fetch`、`redirect_over_three_hops_is_rejected`、`relative_redirect_is_followed_and_three_hops_can_succeed`、`image_and_pdf_are_skipped`、`body_over_one_megabyte_is_skipped_and_exact_limit_is_kept`。解析器是 `HostResolver`，测试用 `MapResolver`，不调用系统 DNS。 |
| 入库：`pending → done/failed/skipped` 各一条；`github_repository` 不联网直接 `skipped`；自定义标题不被覆盖；`only_id` 重试 `failed`；`limit` 生效；不动 `status`/`user_note`/`raw_excerpt`；`remaining` 数字对；迁移后旧的 `github_repository` 条目是 `skipped`。 | 是 | `pending_page_becomes_done_and_replaces_url_title`、`fetch_failure_records_reason_and_leaves_user_fields`、`http_row_is_skipped_without_calling_fetcher`、`github_repository_is_skipped_without_network`、`custom_title_is_not_overwritten`、`only_id_retries_failed_and_limit_keeps_the_rest_pending`、`x_status_row_uses_twitter_oembed_without_cookies`。这些用例都断言 `status` / `user_note` / `source_context` / `raw_excerpt` / `updated_at` 原样。迁移：`upgrade_from_032_adds_inbox_fetch_and_skips_existing_github`。 |
| 迁移 033 登记与版本检查的测试（照 031、032 的写法）。 | 是 | `db/migrations.rs` 在 032 后登记 `033 inbox_fetch`（约 209–213 行）。原有「最新版本 / 二次迁移 / 从 031 升级」断言改为包含 033；新增从 032 升级的测试。SQL 语句与任务卡一致，`INSERT` 写法与 032 相同。 |
| `serde` 键集合：`FetchSummary` 是 `done/failed/skipped/remaining`；`InboxItem` 多出 `fetchStatus/fetchError/pageAuthor/excerpt`。 | 是 | `fetch_summary_serde_keys_are_camel_case_words`。`inbox_view` 原契约测试的键列表已加上这四项，并断言默认是 `pending` / 空作者 / 空摘要。`list_inbox_clips_excerpt_by_chars_not_bytes`：400 个「测」不加省略号，401 个截成 400 个再加「…」。 |
| 2. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多。 | 是 | `rust-fmt` 退出码 0。`rust-check` 退出码 0，`Finished dev profile`，4 条警告：`MigrationReport.applied` / `backup_path`（`db/migrations.rs:14`，结构体本卡没改），以及 `v2_ops.rs` 里原有的 `ensure_ready`、`repair_model_json`、`restore_database_file`（该文件不在 diff 里）。`inbox_fetch.rs` 没有警告。编译中途曾有两条本卡引入的警告（未使用的 IP 类型导入、`Youtube` 里没读的字符串），已删掉，最终检查不再出现。 |
| 3. `git diff --stat` 里只出现「允许改动」里的路径（含迁移文件）。 | 是 | `git status --short` 只有：`acl-manifests.json`、`gsat-commands.toml`、`capture.rs`、`db/migrations.rs`、`inbox_view.rs`、`lib.rs`，以及未跟踪的 `inbox_fetch.rs`、`033_inbox_fetch.sql`。`git diff --stat`：6 files changed, 245 insertions(+), 10 deletions(-)。`web_fetch.rs` 在允许清单里，但没有改。 |
| 4. 报告里给出：`SafePageFetcher` 里「每一跳先查再解析再固定 IP」的那段代码的文件:行号；命令在 `generate_handler!` 和 `gsat-commands.toml` 里的行号；证明没有对任何第三方域名带 cookie/令牌的 curl 参数清单。 | 是 | 见下面三节。 |

每一跳（`inbox_fetch.rs`）：

- `SafePageFetcher::get` 第 89–92 行：调用 `fetch_guarded`，传输层是 `curl_once_pinned`，不递归调用自己。
- `fetch_guarded` 第 349–357 行：每一跳先 `gate_request`（第 378–392 行：只留 https、非 443 跳过、再调 `capture::url_is_safe`），再 `resolver.resolve(host, 443)`，再 `checked_ip`（第 399–405 行：任何一个地址被 `ip_is_blocked` 就拒绝，此时还没调用传输层），通过后才把第一个公网 IP 交给传输层。
- `curl_once_pinned` 第 504–518 行：再次拒绝内网 IP，然后用 `safe_curl_arguments` 带上 `--resolve`。

命令注册：

- `generate_handler!`：`apps/desktop/src-tauri/src/lib.rs:7106`（`fetch_inbox_pending`，紧挨 `list_inbox`）。命令本体在 `lib.rs:1179`，`spawn_blocking` + `SafePageFetcher` + 上限 20。没有在启动时调用。
- `apps/desktop/src-tauri/permissions/gsat-commands.toml:93`。
- `apps/desktop/src-tauri/gen/schemas/acl-manifests.json` 第 1 行，`"list_inbox"` 后面插入了 `"fetch_inbox_pending"`。和原文件相比只多这 22 个字节。

curl 参数（`safe_curl_arguments`，第 470–491 行），顺序固定为：

`-q` `-sS` `--proto` `=https` `--proto-redir` `=https` `--max-time` `10` `--connect-timeout` `6` `--max-filesize` `1000000` `-A` `<UA 常量>` `-D` `<头文件>` `--resolve` `<主机:443:IP>` `--` `<url>`

没有 `-L`、`-b`、`--cookie`、`-H`、`-u`、`--oauth2-bearer`，参数字符串里也不含 cookie、authorization、bearer、token。单测 `safe_curl_arguments_pin_ip_and_carry_no_credentials` 锁住这份清单。oEmbed 的 UA 常量是 `fox-stars-lab`，普通网页是写死的桌面 Safari 字符串（第 24–28 行），不是每次拼出来的。IPv6 钉成 `主机:443:[地址]`。

## 改了哪些文件
- `packages/storage/migrations/fox/033_inbox_fetch.sql`：追加读取状态、错误、时间、作者、摘要；已有 GitHub 仓库标成跳过。
- `apps/desktop/src-tauri/src/db/migrations.rs`：登记 033，补升级测试，原有版本断言改到 033。
- `apps/desktop/src-tauri/src/inbox_fetch.rs`：新模块。假抓取器、安全跳转、三种页面的提取、入库和测试。
- `apps/desktop/src-tauri/src/lib.rs`：`mod inbox_fetch`、命令 `fetch_inbox_pending`、`generate_handler!`。
- `apps/desktop/src-tauri/permissions/gsat-commands.toml`：允许该命令。
- `apps/desktop/src-tauri/gen/schemas/acl-manifests.json`：同一条命令。
- `apps/desktop/src-tauri/src/inbox_view.rs`：列表带上读取状态、错误、作者，以及按字符截断的摘要。
- `apps/desktop/src-tauri/src/capture.rs`：`ip_is_blocked` 改为 `pub(crate)`，判断本身没改。

## 验证
- `tools/cards/run.sh rust-fmt-fix` → 退出码 0。
- `tools/cards/run.sh rust-test inbox_fetch` → `43 passed; 0 failed`。
- `tools/cards/run.sh rust-fmt` → 退出码 0。
- `tools/cards/run.sh rust-check` → 退出码 0，4 条原有警告，见上表。
- `tools/cards/run.sh rust-test` → `646 passed; 0 failed; 5 ignored; 1 filtered out`。
- `git status --short` 与 `git diff --stat` → 只有允许改动的路径。

## 没做到的 / 偏离任务卡的地方
- 没有对公网发请求，也没有在本轮执行 curl。测试全部走假 `PageFetcher` / 可注入解析器。停止条件里的「`--resolve` 不可用就停手」没有触发：实现按卡使用 `--resolve`，没有退回到不固定 IP。
- `PageResponse` 多了一个 `location: Option<String>`。任务卡给出的结构没有这个字段。重定向目标放在这里，测试不用真 curl 也能走跳转；真实 curl 从响应头填它。
- 失败和跳过也会写 `fetched_at`。任务卡只要求成功时写。`updated_at` 不改。
- 作者优先级：`meta name=author`，其次公众号昵称，再次 `og:site_name`。任务卡把前两个和昵称并列，没有规定先后。昵称从原始 HTML 里取（它在 `<script>` 中），正文摘要则先去掉 script/style/noscript/svg 和注释。
- 公众号拦截语除了正文，标题和摘要里出现也会判失败。比「只看正文」更严。
- 摘要是「简介换行后再接正文」，两段重复也不去重，总长 2000 字符。
- 作者为空时标题是 `「正文」`，不写冒号。正文超过 40 个字符才加「…」。
- 普通网页的 HTTP 错误文案是自行写的短句：404「页面不存在」，401/403「页面拒绝访问」，5xx「页面暂时打不开」，其它「页面返回了异常状态 {状态码}」。X 的 403/404/5xx 用任务卡原文。YouTube 的 403/404/5xx 用「视频不存在、已删除或暂时打不开」。JSON 解析失败分别是「帖子内容读不出来」「视频信息读不出来」；没有 `<p>` 是「帖子里没有正文」；没有视频标题是「视频没有标题」。
- GitHub 仓库跳过原因是「GitHub 仓库不读取网页」。任务卡只要求不联网并标成 skipped，没给这句原文。
- 入库前标题再截到 500 字符；YouTube 标题先截到 300。网页标题按卡截到 120。数据库的 title 列没有长度 CHECK，这是多加的上限。
- 不按账号过滤，卡里的 SQL 也没有账号条件。
- 没有单独的取消令牌。一次最多 20 条，读完一条再写一条；没写上的仍是 `pending`，可以再跑。`only_id` 只重试这一条的 pending/failed，skipped 不会被自动重试。
- `http://` 在 `url_is_safe` 之前就跳过，原因是「只读取 https 链接」。因此「http 加内网地址」记成跳过，不记成内网失败。
- 非 2xx 不按「不是网页」处理，好让 X/YouTube 的 404 走各自的失败原因。只有 2xx 才检查内容类型。
- 相对 `Location` 用 `Url::join`。`https://example.com/dir/start` 加 `/next` 再加 `done`，结果是 `https://example.com/done`。
- 桌面迁移走 Rust 的 `include_str`。`packages/storage/src/migrations.ts` 不在允许改动里，没有改。
- 迁移文件开头多了两行注释，句式照 031/032；后面的 SQL 与任务卡一致。

## 需要总控或 Fox 决定的事
- 无

## 给审核者的提示
- 不要在测试里构造 `SafePageFetcher`：它会做系统 DNS，再尝试 curl。跳转和钉 IP 的测试走 `fetch_guarded` + `MapResolver`。
- `CurlFetcher` / `curl_arguments` 没改。新的参数表在 `inbox_fetch.rs` 的 `safe_curl_arguments`。
- 读库、联网、写库是分开的。每条成功或失败各开一次 `IMMEDIATE` 事务。不改 `status`、`user_note`、`source_context`、`raw_excerpt`、`updated_at`。标题只有在仍等于 `original_url` 或 `normalized_url` 时才替换；提取出的标题为空时不改原标题。
- 非测试代码没有 `unwrap()` / `expect()`。`expect` 只出现在 `mod tests` 里。
- 全量结果里的 5 个 ignored、1 个 filtered 都不是这张卡新增的。
