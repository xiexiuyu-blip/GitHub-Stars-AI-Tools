# 交回报告 · 卡 24-inbox-repos

## 一句话结论
做完了：读文章时抽出正文里的 GitHub 项目，写入 `page_repos`，列表用两次查询标出已收藏和已在收集箱。

## 验收逐条
| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| `tools/cards/run.sh rust-test` 全部通过，新增测试至少 14 个 | 是 | `tools/cards/run.sh rust-test` → `test result: ok. 745 passed; 0 failed; 5 ignored; 0 measured; 1 filtered out; finished in 74.63s`。新增 18 个（抽取 11、入库 4、列表 2、迁移 1）。被滤掉的 1 个是既有钥匙串测试 `auth::tests::macos_secure_store_initializes_keychain_before_entry_creation`，本卡没有跑它。 |
| 抽取：公众号形状夹具（`js_content` 里 `<a href="https://github.com/a/b">` 和纯文字 `github.com/c/d`、`项目地址：https://github.com/e/f。`）；`&amp;` 的链接；带 `.git`、`/tree/main`、`#readme`、末尾标点的写法；大小写重复只留一个；顺序是第一次出现的顺序；超过 30 个只留前 30；32 位十六进制的 `name` 被丢；导航 / 页脚里的 GitHub 链接不算（只在 `js_content` 之外出现）；`/features`、`/topics/x` 这类不算；页面本身是 `github.com` 时返回空；普通网页用 `<article>`；没有任何链接返回空 | 是 | `github_repos_in` 在 `apps/desktop/src-tauri/src/inbox_fetch.rs:784`。公众号夹具 `wechat_js_content_collects_href_text_and_ignores_chrome`（`inbox_fetch.rs:3123`）断言 JSON 全文为 `["a/b","c/d","e/f"]`（3137–3140 行）。其余：`href_and_text_entities_are_decoded`、`git_tree_fragment_and_punctuation_still_name_the_repo`、`same_repo_keeps_the_first_spelling_only`、`repos_keep_first_seen_order`、`repos_keep_only_the_first_thirty`、`hex_names_and_illegal_owners_are_dropped`、`feature_and_topic_paths_are_not_repos`、`github_page_returns_empty`、`ordinary_pages_prefer_article_then_main_then_body`、`page_without_github_repos_returns_empty`。`inbox_fetch::tests`：63 passed，0 failed，2.54s。 |
| 入库：成功写 `page_repos`；失败 / 跳过不改它；超长时丢尾部元素而不是写库失败；不动 `status` / `user_note` / `source_context` / `raw_excerpt` / `updated_at` | 是 | `page_repos_json` 在 `inbox_fetch.rs:1784`，超长从尾部丢，序列化失败或一个元素就超长时写成 `[]`，写库仍成功。测试：`successful_fetch_writes_page_repos_without_touching_user_fields`（3317）、`failed_or_skipped_fetch_does_not_change_page_repos`、`oversized_repo_list_drops_tail_and_still_writes`（3368）、`oembed_success_stores_empty_repo_list`（X / YouTube 成功写成空数组）。 |
| 列表：`starred`（含大小写不同）、`in_inbox`（含已归档）、都不是；一页列表只查两次库；`serde` 键集合含 `githubRepos` 和 `RepoMention` 的三个键 | 是 | 两次查询的入口注释在 `inbox_view.rs:370`，`fill_repo_mentions` 在 408 行；没有仓库名时 429–431 行直接返回。收藏 SQL 在 `starred_full_names` 455–467 行，收集箱 SQL 在 `inbox_urls_by_id` 479–490 行，都用 `json_each`。`list_inbox_marks_starred_and_inbox_mentions`（1640）覆盖大小写、已归档、别的账号、已移除、自己提到自己。`list_inbox_many_items_keep_repo_flags`（1845）用 80 条间接证明批量查询。契约测试断言了 `githubRepos` 以及 `fullName` / `inInbox` / `starred`。`inbox_view::tests`：23 passed，0 failed，3.62s。 |
| 迁移 034 登记与「从 033 升级」的测试 | 是 | `packages/storage/migrations/fox/034_inbox_repos.sql` 已按卡内 SQL 追加，并在 `db/migrations.rs` 的 `MIGRATIONS` 登记。`upgrade_from_033_adds_empty_page_repos`（`db/migrations.rs:923`）通过：旧行默认 `[]`，8000 字符通过、8001 被 CHECK 拒绝，再迁移不重复应用。`db::migrations::tests`：8 passed，0 failed，2 ignored，1.57s。 |
| `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多 | 是 | `rust-fmt` 退出码 0。`rust-check` 退出码 0，`gsat-desktop (lib) generated 5 warnings`，与开工前基线相同：`MigrationReport.applied` / `backup_path`、`js_assigned_string`、`ensure_ready`、`repair_model_json`、`restore_database_file`。没有第 6 条。 |
| `git diff --stat` 里只出现「允许改动」里的路径（含迁移文件） | 是 | 见下方「验证」。未跟踪的新文件也在允许改动里。`capture.rs` 在允许改动里，没有改。 |
| 报告里给出：`github_repos_in` 的文件:行号；证明「列表只查两次库」的那段代码的文件:行号；一份用公众号夹具抽出来的 JSON 数组全文 | 是 | `github_repos_in`：`apps/desktop/src-tauri/src/inbox_fetch.rs:784`。两次查询：`apps/desktop/src-tauri/src/inbox_view.rs:455`（收藏）和 `:479`（收集箱），由 `:370` 调用 `:408`。公众号夹具 JSON 全文：`["a/b","c/d","e/f"]`。 |

## 改了哪些文件
- `packages/storage/migrations/fox/034_inbox_repos.sql`：给 `inbox_items` 追加 `page_repos`，默认 `[]`，长度不超过 8000，并登记迁移 034。
- `apps/desktop/src-tauri/src/db/migrations.rs`：`MIGRATIONS` 登记 034；原来写死「最新是 033」的断言改成 034；新增从 033 升级的测试。
- `apps/desktop/src-tauri/src/inbox_fetch.rs`：`github_repos_in`；成功时把仓库名单写入 `page_repos`；失败和跳过不改这一列；超长从尾部丢元素。
- `apps/desktop/src-tauri/src/inbox_view.rs`：`RepoMention` 和 `githubRepos`；一页先读出仓库名，再最多查两次库。

## 验证
- `tools/cards/run.sh rust-test inbox_fetch::tests` → 63 passed，0 failed，2.54s
- `tools/cards/run.sh rust-test inbox_view::tests` → 23 passed，0 failed，3.62s
- `tools/cards/run.sh rust-test db::migrations::tests` → 8 passed，0 failed，2 ignored，1.57s
- `tools/cards/run.sh rust-test` → 745 passed；0 failed；5 ignored；1 filtered out；74.63s
- `tools/cards/run.sh rust-fmt` → 退出码 0
- `tools/cards/run.sh rust-check` → 退出码 0，5 条警告，与开工前基线相同
- `git status --short` → `M apps/desktop/src-tauri/src/db/migrations.rs`、`M apps/desktop/src-tauri/src/inbox_fetch.rs`、`M apps/desktop/src-tauri/src/inbox_view.rs`、`?? packages/storage/migrations/fox/034_inbox_repos.sql`
- `git diff --stat` → 上面 3 个已跟踪文件，1114 insertions、46 deletions。没有卡外路径。新迁移文件未跟踪，所以不出现在 `--stat` 里，它在允许改动清单中。

## 没做到的 / 偏离任务卡的地方
- 没有改前端。任务卡写明不改前端，界面由总控另做。`apps/desktop/src/lib/contracts.ts` 的 `InboxItem` 还没有 `githubRepos`，运行时多出来的 JSON 会被忽略。
- `starred` 和 `in_inbox` 都限定在当前账号（与列表其它查询一样，取 `updated_at` 最新的 GitHub 账号）。任务卡对 `starred` 没写 `account_id`。另一个账号里的 active 仓库或收集箱条目不算。这是更严、可回退的读法，`list_inbox_marks_starred_and_inbox_mentions` 锁住了它。`sync_status` 不是 `active` 的不算已收藏。
- `in_inbox` 排除当前行自己。一条 GitHub 收集如果在 `page_repos` 里提到自己，不算「已在收集箱」。
- 选中的区域是 `<body>` 时，body 里面的 nav、footer、aside 会算进去。任务卡要求依次用 body，没有要求再剥这些标签。区域外面的导航、页脚、侧栏不参与。公众号找不到 `js_content` 时用去掉脚本和样式后的整页，不再去试 article、main、body。`content_html_for_repos`（`inbox_fetch.rs:800`）的注释把「整页回退也不含导航」说重了，整页回退会包含页内导航。
- 没有改 `capture.rs`。owner 只留字母、数字和 `-`，比 `valid_github_name` 更严；32 位十六进制和最多 30 个是抽取之后的过滤。
- 没有跑、也没有改被 `#[ignore]` 的 `migrate_live_dev_database_when_explicitly_requested`。它仍断言 schema 版本 32（033 时就已经过时）。跑它会写开发库。
- 没有改 `packages/storage/src/migrations.ts`。那是另一份 001–003 清单。

