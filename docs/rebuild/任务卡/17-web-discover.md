---
编号: 17-web-discover
允许改动:
  - apps/desktop/src-tauri/src/web_fetch.rs
  - apps/desktop/src-tauri/src/discover.rs
  - apps/desktop/src-tauri/src/community.rs
  - apps/desktop/src-tauri/src/lib.rs
  - apps/desktop/src-tauri/src/db/migrations.rs
  - packages/storage/migrations/fox/031_web_content.sql
  - docs/rebuild/spec/discover_queries.json
  - apps/desktop/src-tauri/permissions/gsat-commands.toml
  - apps/desktop/src-tauri/gen/schemas/acl-manifests.json
---

# 卡 17 · 联网内容：发现（按场景缺口找新项目）与社区（别人怎么用）的后台

## Fox 能看到的效果（界面由后面的卡接）
- 「发现」页：按业务场景（电商、SEO·增长这些「偏少」的方向排前面）列出 GitHub 上**他还没收藏**的相关项目，每个带星数、语言、最近更新、原文描述，可以一键「收进收集箱」。数据来自 GitHub 搜索，点「刷新」才联网，平时读本地缓存，瞬间打开。
- 详情页「社区」标签：显示这个项目在 **Hacker News** 上被讨论过的帖子，以及 **GitHub 上评论最多的 Issues**（别人遇到什么问题），带分数、评论数、链接。同样先读缓存，点「刷新」才联网。

这张卡**只做后台**：联网层、两个缓存表、三个命令。界面另有卡。

## 背景与先读什么
- 契约（只读，**不许改**）：`apps/desktop/src/lib/contracts.ts` 末尾的 `DiscoverScenario`、`DiscoverCandidate`、`DiscoverView`、`DiscoverRefreshReport`、`CommunityItem`、`RepoCommunity`。Rust 端用 `serde(rename_all = "camelCase")`，键集合要和契约完全一致（`Option` 为 `None` 时也写出 `null` 键，不要 `skip_serializing_if`）。
- 联网的现有写法：`apps/desktop/src-tauri/src/rankings_live.rs` 的 `http_get`（reqwest 失败时用系统 `curl` 兜底——本机访问 GitHub 常见 TLS 握手失败，所以**新代码直接用系统 `curl`**，不用 reqwest）。
- 场景与归类：`apps/desktop/src-tauri/src/scenarios.rs`（`scenarios` 表、`list_scenarios_summary` 的口径：`count`、`thin`）；Fox 已收藏的仓库在 `repositories`（`sync_status='active'`）。
- 收集箱：`apps/desktop/src-tauri/src/inbox_view.rs` 与 `capture.rs`（收集箱条目怎么记链接、怎么判断「已在收集箱」；`inbox_items` 的状态：`archived` 不算在收集箱里）。
- 命令怎么写成异步、阻塞的活怎么放线程：看 `lib.rs` 里 `fetch_repository_readmes` / `sync_github_stars` 的写法。**联网命令必须是 `async fn`，阻塞的 curl 调用放进 `tauri::async_runtime::spawn_blocking`**，否则会卡住整个界面。
- 迁移规则：`CLAUDE.md`「Schema 与迁移」一节（只追加、登记到 `db/migrations.rs` 的 `MIGRATIONS`、补测试；下一个编号是 031）。
- 测试怎么造库：`db/migrations.rs` 的 `empty_database_applies_every_migration`；`scenarios.rs` 的 `temp_db` / `ready` / `insert_repo` 写法。**所有测试只用临时库和假的联网器，不联网、不读钥匙串、不读写真实数据目录。**

## 要做的事

