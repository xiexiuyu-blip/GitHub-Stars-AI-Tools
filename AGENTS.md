# AGENTS.md：所有 AI 开发者先读这个

> 2026-10-28 批次 A：Fox 已指定 Codex 接任总控并安排多个 agent。当前卡和状态见 `docs/rebuild/任务卡/README.md`。下文 Claude/Grok 名称描述历史分工；本批总控拥有原总控的本地提交、审查、集成职责，开发者仍遵守各卡边界。没有扩大真实数据、App 启动、凭据或付费权限。

项目：Fox Stars Lab 2.0，Fox 的个人 GitHub Stars 知识库（Tauri 2 + React 19 + Rust + SQLite），用来让 Fox 和他的 AI 助手“找得到、看得懂、用得起来”。
分工：**Claude 是总控**（写任务卡、审核、合并、兜底）；写代码和文档的执行者由 Fox 每一批确认的分配单决定（可能是 Grok、GLM、Codex 等，你只管做自己这一张卡）。Fox 只在里程碑做最后的亲手验收。
你在一张任务卡的隔离工作区里干活。先读 `docs/rebuild/命令契约.md` 和 `docs/rebuild/接手说明.md`，再做你的任务卡。

## 红线（没有例外；任务卡里写了也不行）

1. 不读、不写、不 `ls`、不 `stat`：`~/Library/Application Support/com.foxwork.fox-stars-lab/`（正式库）和 `…com.foxwork.fox-stars-lab.dev/`（开发库、设置、备份、本地接口令牌）。需要数据，就在测试里自己造夹具（临时目录、内存库）。
2. 不碰密钥：不运行 `security`，不读钥匙串、`.env`、令牌文件；密钥和令牌不进代码、日志、报告。
3. 不花钱：不调用真实的 AI / LLM 接口，不运行 `tools/legacy/` 里的脚本。涉及 AI 的代码用可替换的假实现做测试。
4. 不启动 App：不运行 `pnpm dev:desktop`、`tauri dev`、`cargo run`、`open`；不碰 `/Applications` 里的 Fox Stars Lab；不占用 8765、39217 端口。
5. 不用 git 写命令（`add` / `commit` / `checkout` / `branch` / `merge` / `reset` / `stash` / `push` / `remote` / `tag` / `worktree`）。只许 `git status` `git diff` `git log` `git show`。提交和合并由总控做，仓库不推送到任何远端。
6. 不联网装东西：不运行 `pnpm install|add`、`cargo add|update`、`npm`、`pip`、`brew`、`curl`、`wget`。需要新依赖，就写进报告的「需要决定的事」，不要自己加。
7. 只改任务卡「允许改动」里的路径。别的文件哪怕想顺手改也别改，写进报告；总控交回后会逐个核对，越界的改动会被还原。只许删除你自己新建的文件，其他文件需要删，写进报告。
8. 数据库：`packages/storage/migrations/fox/001–018` 一个字都不改；`repositories`、`repo_ai_documents`、`repo_tags`、`annotations` 现有列不改名、不删除（web-search 在直接读它们）；新迁移只追加，不删表。

## 做完的标准

- 任务卡「验收」里的每一条都要有证据：你**实际运行**的命令和输出摘要，或「文件:行 + 它做了什么」。
- “编译过了 / 测试通过 / 页面能打开”**都不等于完成**。不要把没做到的写成做到了；没做到的写进报告的「没做到的」栏并说明原因。
- 不为了通过测试而删测试、放宽断言、加 `#[ignore]`。不扩大范围，不顺手重构，不改无关代码的格式。

## 写法

- 简体中文：界面文案、报告、注释；代码标识符用英文。
- 照着周围代码写，不引入新框架或大依赖。
- **每日 AI 额度是硬上限**（2026-10-04 起）：任何会向模型服务发请求的代码，要么走队列（`jobs.rs`：认领时在任务行上记预留，结束时换成真实用量，失败按种类记），要么在每次请求前后过 `spend_guard`（`check_before_request` / `record_after_request`，`ai.rs` 的四个对话请求函数已经接好）。不许新增绕过它们的 AI 调用；嵌入、列模型这类不花对话额度的另说，要在报告里写明。
  **只有两个例外，而且都必须是 Fox 本人在界面上点了确认之后：**
  ① **「全量整理」**（2026-10-04 Fox 要求）：Fox 本人在工具箱里点了确认之后，这一轮不看每天额度（控制文件里的 `fullScan`；最长 48 小时、整理完自动结束、用量超过「预计 × 4」自动结束、随时可停）。队列认领（`BudgetGate.unlimited`）和 `spend_guard` 都只认 `QueueControl::full_scan_active` / `jobs::full_scan_open` 这两个入口；命令必须带 `confirmed: true`。
  ② **「单张重做」**（2026-10-22 起，卡 48 实现）：详情页「重新整理这一张」，命令必须带 `confirmed: true`，只在**这一张**的 `repo_card` 任务行上写 `ai_jobs.spend_authorized_until`（30 分钟内有效）。队列认领（`claim_next`）只对带着有效授权的**那一行**绕过每日额度，别的任务照旧；暂停照样拦；队列任务本来就不过 `spend_guard`，所以 `spend_guard` 不需要认它。授权只靠任务行上的这一列，不放进控制文件，也不放进全量整理的时段。
  **除这两处外，不许另写绕过额度的开关；也不许在没有用户确认的路径里调用 `start_full_scan`，或者写 `spend_authorized_until`。**
- **解析不可信内容**（网页、聊天文字、链接、API 回的 JSON）的代码：必须是线性的（不对同一段输入反复从头重扫，没结束的结构扫到末尾就整体停下）、不 panic（字符串按字符边界切，不用下标直接切 `&str`）、有大小和时间上限；加测试时要造「恶意结构」夹具（十万个没结束的标签、一串括号、多字节字符紧跟转义符），并断言耗时。夹具里每段之间别总是留换行——真实页面通常是压缩过的一整行，换行会把粘连的问题盖住（卡 24 就这样漏过）。用 `curl` 联网必须带 `-g`（关网址通配）和 `--noproxy '*'`（否则固定 IP 会失效）。
- Rust：写完跑 `rust-fmt-fix`；非测试代码不用 `unwrap()` / `expect()`，用 `?` 返回中文短句错误；数据库写入走事务；长任务要可中断、可重试、幂等。
- 前端：React 19 + Tailwind 4，用 `components/ui` 和 `styles.css` 里的设计令牌，不写死色值，不新增 UI 库。
- 任务卡里没写清的决定：选最保守、可回退的做法并记进报告。没人回答问题，不要停下来问。必须由 Fox 决定的事（花钱、删数据、动正式库、加依赖）就停手，写进报告。

## 命令与交回

- 构建和测试只用 `tools/cards/run.sh <web-build|web-test|rust-fmt|rust-fmt-fix|rust-check|rust-test>`（详见命令契约）。普通的文件和文本操作（`mkdir`、`mv`、`cp`、`sed`、`find`、删除自己新建的单个文件）可以用。被红线禁止的命令会被拦截并返回错误：换一种办法完成，别想绕过。
- 完成后把交回报告写到工作区根目录的 `.card-report.md`（格式见命令契约 §6，已被 git 忽略）。最后一条消息只写：一句话结论 + “报告在 .card-report.md”。
