---
编号: 23-inbox-fetch
允许改动:
  - apps/desktop/src-tauri/src/inbox_fetch.rs
  - apps/desktop/src-tauri/src/inbox_view.rs
  - apps/desktop/src-tauri/src/web_fetch.rs
  - apps/desktop/src-tauri/src/capture.rs
  - apps/desktop/src-tauri/src/lib.rs
  - apps/desktop/src-tauri/src/db/migrations.rs
  - apps/desktop/src-tauri/permissions/gsat-commands.toml
  - apps/desktop/src-tauri/gen/schemas/acl-manifests.json
  - packages/storage/migrations/fox/033_inbox_fetch.sql
---

# 卡 23 · 收集箱：把收进来的链接读成「标题 + 摘要」（X 帖子、公众号文章、YouTube、普通网页）

## 为什么有这张卡
收集页现在只认链接。GitHub 仓库链接有 `owner/name` 当标题，**别的链接（X 帖子、公众号文章、视频、博客）在收集箱里的标题就是网址本身**，Fox 看不出每一条是什么、值不值得留。群聊里别人分享的东西大部分恰好就是这些。
这张卡做**后台读取**：对收集箱里还没读过的链接，联网取回公开的标题、作者和一段正文摘要，存进数据库；界面（总控另做）据此显示成「标题 + 两行摘要」，读不到的写清原因。
**只读公开内容**：不登录、不带 cookie、不带任何令牌。读不到（需要登录、被拦、已删除）就如实写原因，不绕。

## Fox 能看到的效果
- 在收集页粘一条 X 链接、一篇公众号文章、一个 YouTube 视频，几秒后收集箱里那一条有了标题（X 是「作者：正文开头」）和一段摘要，而不是一串网址。
- 读不到的（比如公众号拦了自动读取、X 帖子被删）有一句人话原因，可以点「重试」。

## 背景与先读什么
- `capture.rs`：`url_is_safe`（第 321 行附近）只做**字面量**检查，注释里写着「以后真正去抓网页时，还要把域名解析成 IP 后再查一次」——**这张卡就是那个「以后」**。`ip_is_blocked`（私有、回环、链路本地、云元数据 169.254.x、CGNAT、IPv4 映射的 IPv6 等）已有，需要时改成 `pub(crate)` 复用。
- `web_fetch.rs`：`HttpFetcher` trait + `CurlFetcher`（卡 17）。`curl_arguments` 用了 `-L --max-redirs 5`，**自动跟随重定向，不适合抓用户贴来的任意链接**（重定向可能跳到内网地址）。**不要改它给卡 17 的发现/社区用的行为**；这张卡另写一个更严的取页函数/类型（可以放在 `web_fetch.rs` 里新增，也可以放在 `inbox_fetch.rs`）。
- `v2_ops::commit_capture`（`v2_ops.rs` 约 621 行）写 `inbox_items`：`title` 对非 GitHub 链接就是网址；列 `item_type`（`webpage` / `video` / `github_repository`）、`source_platform`（链接所在平台：`x`、`wechat`、`youtube`、`bilibili`、`web` …）、`raw_excerpt`（**留给「原话」，这张卡不要写它**）。**不许改 `v2_ops.rs`。**
- `inbox_view.rs`：`InboxItem`（第 49 行）是列表返回的结构；`list_inbox` 的 SQL 要多取新列。前端类型总控自己改，你只管 Rust 这边的 `serde` 键名（驼峰）。
- 迁移规则：`CLAUDE.md`「Schema 与迁移」；**这张卡用 033**（032 是卡 18 的）；在 `db/migrations.rs` 的 `MIGRATIONS` 登记并补测试；`ALTER TABLE ... ADD COLUMN` 对已有数据安全。
- 命令怎么注册：看 `lib.rs` 里 `generate_handler!`、`permissions/gsat-commands.toml`、`gen/schemas/acl-manifests.json`（改了命令后跑一次 `rust-check` 会重新生成 `acl-manifests.json`，把生成的结果一起留下）。

## 要做的事

### 1. 迁移 `packages/storage/migrations/fox/033_inbox_fetch.sql`
```sql
ALTER TABLE inbox_items ADD COLUMN fetch_status TEXT NOT NULL DEFAULT 'pending'
  CHECK (fetch_status IN ('pending','done','failed','skipped'));
ALTER TABLE inbox_items ADD COLUMN fetch_error TEXT NOT NULL DEFAULT '' CHECK (length(fetch_error) <= 500);
ALTER TABLE inbox_items ADD COLUMN fetched_at TEXT;
ALTER TABLE inbox_items ADD COLUMN page_author TEXT NOT NULL DEFAULT '' CHECK (length(page_author) <= 200);
ALTER TABLE inbox_items ADD COLUMN page_excerpt TEXT NOT NULL DEFAULT '' CHECK (length(page_excerpt) <= 8000);
UPDATE inbox_items SET fetch_status = 'skipped' WHERE item_type = 'github_repository';
INSERT INTO schema_migrations(version, name) VALUES('033', 'inbox_fetch');
```
（`INSERT INTO schema_migrations` 一行的写法照 031、032 现有迁移文件的样子，别发明新写法。）

