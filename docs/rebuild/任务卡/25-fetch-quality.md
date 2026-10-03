---
编号: 25-fetch-quality
允许改动:
  - apps/desktop/src-tauri/src/inbox_fetch.rs
---

# 卡 25 · 收集箱读网页的质量：压缩、字符集、乱码保护、拦截页识别

## 为什么有这张卡
总控 2026-10-04 凌晨用真网络把常见中文站点的链接收进收集箱再读，发现三类「读成功了但内容是错的」：
1. **B 站视频页**：服务器**没被要求压缩也照样返回 `content-encoding: gzip`**（连 `Accept-Encoding: identity` 都无视）。现在的 `decode_body` 直接 `String::from_utf8_lossy` 原始字节，于是标题是网址、摘要是一串二进制乱码（`���G�/���j�}6j…`），状态还写成「读完了」。
2. **36kr**：返回的是「火山引擎 正在进行安全检测…请耐心等待」的人机验证中间页，没有 `<title>`，现在被当成读成功，这句话成了摘要。
3. **非 UTF-8 的中文页面**（GBK / GB2312 / Big5，老站、政府站、部分门户）：同一个 `from_utf8_lossy` 会把正文变成乱码。

Fox 的收集主要来自微信群，里面 B 站、小红书、抖音、门户站的链接很多。「读成功」的乱码比「读失败」更糟——失败有一句人话原因和重试，乱码会让人以为这个工具坏了。

## Fox 能看到的效果
- B 站视频链接收进来，几秒后有标题（视频名）和一段简介，而不是乱码。
- 被人机验证拦住的网站，那一条显示红字「网站要求先做人机验证，这里读不了」，而不是把「正在进行安全检测」当摘要。
- GBK 编码的老网站标题和摘要是正常中文。

## 背景与先读什么
- `inbox_fetch.rs`（卡 23 / 24，已合并）：
  - `curl_once_pinned`（约第 520 行）拼 curl 参数（`safe_curl_arguments`）并取回字节，最后 `decode_body(&output.stdout)` 转字符串——**要改的就是这里**。响应头已经被写进 `-D` 文件并解析成 `Vec<(String, String)>`（`parse_header_dump`、`header_value`）。
  - `PageResponse { status, content_type, body: String, final_url, location }` 保持不变（别的代码和测试都用它）。
  - `accept_body`、`allowed_type`、`too_big_problem()`、`FetchProblem::{skip, fail}`、`MAX_BODY_BYTES`（压缩后字节数上限，curl 的 `--max-filesize` 管它）。
  - `fetch_html` → `extract_web`（返回 `WebPage { title, author, excerpt, failure }`，`failure` 目前有公众号被拦、空页面两种）。
- **依赖已经加好**（总控已在 `Cargo.toml` 加了 `flate2 = "1"` 和 `encoding_rs = "0.8"`，两者本来就在依赖树里、没有新包）。**你不要改 `Cargo.toml` / `Cargo.lock`。**
- 测试里**不许联网**。测试夹具里的压缩数据用 `flate2::write::GzEncoder` / `ZlibEncoder` / `DeflateEncoder` 现场压出来；GBK 字节用 `encoding_rs::GBK.encode("中文").0` 现场编码。

## 要做的事（都在 `inbox_fetch.rs`，纯函数 + 单测）

### 1. `fn decode_page(bytes: &[u8], headers: &[(String, String)]) -> Result<String, FetchProblem>`
`curl_once_pinned` 里用它代替 `decode_body`（`decode_body` 可以删掉，或只留给测试）。顺序：
1. **解压**：看 `Content-Encoding`（不分大小写，可能有多个值用逗号隔开——只处理单个值；多个值一律当不支持）：
   - 无 / `identity`：不解压。**但**如果没有 `Content-Encoding` 头而字节以 `1f 8b` 开头，当 gzip 解（有的服务器忘了写头）。
   - `gzip` / `x-gzip`：`flate2::read::GzDecoder`。
   - `deflate`：先按 zlib 包装解（`ZlibDecoder`），开头就失败再按裸 deflate 解（`DeflateDecoder`）。
   - 其他（`br`、`zstd`、`compress`…）：`FetchProblem::fail("网站返回了这里读不了的压缩格式")`。
   - 解压后大小上限 `MAX_DECODED_BYTES = 16_000_000`：用 `Read::take(MAX_DECODED_BYTES + 1)` 读，超过就返回 `too_big_problem()`（防压缩炸弹，**绝不能先全部解出来再比较**）。数据损坏 → `FetchProblem::fail("页面压缩数据损坏")`。