## 需要总控或 Fox 决定的事
无。下一步界面要自己加上 `githubRepos`，任务卡已经把界面交给总控，不阻塞本卡合并。

## 给审核者的提示
- 抽取复用 `capture::extract_links_from_text`，没有改它现有的判断。`gist.github.com` 不当成 `github.com`。主机比较走 `bare_host`（去掉 `www.`，忽略大小写）。用的是传入的 `url`，不是跳转后的 `final_url`。
- 成功写入会覆盖 `page_repos`。X / YouTube 成功时写成 `[]`。失败和跳过的 SQL 不提这一列。空标题时参数是 `[author, excerpt, page_repos, id]`，有标题时再加 title。仍不改 `status`、`user_note`、`source_context`、`raw_excerpt`、`updated_at`。标题等于原链接时仍可能被换成页面标题，这是卡 23 的原行为。
- 单个元素序列化后超过 8000 字符时，整列写成 `[]`，写库仍然成功。
- 两次查询用 `json_each`。捆绑的 SQLite 能用，测试已通过。这一页没有仓库名时，额外查询次数是 0。
- 原来「最新迁移是 033」的断言改成了 034。断言跟着新迁移走，没有放宽，也没有给测试加 `#[ignore]`。
- 公众号夹具抽出来的 JSON 数组全文：`["a/b","c/d","e/f"]`。