### 1. 迁移 `packages/storage/migrations/fox/031_web_content.sql`（并在 `db/migrations.rs` 登记、补测试）
```sql
CREATE TABLE IF NOT EXISTS discover_candidates (
  scenario_id TEXT NOT NULL,
  full_name TEXT NOT NULL,
  html_url TEXT NOT NULL,
  description TEXT,
  language TEXT,
  stars_count INTEGER NOT NULL DEFAULT 0,
  pushed_at TEXT,
  topics_json TEXT NOT NULL DEFAULT '[]',
  fetched_at TEXT NOT NULL,
  PRIMARY KEY (scenario_id, full_name)
);
CREATE INDEX IF NOT EXISTS idx_discover_candidates_scenario ON discover_candidates(scenario_id, stars_count DESC);

CREATE TABLE IF NOT EXISTS discover_runs (
  scenario_id TEXT PRIMARY KEY,
  last_fetched_at TEXT NOT NULL,
  queries INTEGER NOT NULL DEFAULT 0,
  failed_queries INTEGER NOT NULL DEFAULT 0,
  message TEXT
);

CREATE TABLE IF NOT EXISTS repo_community_cache (
  repo_id TEXT NOT NULL,
  source TEXT NOT NULL CHECK (source IN ('hn', 'github_issue')),
  payload_json TEXT NOT NULL,
  fetched_at TEXT NOT NULL,
  error TEXT,
  PRIMARY KEY (repo_id, source)
);
```
`migrations.rs` 里已有「最新版本」「迁移个数」之类的断言，一并更新；新增一个测试：空库迁移后这三张表存在、再迁移一次幂等。**不许改 001–030 的任何文件。**

### 2. `web_fetch.rs`：联网层
- `trait HttpFetcher { fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<HttpResponse, FetchError>; }`；`HttpResponse { status: u16, headers: Vec<(String, String)>, body: String }`（头名小写）；`FetchError { kind: Timeout | Network | Other, message }`。
- `CurlFetcher`：用 `std::process::Command::new("curl")`，**参数逐个传（不经过 shell）**：`-sS -L --max-time 20 --connect-timeout 8 --max-filesize 2000000 -A "fox-stars-lab" -D <临时头文件或 -i 方式> ` 加调用方给的头和网址。要同时拿到状态码、响应头、正文（例如 `-i` 后自己切开，或 `-D -`）；跟随重定向时只取最后一段响应的状态和头。curl 退出码为 28 → `Timeout`，6/7/35 等 → `Network`。**错误信息里不许带完整命令行或请求头。**
- 辅助：`url_encode(&str)`（只放行字母数字和 `-_.~`，其余按 UTF-8 字节百分号编码）、`rate_limit_wait_secs(&HttpResponse, now_epoch) -> Option<u64>`（状态 403 或 429 且 `x-ratelimit-remaining: 0` 时，用 `x-ratelimit-reset`（epoch 秒）减 `now`；没有该头但有 `retry-after` 时用它；否则 `None`）。
- 不读钥匙串、不带任何令牌（这一版用 GitHub 的免登录搜索，一分钟 10 次）。

### 3. `discover.rs`：发现
- 搜索词配置放 `docs/rebuild/spec/discover_queries.json`（`include_str!` 进二进制）：`{ "version": 1, "since_days": 120, "per_scenario_limit": 20, "scenarios": { "<slug>": ["<搜索词>", ...] } }`，搜索词里 `{since}` 在运行时换成「今天往前 `since_days` 天」的 `YYYY-MM-DD`。**13 个场景（xhs、wx、dy、img、vid、ppt、wr、res、data、kb、auto、ec、seo）每个 2 条**，内容如下（原样写进文件）：
  - xhs：`xiaohongshu OR 小红书 in:name,description stars:>100 pushed:>{since}`；`rednote OR 小红书 skill OR mcp OR agent in:description stars:>50 pushed:>{since}`
  - wx：`微信公众号 in:description stars:>100 pushed:>{since}`；`wechat article OR 公众号 排版 OR 发布 in:description stars:>50 pushed:>{since}`
  - dy：`douyin OR 抖音 in:name,description stars:>100 pushed:>{since}`；`短视频 OR short video generator in:description stars:>200 pushed:>{since}`
  - img：`poster OR 海报 OR 封面 in:description stars:>100 pushed:>{since}`；`image generation skill OR cover generator in:description stars:>100 pushed:>{since}`
  - vid：`subtitle OR 字幕 OR whisper in:description stars:>200 pushed:>{since}`；`video editing agent OR 视频剪辑 in:description stars:>100 pushed:>{since}`
  - ppt：`pptx OR powerpoint OR slides skill in:description stars:>100 pushed:>{since}`；`markdown to ppt OR 幻灯片 in:description stars:>100 pushed:>{since}`
  - wr：`copywriting OR 文案 skill in:description stars:>100 pushed:>{since}`；`ai writing assistant 中文 in:description stars:>100 pushed:>{since}`
  - res：`web scraping OR crawler OR 爬虫 in:description stars:>300 pushed:>{since}`；`deep research agent in:description stars:>200 pushed:>{since}`
  - data：`data analysis agent OR 数据分析 in:description stars:>200 pushed:>{since}`；`dashboard OR chart ai in:description stars:>200 pushed:>{since}`
  - kb：`knowledge base OR 知识库 OR second brain in:description stars:>200 pushed:>{since}`；`obsidian plugin ai in:description stars:>100 pushed:>{since}`
  - auto：`workflow automation agent in:description stars:>300 pushed:>{since}`；`mcp server in:name,description stars:>300 pushed:>{since}`
  - ec：`shopify OR 电商 OR ecommerce in:description agent OR mcp stars:>100 pushed:>{since}`；`product listing OR 详情页 OR 商品图 in:description stars:>50 pushed:>{since}`
  - seo：`seo in:name,description agent OR skills stars:>100 pushed:>{since}`；`marketing skills OR growth in:description stars:>100 pushed:>{since}`