2. **字符集**，优先级：BOM（UTF-8 / UTF-16LE / UTF-16BE）→ `Content-Type` 头里的 `charset=`（可能带引号）→ 前 4096 字节里的 `<meta charset=…>` 或 `<meta http-equiv="Content-Type" content="…charset=…">`（把这 4096 字节当 ASCII 不分大小写地找）→ 默认 UTF-8。用 `encoding_rs::Encoding::for_label` 找编码，找不到就 UTF-8；`decode` 把非法字节换成 U+FFFD。`gb2312` / `gbk` 由 encoding_rs 自己映射。
3. **乱码保护**：取解码后文字的前 2000 个字符，若 U+FFFD 或「除换行 / 回车 / 制表以外的控制字符」占比超过 20%，返回 `FetchProblem::fail("页面内容读不出来")`。

### 2. curl 加一个请求头
`safe_curl_arguments` 加 `-H "Accept-Encoding: gzip, deflate"`（只声明这两种，免得服务器回 `br` / `zstd`；也让大页面少传几倍字节）。同步改函数上方那句「不带 `-L`、cookie、`Authorization` 或任何请求头」的注释（现在是「只带这一个请求头」）。**仍然不许带 `-L`、cookie、`Authorization`。** 已有的参数测试要跟着改并补一条断言。

### 3. 拦截页识别
`fn challenge_reason(title: &str, text: &str) -> Option<&'static str>`：`text` 是页面可见正文（`extract_web` 里的 `body`，或 `excerpt` 也行，你选一个并在报告里说明）。
- 条件：**可见正文少于 600 个字符**，并且标题或正文（ASCII 部分不分大小写）包含下面任一短语：
  `正在进行安全检测`、`安全检测`、`人机验证`、`请完成验证`、`滑动验证`、`访问验证`、`检测到异常流量`、`请开启 javascript`、`请启用 javascript`、`just a moment`、`checking your browser`、`verify you are human`、`attention required`、`enable javascript and cookies`、`are you a robot`、`captcha`。
- 命中返回 `Some("网站要求先做人机验证，这里读不了")`；`extract_web` 把它放进 `WebPage.failure`（和公众号被拦同一个通道），于是 `fetch_html` 返回 `Outcome::Failed`。
- **不能误伤**：一篇很长的正常文章里提到「安全检测」「验证码」不能算（所以才有 600 字符的条件）；标题正常、正文很短但没有这些短语的页面照常读成功。

## 不要做
- 不改 `PageResponse`、`PageFetcher`、`fetch_guarded`、`gate_request`、`ip_is_blocked`、重定向与固定 IP 的逻辑；不改 `capture.rs`、`web_fetch.rs`（卡 17 的 curl 路径）。
- 不用 `curl --compressed`（它不限制解压后的大小，有压缩炸弹风险）；不新增依赖、不改 `Cargo.toml`。
- 不在测试里联网；不改前端。

## 验收
1. `tools/cards/run.sh rust-test` 全部通过，新增测试**至少 14 个**：
   - 解压：gzip 中文页面解对；`x-gzip` 和大小写不同的头名；没有头但以 `1f 8b` 开头的当 gzip；deflate（zlib 与裸 deflate 各一）；`br` / `zstd` 返回「读不了的压缩格式」；损坏的 gzip 返回「页面压缩数据损坏」；**压缩炸弹**（把 20 MB 的零压成 gzip）返回 `too_big_problem()` 且测试几秒内跑完、不会吃掉大量内存；多个编码值（`gzip, br`）当不支持。
   - 字符集：`Content-Type: text/html; charset=gbk` 的 GBK 页面；只有 `<meta charset="gb2312">`；`<meta http-equiv="Content-Type" content="text/html; charset=GBK">`；UTF-8 BOM；UTF-16LE BOM；`charset=foo`（未知）退回 UTF-8。
   - 乱码保护：一段随机二进制（比如 0x80–0xFF 的字节循环）返回「页面内容读不出来」；正常中文页不误伤。
   - 拦截页：36kr 那段「火山引擎 正在进行安全检测…」（没有 `<title>`）→ `Outcome::Failed`；Cloudflare 风格的 `<title>Just a moment...</title>` → Failed；超过 600 字的正常文章里夹一句「验证码」→ Done；标题正常的短页面 → Done。
   - `safe_curl_arguments` 含 `Accept-Encoding: gzip, deflate` 且仍然没有 `-L`、`Cookie`、`Authorization`。
2. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比开工前多（先跑一次 `rust-check` 记下基线，应是 4 条）。
3. `git diff --stat` 里只出现 `apps/desktop/src-tauri/src/inbox_fetch.rs`。
4. 报告里给出：`decode_page` 和 `challenge_reason` 的文件:行号；压缩炸弹那条测试的断言和耗时；你选用 `body` 还是 `excerpt` 做拦截页判断以及原因。

## 停止条件
- 需要改 `Cargo.toml` / 新依赖、要改 `inbox_fetch.rs` 以外的文件、或 `flate2` / `encoding_rs` 的 API 在这个版本里做不到上面某一条——停手并写进报告，不要绕。