### 2. 新模块 `src-tauri/src/inbox_fetch.rs`（`lib.rs` 里加 `mod inbox_fetch;`）
**联网必须经过 trait，测试里不许真的联网**：
```rust
pub trait PageFetcher { fn get(&self, url: &str) -> Result<PageResponse, FetchProblem>; }
pub struct PageResponse { pub status: u16, pub content_type: String, pub body: String, pub final_url: String }
```
真实实现 `SafePageFetcher`（用 `curl`，参数风格照 `web_fetch.rs`：`-q -sS --proto =https --max-time 10 --connect-timeout 6 --max-filesize 1000000 -A <见下> -D <头文件>`，**不用 `-L`**）：
1. **每一跳都自己处理重定向**：最多 3 跳；`Location` 相对地址按当前 URL 解析；**每一跳（包括最初的 URL）都要先过 `capture::url_is_safe`，再把主机名解析成 IP（`std::net::ToSocketAddrs`），只要有任何一个解析结果是 `ip_is_blocked` 就拒绝**（防「域名指向内网」）；通过后用 `curl --resolve 主机:端口:IP` 把解析结果固定住，防止解析之后又变（DNS 重绑定）。
2. 只接受 `https`；`http` 链接直接标 `skipped`，原因「只读取 https 链接」。端口只允许 443（其它端口 `skipped`，原因「不读取非常规端口」）。
3. 只接受 `text/html`、`application/xhtml+xml`、`application/json`、`text/plain` 的响应；其它（图片、PDF、下载）标 `skipped`，原因「不是网页」。
4. 响应体最多 1 MB；用 `String::from_utf8_lossy` 解码（不要像 `CurlFetcher` 那样因为非 UTF-8 就整个失败）。
5. User-Agent：X / YouTube 的 oEmbed 用 `fox-stars-lab`；普通网页和公众号用一个常见的桌面 Safari 的 UA 字符串（写成常量，别每次拼）。**不带任何 cookie、令牌、`Authorization`。**

### 3. 内容提取（纯函数，全都有单测；输入是字符串，不联网）
- **X / Twitter**（`x.com` 或 `twitter.com` 的 `/<用户>/status/<数字>`；其它 X 链接——个人主页、话题——`skipped`，原因「不是单条帖子」）：请求官方 oEmbed `https://publish.twitter.com/oembed?url=<把 x.com 换成 twitter.com 的帖子地址，百分号编码>&omit_script=1&dnt=1`，解析 JSON：`author_name`、`html`。正文 = `html` 里第一个 `<p>…</p>` 的纯文本（去标签、解 HTML 实体、`<br>` 当换行、链接 `pic.twitter.com/...` 之类保留）。`title` = `「作者名：正文前 40 个字符…」`，`page_author` = 作者名，`page_excerpt` = 正文（最多 2000 字符）。404/403/5xx → `failed`，原因「帖子不存在、已删除或需要登录才能看」。
- **YouTube**（`youtube.com/watch?v=`、`youtu.be/`、`youtube.com/shorts/`）：官方 oEmbed `https://www.youtube.com/oembed?url=<百分号编码的原链接>&format=json`，取 `title`、`author_name`；`page_excerpt` 留空。
- **其它网页（含 `mp.weixin.qq.com` 公众号文章、B 站、博客）**：`GET` 页面，解析 HTML（可以用 `regex`/手写扫描，**不要执行任何脚本**；如果要加 `scraper` 之类的新依赖，先写进报告停手，不要自己加）：
  - 标题：`og:title` → `twitter:title` → `<title>`（解 HTML 实体、折成一行、最多 120 字符）。
  - 作者：`<meta name="author">`、`og:site_name`；公众号文章里有 `var nickname = "…"` / `profile_nickname` 之类的字段，能取到就取（取不到不报错）。
  - 摘要：`og:description` → `meta name="description"`；再追加正文文字：先找 `<article>`、`id="js_content"`（公众号正文容器）、`<main>`，找不到就用 `<body>`；**整段去掉 `<script>`、`<style>`、`<noscript>`、`<svg>`、注释**，去标签，解实体，空白折成单个空格或换行，取前 2000 个字符。`page_excerpt` = 摘要在前、正文在后，总长最多 2000 字符。
  - 公众号拦截页识别：正文含「环境异常」「完成验证后即可继续访问」「请在微信客户端打开」之一，或 `title` 为空且 `page_excerpt` 少于 30 字 → `failed`，原因「公众号拦住了自动读取，请在微信里打开这篇」。
  - 标题和摘要都空 → `failed`，原因「页面里没有可读的内容」。
