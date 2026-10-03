---
编号: 22-local-api-capture
允许改动:
  - apps/desktop/src-tauri/src/local_api.rs
  - apps/desktop/src-tauri/src/v2_ops.rs
  - apps/desktop/src-tauri/src/lib.rs
  - apps/desktop/src-tauri/permissions/gsat-commands.toml
  - apps/desktop/src-tauri/gen/schemas/acl-manifests.json
  - docs/rebuild/微信联动接入.md
---

# 卡 22 · 微信联动的接收端：本地 API 的 `POST /v1/capture` + 「收集专用令牌」

## 为什么有这张卡
Fox 定的三个新功能之一是「微信联动」：群聊里别人分享的 GitHub 项目、X 帖子、公众号文章，由 Fox 的**微信工作台**（另一个项目，在本机另一个进程里跑）自动送进 Stars Lab 的收集箱，不用他手动复制粘贴。接收端就是本地 API 的 `POST /v1/capture`（设计见 `docs/rebuild/产品重设计与开发说明书.md` 第 541、555、934–935 行）。
现在本地 API 只有 `GET /v1/health`、`GET /v1/search`、`POST /v1/usage`。这张卡补上 `/v1/capture`，并加一个**收集专用令牌**：微信工作台只拿这个令牌，**它只能往收集箱里放链接，读不到 Fox 的资料库**（最小权限）。

## Fox 能看到的效果
- 工具箱里（界面总控另做）能复制「收集专用令牌」、可以随时重置；微信工作台配上它之后，群聊里出现的链接会进入收集箱，带着「来自微信 · 群名 · 发送人」。
- 同一条消息重复发来不会重复收；已经在 Stars 里的 GitHub 项目不会再进收集箱。

## 背景与先读什么
- `local_api.rs`（整个文件；**安全模型写在文件头注释里，不许削弱**）：只监听 127.0.0.1、校验 `Host`、**带 `Origin` 头的请求一律拒绝**（网页脚本发起的请求都会带）、响应不带 CORS 头、令牌 256 位随机存在 0600 权限的控制文件里（`fox-stars-lab.local-api.json`，`read_control`/`write_control`/`generate_token`/`rotate_token`/`token`）、`route()` 里的路由表。**这张卡不做浏览器书签小工具，所以不开 CORS，`Origin` 规则一点都不改。**
- `v2_ops::commit_capture`（`v2_ops.rs` 约 621 行）：粘贴一段文字 → 抽链接 → 写 `capture_batches` 和 `inbox_items`；`capture_batches.source_platform` 现在写死 `'other'`，`inbox_items.captured_via` 写死 `'paste_batch'`。表结构见 `packages/storage/migrations/fox/021_capture_sources.sql`：`source_platform` 允许 `web, github, x, wechat, telegram, discord, forum, xiaohongshu, bilibili, youtube, local, other`；`captured_via` 允许 `manual, paste_batch, clipboard, bookmarklet, local_api, mcp, share_extension, folder_watch`；`raw_excerpt` 最多 8000 字符（**这张卡用它存「原话」**）。
- `lib.rs` 里 `local_api` 相关的命令（搜 `local_api::`）：怎么开关、怎么拿令牌、怎么重置——**照着做一套「收集专用令牌」的**。命令注册：`generate_handler!`、`permissions/gsat-commands.toml`、`gen/schemas/acl-manifests.json`（改了命令后跑一次 `rust-check` 会重新生成 `acl-manifests.json`，把生成的结果一起留下）。

## 要做的事

### 1. `v2_ops.rs`：把收集写入泛化（旧入口行为不变）
- 新增 `pub struct CaptureSource<'a> { pub platform: &'a str, pub via: &'a str, pub original_text: Option<&'a str> }` 和 `pub fn commit_capture_with(path, text, source_context, source: &CaptureSource) -> Result<CaptureCommit, String>`。
- 旧的 `commit_capture(path, text, source_context)` 改成调用新函数，传 `platform = "other"`、`via = "paste_batch"`、`original_text = None`——**旧行为（含现有全部测试）一个字都不能变**。
- `platform` 不在上面允许的列表里就按 `'other'` 写（不报错）；`via` 只许 `paste_batch` 或 `local_api`，别的当 `paste_batch`。
- `capture_batches.source_platform` 写传入的 `platform`；`inbox_items.captured_via` 写传入的 `via`；`inbox_items.source_platform` 仍然写**链接本身所在的平台**（`link.platform`，现状不变）。
- `original_text` 有值时，每个新建条目的 `raw_excerpt` = 它（去掉首尾空白，最多 2000 个字符，按字符截）。`None` 时不写（现状）。
- 批次去重键仍然是正文的哈希；其它逻辑不变。