- `refresh_discover_with(fetcher, db_path, scenario_id, now) -> Result<DiscoverRefreshReport, String>`：对这个场景的每条搜索词调 `https://api.github.com/search/repositories?q=<url_encode(词)>&sort=stars&order=desc&per_page=15`（头：`Accept: application/vnd.github+json`、`X-GitHub-Api-Version: 2022-11-28`）；解析 `items[]`（`full_name`、`html_url`、`description`、`language`、`stargazers_count`、`pushed_at`、`topics`、`archived`、`fork`、`disabled`）；**丢掉 archived / fork / disabled**；按 `full_name` 小写去重；按星数从高到低取 `per_scenario_limit` 个；整批一个事务**替换**这个场景的缓存（先删这个场景旧行再写），并写 `discover_runs`；已在 Fox 的 Stars 里的候选也写缓存（读取时再标 `mine`），`skippedMine` 记个数。某条搜索词失败不影响别的（`failedQueries` 加一）；全部失败不要清掉旧缓存，返回 `Ok` 并在 `message` 写人话（「联网失败，保留上次的结果」）。遇到限流：停止后面的搜索词，已拿到的部分照样写入，`rateLimitResetAt`（ISO 时间）和 `message`（「GitHub 搜索太频繁了，约 N 秒后再试」）填上。场景 id 不存在 → 返回中文错误「找不到这个场景」。
- `list_discover_view(db_path, scenario_id: Option<&str>) -> Result<DiscoverView, String>`：只读本地。`scenarios` 含所有未归档场景，按 `sortOrder`；`myCount`/`thin` 与 `list_scenarios_summary` 同口径；`candidateCount` 不含 `mine`；`selected`：传了合法 id 就用它，否则取「偏少」里 `sortOrder` 最小的，都不偏少就取 `sortOrder` 最小的；`candidates` 是选中场景的缓存，按星数从高到低；`mine` = 在 `repositories`（`sync_status='active'`）里有同名（不分大小写）；`inInbox` = 收集箱里有这个链接且状态不是 `archived`（自己看 `inbox_view.rs` 怎么判断，别另造一套）；`topics` 解析 `topics_json`，坏 JSON 当空数组。
- 命令（在 `lib.rs`）：`list_discover(scenario_id: Option<String>)`（同步即可，只读）；`refresh_discover(scenario_id: String)`（**async + spawn_blocking**，用 `CurlFetcher`）。

