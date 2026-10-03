---
编号: 24-inbox-repos
允许改动:
  - apps/desktop/src-tauri/src/inbox_fetch.rs
  - apps/desktop/src-tauri/src/inbox_view.rs
  - apps/desktop/src-tauri/src/capture.rs
  - apps/desktop/src-tauri/src/db/migrations.rs
  - packages/storage/migrations/fox/034_inbox_repos.sql
---

# 卡 24 · 收集箱：从读到的文章里找出它提到的 GitHub 项目

## 为什么有这张卡
Fox 收进来的链接里，很大一部分是公众号文章、博客，标题大概是「推荐 5 个本周很火的 GitHub 项目」「GitHub 热点速览」「吵着要学 AI，看看这些开源项目」。这类文章的**正文里就写着项目地址**（总控用真实公众号文章验证：一篇 7 个、一篇 10 个 `github.com/owner/repo`）。卡 23 已经能把文章读成标题和摘要，但摘要只留前 2000 字，文章里提到的项目就丢了。
这张卡做**后台抽取**：读文章时，顺便把正文里出现的 GitHub 项目找出来存起来；列表（总控另做界面）据此显示「文中提到 7 个 GitHub 项目，其中 3 个已经在你的收藏里」，一键把没收藏的收进收集箱。这样「看到一篇文章 → 里面的项目进入我的收集」就连起来了。

## Fox 能看到的效果
- 收进一篇「推荐 N 个 GitHub 项目」的公众号文章，几秒后那一条下面写着「文中提到 N 个 GitHub 项目（M 个已收藏）」，展开能看到每个项目名和「已收藏 / 已在收集箱 / 新的」；点一下把新的全收进收集箱。

## 背景与先读什么
- `inbox_fetch.rs`（卡 23，已合并）：`fetch_html`、`extract_web`（公众号页面里正文在 `id="js_content"`，普通网页用 `<article>` / `<main>` / `<body>`；`remove_hidden` 已去掉脚本和样式）、`Outcome::Done { title, author, excerpt }`、`write_outcome`（写库）、`inbox_view.rs` 的 `InboxItem` / `load_items` / `read_inbox_item`（列表返回的结构）。
- `capture.rs`：`extract_links_from_text(text) -> Vec<ExtractedLink>`——能从一段文字里抽链接，并把 GitHub 仓库链接归一成 `kind == "github_repo"` + `owner` + `name`（会去掉 `.git`、`/tree/...`、末尾标点，并排除 `/features`、`/topics` 这类不是仓库的路径）。**优先复用它，不要另写一套 GitHub 链接解析。**
- 迁移规则：`CLAUDE.md`「Schema 与迁移」；**这张卡用 034**（033 是卡 23 的）；在 `db/migrations.rs` 的 `MIGRATIONS` 里登记并补测试（照 033 的写法，包括「从 033 升级」的测试）。
- 真实页面的形状见 `inbox_fetch.rs` 里的测试 `real_wechat_page_shape_is_read_in_full`（脚本字符串里有没配对的 `<script>`、`visibility: hidden` 的 `js_content`）。**联网必须经 `PageFetcher`，测试里不许真的联网。**

## 要做的事

### 1. 迁移 `packages/storage/migrations/fox/034_inbox_repos.sql`
```sql
ALTER TABLE inbox_items ADD COLUMN page_repos TEXT NOT NULL DEFAULT '[]' CHECK (length(page_repos) <= 8000);
INSERT INTO schema_migrations(version, name) VALUES('034', 'inbox_repos');
```
`page_repos` 是 JSON 数组，元素是 `"owner/name"` 字符串（最多 30 个），没找到是 `[]`。（`INSERT` 写法和文件头注释照 033。）

### 2. 抽取（`inbox_fetch.rs`，纯函数，全部单测）
`fn github_repos_in(raw_html: &str, page_url: &str) -> Vec<String>`：
1. 取**正文区域**：公众号页面（`is_wechat_url`）用 `id="js_content"` 的内部 HTML；别的页面依次用 `<article>`、`<main>`、`<body>`；都取不到就用去掉脚本样式后的整页。不要在整页的导航、页脚、侧栏里找（那里常有站点自己的 GitHub 链接）。
2. 在正文区域里找两类来源：**`href="…"` 属性值**（先做 HTML 实体解码，处理 `&amp;`）和**可见文字**（去标签后的文本，公众号正文里很多是把网址当文字写的：`项目地址：github.com/owner/repo`、`https://github.com/owner/repo`）。
3. 把这两类来源合成一段文字交给 `capture::extract_links_from_text`，只留 `kind == "github_repo"` 且有 `owner`、`name` 的，输出 `"{owner}/{name}"`。
4. 去重（不分大小写，保留第一次出现的写法），保持出现顺序；**最多 30 个**；丢掉明显不是仓库的：`name` 是 32 位十六进制（公众号页面里有 `repository/<哈希>` 这类内部链接）、`owner` 或 `name` 为空、`owner`/`name` 里有不合法字符（GitHub 用户名只有字母数字和连字符，仓库名还允许 `.` 和 `_`）；页面本身就是 GitHub 的链接（`page_url` 的主机是 `github.com`）不抽取，直接返回空。
5. 失败（解析不了）返回空数组，不报错。