- **HTML 实体**：至少处理 `&amp; &lt; &gt; &quot; &#39; &apos; &nbsp;` 和数字实体（`&#12345;`、`&#x4e2d;`），不要引入新依赖，自己写，带测试。
- 所有提取出来的文字都是**不可信的第三方内容**：只存纯文本，不存 HTML；去掉控制字符；**不要把它拼进任何提示词或命令**（这张卡也不碰 AI）。

### 4. 入库与命令
- `pub fn fetch_pending(db_path: &Path, fetcher: &dyn PageFetcher, only_id: Option<&str>, limit: usize) -> Result<FetchSummary, String>`：
  1. 取 `fetch_status = 'pending'` 的条目（给了 `only_id` 就只取那一条，**这时 `failed` 的也允许重试**），按 `created_at` 升序，最多 `limit` 条（命令里传 20）。`github_repository` 类型的直接标 `skipped`（不联网）。
  2. **读数据库、联网、写数据库三步分开**：联网期间不持有数据库连接和事务；一条一条串行处理（不并发）；每条写库一次。
  3. 成功：`fetch_status = 'done'`，`fetch_error = ''`，`fetched_at` = 现在；`title` **只有当它等于 `original_url` 或 `normalized_url` 时才被替换**（用户或别处改过的标题不覆盖），`page_author`、`page_excerpt` 写入。失败：`failed` + 原因；跳过：`skipped` + 原因。**任何情况下都不要改 `status`、`user_note`、`source_context`、`raw_excerpt`。**
  4. 返回 `FetchSummary { done, failed, skipped, remaining }`（`serde` 驼峰；`remaining` = 还在 `pending` 的条数）。
- Tauri 命令 `fetch_inbox_pending(app_handle, id: Option<String>) -> Result<FetchSummary, String>`：在 `spawn_blocking` 里用 `SafePageFetcher` 调上面的函数（照 `lib.rs` 里别的联网命令的写法）。注册命令：`generate_handler!`、`gsat-commands.toml`、`acl-manifests.json`。**不要自动触发**（前端自己调）。
- `inbox_view.rs`：`InboxItem` 增加 `fetch_status: String`、`fetch_error: String`、`page_author: String`、`excerpt: String`（= `page_excerpt` 的前 400 个字符，按字符不是字节截；超过加「…」），`list_inbox` 把它们取出来；已有测试要继续通过。

## 不要做
- 不改前端、`v2_ops.rs`、契约文档、卡外文件；不碰 AI、不碰钥匙串、不读令牌。
- 不在测试里联网（全用 `PageFetcher` 的假实现 + 字符串夹具）。
- 不登录、不带 cookie、不绕过拦截页、不用无头浏览器。
- 不改 `CurlFetcher` / `curl_arguments` 现有行为（卡 17 在用）。

## 验收
1. `tools/cards/run.sh rust-test` 全部通过，新增测试**至少 28 个**（假 `PageFetcher`）：
   - 提取：X oEmbed（普通、带 `<br>`、带实体、中文、超长截断）、YouTube oEmbed、普通网页（`og:*` 齐全、只有 `<title>`、标题里有实体、`<script>`/`<style>` 里的字不进摘要、`<article>` 优先、没有正文容器时用 body）、公众号文章夹具（`js_content`、`nickname`）、公众号拦截页、空页面、非 UTF-8 乱码不崩；实体解码（命名、十进制、十六进制）。
   - 安全：`http` 跳过、非 443 端口跳过、解析到私有/回环/元数据 IP 的域名被拒（给 `PageFetcher` 之前的解析函数写成可注入的，`localhost`、`10.0.0.1`、`169.254.169.254`、`[::1]`、`::ffff:127.0.0.1`、带账号密码的链接都要有用例）；重定向到内网地址被拒；重定向超过 3 跳被拒；非网页类型（`image/png`、`application/pdf`）跳过；响应体超过 1 MB 的处理。
   - 入库：`pending → done/failed/skipped` 各一条；`github_repository` 不联网直接 `skipped`；自定义标题不被覆盖；`only_id` 重试 `failed`；`limit` 生效；不动 `status`/`user_note`/`raw_excerpt`；`remaining` 数字对；迁移后旧的 `github_repository` 条目是 `skipped`。
   - 迁移 033 登记与版本检查的测试（照 031、032 的写法）。
   - `serde` 键集合：`FetchSummary` 是 `done/failed/skipped/remaining`；`InboxItem` 多出 `fetchStatus/fetchError/pageAuthor/excerpt`。
2. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多。
3. `git diff --stat` 里只出现「允许改动」里的路径（含迁移文件）。
4. 报告里给出：`SafePageFetcher` 里「每一跳先查再解析再固定 IP」的那段代码的文件:行号；命令在 `generate_handler!` 和 `gsat-commands.toml` 里的行号；证明没有对任何第三方域名带 cookie/令牌的 curl 参数清单。

## 停止条件
- 需要新依赖（HTML 解析库等）、要改卡外文件、要改 `commit_capture`、要联网才能测——停手并写进报告。
- 发现 `curl --resolve` 在这台机器的 curl 上不可用——停手并写进报告，不要退回到「不固定 IP」的做法。