### 4. `community.rs`：社区
- `HnFetch`：`https://hn.algolia.com/api/v1/search?query=<url_encode("github.com/<owner>/<repo>")>&restrictSearchableAttributes=url&tags=story&hitsPerPage=5`；解析 `hits[]`（`title`、`objectID`、`points`、`num_comments`、`created_at`、`url`）；链接用 `https://news.ycombinator.com/item?id=<objectID>`；`title` 为空的丢掉；`state` 为 `None`。
- `IssuesFetch`：`https://api.github.com/search/issues?q=<url_encode("repo:<owner>/<repo> is:issue")>&sort=comments&order=desc&per_page=5`（头同上）；解析 `items[]`（`title`、`html_url`、`comments`、`created_at`、`state`）；`points` 为 `None`。
- `get_repo_community_with(fetcher, db_path, repo_id, refresh, now)`：仓库不存在 → 中文错误「找不到这个仓库」。`refresh=false`：只读缓存（两个来源各一行），不联网；任一来源没有缓存行或 `fetched_at` 早于 24 小时 → `stale=true`。`refresh=true`：两个来源**各自**联网、各自写缓存行（成功写 `payload_json`，失败写 `error` 并保留旧的 `payload_json`），一个失败不影响另一个；失败信息进返回的 `errors`（人话，限流时带等待秒数）。`items` 按「HN 在前，各自按分数 / 评论数从高到低」合并；`fetchedAt` 取两个来源里较早的一个，都没有为 `None`。
- 命令：`get_repo_community(id: String, refresh: Option<bool>)`——`refresh` 为真时 **async + spawn_blocking**；为了简单，整个命令写成 `async fn`，不联网时也走同一条路。

### 5. 注册
`lib.rs` 加 `mod web_fetch; mod discover; mod community;`，三个命令登记到 `generate_handler!` 和 `permissions/gsat-commands.toml`；`gen/schemas/acl-manifests.json` 是生成的单行文件，跑一次 `tools/cards/run.sh rust-check` 让它重新生成，不要手改。

## 不要做
- 不改契约 `contracts.ts`、不改前端、不改 `scenarios.rs`、`inbox_view.rs`、`capture.rs`、`rankings_live.rs`（只读它们）；不新增依赖；不读钥匙串；不带任何令牌；测试里不联网。
- 不要启动 App，不要读写任何数据目录。

## 验收
1. `tools/cards/run.sh rust-test` 全部通过，新增测试**至少 28 个**，覆盖：迁移（表存在、幂等）；`url_encode`（中文、空格、`+`、`:`）；`rate_limit_wait_secs`（403 + remaining 0、429 + retry-after、没有限流）；`CurlFetcher` 的参数拼装（把「拼参数」写成纯函数测试，不真的执行 curl）；GitHub 搜索响应解析（正常、缺字段、archived/fork/disabled 被丢、去重、取前 N、`topics` 缺失）；`{since}` 替换（用注入的 `now`）；`refresh_discover_with`：成功写缓存并替换旧行、某条失败其余照写、全部失败保留旧缓存、限流时停止并写入已得部分、场景 id 不存在；`list_discover_view`：默认选中规则（偏少优先、都不偏少取第一个、传了合法 id）、`mine` 不分大小写、`inInbox`（待处理算、已归档不算）、`candidateCount` 不含 `mine`、坏 `topics_json`；社区：HN 解析、Issues 解析、`refresh=false` 不调用联网器（用会 panic 的假联网器证明）、stale 的各种情形（无缓存、过期、新鲜）、一个来源失败另一个仍写入且旧数据保留、`items` 排序与 `fetchedAt`；所有返回值的**契约键集合测试**（`DiscoverView`、`DiscoverScenario`、`DiscoverCandidate`、`DiscoverRefreshReport`、`RepoCommunity`、`CommunityItem`，含 `null` 键）。
2. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多（4 条）。
3. 给出：3 个命令在 `generate_handler!` 和 `gsat-commands.toml` 里的行号；`CurlFetcher` 参数拼装的位置；证明 `refresh_*` 命令是 `async` 且阻塞调用在 `spawn_blocking` 里（文件:行）；证明没有读钥匙串、没有带令牌（`grep -n "keychain\|Authorization\|token" web_fetch.rs discover.rs community.rs` 的输出，应为空或只是注释）。
4. `git diff --stat` 里只出现「允许改动」里的路径；`001`–`030` 迁移文件没有改动。
5. 报告里写清：哪些逻辑因为不能联网而没有真实验证，以及用什么假数据覆盖；GitHub 搜索词里 `OR`、中文是否按文档语法写对（你不能联网查，只写明「按 GitHub 搜索语法写的，需总控联网验证」）。

## 停止条件
- 要改契约、前端或卡外文件；要新依赖；发现 `inbox_view.rs` 没法判断「已在收集箱」——停手并写进报告。