### 3. 入库
- `Outcome::Done` 增加 `repos: Vec<String>`（X / YouTube 的 oEmbed 结果是空数组）。`fetch_html` 成功时把 `github_repos_in(&page.body, url)` 填进去。
- `write_outcome`：成功时把 `repos` 序列化成 JSON 数组写 `page_repos`（超过 8000 字符就从后往前丢元素直到放得下，永远不要因为这个让写库失败）；失败 / 跳过时**不改** `page_repos`。**不要改 `status`、`user_note`、`source_context`、`raw_excerpt`、`updated_at`。**

### 4. 列表（`inbox_view.rs`）
- `InboxItem` 增加 `github_repos: Vec<RepoMention>`（`serde` 驼峰：键 `githubRepos`），`RepoMention { full_name: String, starred: bool, in_inbox: bool }`（键 `fullName` / `starred` / `inInbox`）。
- `starred`：`repositories` 里有这个仓库（`sync_status = 'active'`，`lower(full_name)` 相等）；`in_inbox`：`inbox_items` 里有别的条目的 `normalized_url` 等于 `https://github.com/{owner}/{name}`（不分大小写，**包括已归档的**，因为已经处理过）。
- **一页列表只查两次库**（先收集这一页所有条目的仓库名，再批量查 `repositories` 和 `inbox_items`），不要每个仓库查一次。条目没有 `page_repos`（`[]` 或解析失败）就是空数组。
- 保持原有排序、分页、计数不变；已有测试里的键集合要把 `githubRepos` 加进去。

## 不要做
- 不改前端、`v2_ops.rs`、契约文档、卡外文件；不碰 AI、钥匙串、令牌。
- 不在测试里联网（全用 `PageFetcher` 的假实现 + 字符串夹具）。
- 不新增依赖；不另写一套 GitHub 链接解析；不去 GitHub API 验证这些仓库是否存在。
- 不自动把抽出来的仓库写进收集箱（那是用户点了才做的事，总控界面里调 `commit_capture_batch`）。

## 验收
1. `tools/cards/run.sh rust-test` 全部通过，新增测试**至少 14 个**：
   - 抽取：公众号形状夹具（`js_content` 里 `<a href="https://github.com/a/b">` 和纯文字 `github.com/c/d`、`项目地址：https://github.com/e/f。`）；`&amp;` 的链接；带 `.git`、`/tree/main`、`#readme`、末尾标点的写法；大小写重复只留一个；顺序是第一次出现的顺序；超过 30 个只留前 30；32 位十六进制的 `name` 被丢；导航 / 页脚里的 GitHub 链接不算（只在 `js_content` 之外出现）；`/features`、`/topics/x` 这类不算；页面本身是 `github.com` 时返回空；普通网页用 `<article>`；没有任何链接返回空。
   - 入库：成功写 `page_repos`；失败 / 跳过不改它；超长时丢尾部元素而不是写库失败；不动 `status` / `user_note` / `source_context` / `raw_excerpt` / `updated_at`。
   - 列表：`starred`（含大小写不同）、`in_inbox`（含已归档）、都不是；一页列表只查两次库（可以用「条目很多也能正确返回」间接证明，不必数 SQL 条数）；`serde` 键集合含 `githubRepos` 和 `RepoMention` 的三个键。
   - 迁移 034 登记与「从 033 升级」的测试。
2. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多（先跑一次 `rust-check` 记下基线）。
3. `git diff --stat` 里只出现「允许改动」里的路径（含迁移文件）。
4. 报告里给出：`github_repos_in` 的文件:行号；证明「列表只查两次库」的那段代码的文件:行号；一份用公众号夹具抽出来的 JSON 数组全文。

## 停止条件
- 需要新依赖、要改卡外文件、`extract_links_from_text` 不能满足需求而必须改它的行为（只允许在 `capture.rs` 里加新函数 / 改可见性，**不许改它现有的判断**）——停手并写进报告。