### 2. `local_api.rs`：收集专用令牌 + `POST /v1/capture`
1. 控制文件新增键 `captureToken`（同样 256 位随机、同样的 0600 写法，**和 `token` 互相独立**）。新函数：`capture_token(db_path) -> Option<String>`、`ensure_capture_token(db_path) -> Result<String, String>`（没有就生成并写入）、`rotate_capture_token(db_path) -> Result<String, String>`。**重置其中一个令牌不影响另一个，也不丢控制文件里别的键**（现有 `rotate_token` 就是这么做的，照做）。
2. 认证：`/v1/capture` 接受 `Authorization: Bearer <完整令牌>` **或** `Bearer <收集专用令牌>`；**其它所有路由只认完整令牌**。用收集专用令牌访问别的路由 → `403`，消息「收集专用令牌只能用于 /v1/capture」；令牌错误或缺失 → `401`（沿用现有消息）。令牌比较继续用 `constant_time_eq`。控制文件里没有 `captureToken` 时，收集专用令牌一律不被接受（不要因为两个都是空就放行）。
3. 请求：`POST /v1/capture`，`Content-Type: application/json`，正文：
   ```json
   { "text": "群里原话……", "urls": ["https://github.com/a/b"], "sourcePlatform": "wechat", "sourceContext": "群：小红书运营交流 · 张三 · 2026-10-04 09:30" }
   ```
   - `text`、`urls` 至少有一个非空，否则 `400`「没有可收集的内容」；`urls` 最多 100 条、每条最多 2048 字符，超出 `400`；`text` 最多 50000 字符，超出 `400`；`sourceContext` 最多 2000 字符（超出按字符截断，不报错）；字段类型不对 `400`；缺 `application/json` → `415`（沿用 `/v1/usage` 的写法）。
   - 把 `urls`（每条一行）和 `text` 合成一段文字交给 `commit_capture_with`：合成顺序 = `urls` 在前、`text` 在后，中间空一行；`CaptureSource { platform: sourcePlatform 或 "other", via: "local_api", original_text: text 非空时给 text，否则 None }`。
   - 响应 `200`：`CaptureCommit` 的 JSON（`created`、`skipped`、`duplicateBatch`、`alreadyStarred`、`alreadyInInbox`、`blocked`、`linksFound`）外加 `"ok": true`。**没找到链接也是 `200`**（`linksFound: 0`），不是错误。写库失败 `500` 且不泄露内部细节。
4. 控制文件的读取/写入、`Host`/`Origin` 校验、请求大小限制（`MAX_BODY_BYTES` 64 KB）都沿用，不要放宽。**日志、错误消息、任何返回值里都不许出现令牌。**

### 3. `lib.rs`：给工具箱用的命令（界面总控另做）
- `get_local_api_capture_token(app_handle) -> Result<String, String>`：`ensure_capture_token` 后返回；`rotate_local_api_capture_token(app_handle) -> Result<String, String>`。命令的写法、权限、命名风格照现有的本地 API 命令。**如果现有的「完整令牌」命令不是这样暴露给前端的，就照它现在的做法，并在报告里说明。**

### 4. 文档 `docs/rebuild/微信联动接入.md`（写给微信工作台的开发者，不是写给 Fox）
- 用中文，**一页纸以内**：怎么开（工具箱里开启本地 API）、地址（`http://127.0.0.1:39217`）、令牌从哪来（Fox 在工具箱复制「收集专用令牌」，粘进微信工作台的设置里；**不要让工作台去读 Stars Lab 的数据目录**）、请求/响应示例（`curl`）、字段说明、错误码表、幂等说明（同一段文字重发返回 `duplicateBatch: true`）、建议（一次请求一条消息；只发含链接的消息；`sourceContext` 写成「群名 · 发送人 · 时间」；不要发聊天全文，也不要发手机号等隐私）。
- 写明：这个接口**只在本机、默认关闭**；令牌泄露了就在工具箱点「重置」。

## 不要做
- 不做浏览器书签小工具、不开 CORS、不放宽 `Origin` 规则、不监听 127.0.0.1 以外的地址。
- 不改前端、`capture.rs`、`inbox_view.rs`、契约文档、卡外文件；不碰 AI、钥匙串。
- 不在本地 API 里加读取类的新接口（`/v1/items` 等不在这张卡里）。
- 不在代码、测试、文档里写任何真实令牌；测试用随机生成的临时令牌，临时目录里造库，**不碰真实的 HOME 和数据目录**。

## 验收
1. `tools/cards/run.sh rust-test` 全部通过，新增测试**至少 24 个**：
   - 认证矩阵：无令牌 401；错误令牌 401；完整令牌访问 `/v1/capture` 200；收集专用令牌访问 `/v1/capture` 200；收集专用令牌访问 `/v1/search`、`/v1/usage` → 403；完整令牌访问其它路由行为不变；控制文件没有 `captureToken` 时空 Bearer 不能通过；两个令牌分别重置后旧的失效、另一个不受影响；带 `Origin` 头 → 拒绝（与现状一致）；错误的 `Host` → 拒绝（与现状一致）。
   - 请求校验：缺 `Content-Type` 415；非 JSON 400；`text` 和 `urls` 都空 400；`urls` 101 条 400；单条 URL 2049 字符 400；`text` 50001 字符 400；`sourceContext` 超长被截断而不是报错；字段类型错误 400。
   - 行为：只传 `urls`；只传 `text`；两者都传（合成顺序）；`sourcePlatform` 合法值写进 `capture_batches.source_platform`、非法值变 `other`；条目的 `captured_via` 是 `local_api`；条目的 `source_platform` 仍是链接平台（GitHub 链接是 `github`）；`raw_excerpt` 有原话且被截到 2000 字符；同一请求重发 `duplicateBatch: true`；已 Star 的仓库计入 `alreadyStarred`；内网地址计入 `blocked`；没有链接 200 且 `linksFound: 0`；响应 JSON 键集合。
   - 旧入口：`commit_capture` 旧测试全部原样通过，且新增一个断言它仍写 `paste_batch` / `other`。
   - 控制文件：新生成的文件权限是 0600；写 `captureToken` 不丢 `enabled`、`token` 等已有键。
2. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多。
3. `git diff --stat` 里只出现「允许改动」里的路径。
4. 报告里给出：路由表里认证分支的文件:行号；证明收集专用令牌访问非 `/v1/capture` 路由返回 403 的测试名；命令在 `generate_handler!` 和 `gsat-commands.toml` 里的行号。

## 停止条件
- 要放宽本地 API 的任何安全规则、要改卡外文件、要新依赖——停手并写进报告。
