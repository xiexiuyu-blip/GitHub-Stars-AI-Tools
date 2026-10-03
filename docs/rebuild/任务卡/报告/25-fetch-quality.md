# 交回报告 · 卡 25-fetch-quality

## 一句话结论
做完了：收集箱读网页会解 gzip/deflate、按字符集解码，乱码和人机验证页改判失败，不再当成读成功。

## 验收逐条
| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| `tools/cards/run.sh rust-test` 全部通过，新增测试至少 14 个 | 是 | 全量：`773 passed; 0 failed; 5 ignored; 0 measured; 1 filtered out`（过滤的是 run.sh 跳过的钥匙串用例），`finished in 69.71s`。`inbox_fetch.rs` 新增 23 个 `#[test]`（约 4081–4319 行）。 |
| 解压：gzip 中文页；`x-gzip` 与头名大小写；无头但 `1f 8b` 当 gzip；deflate 的 zlib 与裸 deflate；`br` / `zstd` 返回「读不了的压缩格式」；损坏 gzip 返回「页面压缩数据损坏」；20 MB 零的 gzip 炸弹返回 `too_big_problem()` 且几秒内结束；`gzip, br` 当不支持 | 是 | `gzip_decodes_chinese_page`、`x_gzip_and_mixed_case_header_name_decode`、`gzip_magic_without_content_encoding_header_decodes`、`zlib_wrapped_deflate_decodes_chinese`、`raw_deflate_decodes_chinese`、`brotli_encoding_is_unsupported`、`zstd_encoding_is_unsupported`、`corrupt_gzip_is_rejected`、`gzip_bomb_is_too_big_quickly`、`multiple_content_encodings_are_unsupported`。炸弹单独跑：`finished in 0.26s`。断言在 4174–4178 行：`assert_eq!(problem, too_big_problem())`，并且 `elapsed < 5s`。解压用 `Read::take(MAX_DECODED_BYTES + 1)`（684–694 行），不会先把整包解完。 |
| 字符集：`Content-Type: text/html; charset=gbk`；只有 `<meta charset="gb2312">`；`<meta http-equiv="Content-Type" content="text/html; charset=GBK">`；UTF-8 BOM；UTF-16LE BOM；`charset=foo` 退回 UTF-8 | 是 | `content_type_gbk_decodes_chinese`（另含双引号、单引号）、`meta_charset_gb2312_decodes_chinese`、`meta_http_equiv_gbk_decodes_chinese`、`utf8_bom_wins_over_a_wrong_charset`（BOM 压过错误的 `charset=gbk`）、`utf16le_bom_decodes_chinese`、`unknown_charset_falls_back_to_utf8`。优先级在 `text_encoding`（约 748 行）。 |
| 乱码保护：0x80–0xFF 循环返回「页面内容读不出来」；正常中文页不误伤 | 是 | `binary_noise_is_unreadable`、`normal_chinese_page_is_not_flagged_as_garbled`。判定在 `text_is_garbled`：前 2000 个字符里 U+FFFD 或（除换行、回车、制表外的）控制字符占比**超过** 20% 才失败。 |
| 拦截页：36kr「火山引擎 正在进行安全检测…」无 `<title>` → `Outcome::Failed`；`<title>Just a moment...</title>` → Failed；超过 600 字且夹「验证码」→ Done；标题正常的短页面 → Done | 是 | `security_check_interstitial_fails`、`cloudflare_just_a_moment_fails`、`long_article_mentioning_captcha_succeeds`（正文超过 600 字，同时夹「验证码」和「安全检测」，断言 `Outcome::Done`）、`short_page_with_normal_title_succeeds`。走 `fetch_html`。 |
| `safe_curl_arguments` 含 `Accept-Encoding: gzip, deflate`，且仍然没有 `-L`、`Cookie`、`Authorization` | 是 | 新测试 `accept_encoding_requests_gzip_and_deflate_only`。原测试 `safe_curl_arguments_pin_ip_and_carry_no_credentials` 已改：断言恰好一个 `-H`，值为 `Accept-Encoding: gzip, deflate`，并禁止 `-L`、cookie、`Authorization`、`--compressed`。参数在 `safe_curl_arguments`（约 490 行）。 |
| `rust-fmt`、`rust-check` 通过，警告数不比开工前多（基线 4 条） | 是 | 开工前 `rust-check`：4 条（`MigrationReport.applied` / `backup_path`、`ensure_ready`、`repair_model_json`、`restore_database_file`）。做完后再跑：仍是这 4 条，`Finished dev profile`。`rust-fmt` 退出码 0。 |
| `git diff --stat` 里只出现 `apps/desktop/src-tauri/src/inbox_fetch.rs` | 是 | `apps/desktop/src-tauri/src/inbox_fetch.rs \| 703 +++++++++++++++++++++++++++++-`，1 file changed, 697 insertions(+), 6 deletions(-)。 |

`decode_page` 在 `inbox_fetch.rs:610`。`challenge_reason` 在 `inbox_fetch.rs:1374`。`curl_once_pinned` 在 572 行用 `decode_page` 代替原来的 `decode_body`。

拦截页看的是 **`body`**（`extract_web` 里 `pick_body` 的可见正文），不是 `excerpt`。调用在 1091 行。摘要会拼上 meta description，并且截到 2000 字，600 字的门槛就不再是页面上能看到的字。

## 改了哪些文件
- `apps/desktop/src-tauri/src/inbox_fetch.rs`：解压、字符集、乱码保护、`Accept-Encoding` 请求头、人机验证页识别，以及对应测试。删了只做 `from_utf8_lossy` 的 `decode_body`（旧的 `non_utf8_body_decodes_without_panic` 改走 `decode_page`，断言没放宽）。

## 验证
- `tools/cards/run.sh rust-check`（开工前）→ 4 条既有 dead_code 警告，退出码 0。
- `tools/cards/run.sh rust-fmt-fix` → 只整理了本文件。
- `tools/cards/run.sh rust-fmt` → 通过，退出码 0。
- `tools/cards/run.sh rust-check`（做完）→ 仍是那 4 条警告，没有新增。
- `tools/cards/run.sh rust-test gzip_bomb_is_too_big_quickly` → `ok`，`finished in 0.26s`（含把 20 MB 零压成 gzip 的时间，所以解码本身不超过这个数）。
- `tools/cards/run.sh rust-test utf16le_bom_decodes_chinese` → `ok`。
- `tools/cards/run.sh rust-test` → `773 passed; 0 failed; 5 ignored; 1 filtered out; finished in 69.71s`。
- `git diff --stat` → 只有 `inbox_fetch.rs`。

## 没做到的 / 偏离任务卡的地方
- 无。没有改 `Cargo.toml`，没有改 `inbox_fetch.rs` 以外的文件。

## 需要总控或 Fox 决定的事
- 无。

## 给审核者的提示
- 压缩炸弹的断言是 `assert_eq!(problem, too_big_problem())`（4174 行）加 `elapsed < Duration::from_secs(5)`（4175–4178 行）。`too_big_problem()` 的文案仍是「页面太大（超过 6 MB），没有读取」。任务卡要求炸弹走这个函数，所以 16 MB 的解压上限和 6 MB 的文案不是同一个数字。单独跑这条测试，cargo 报告 `finished in 0.26s`。
- 解压后如果字符串仍超过 `MAX_BODY_BYTES`（6 MB），`accept_body` 还是会跳过。任务卡不许改 `accept_body`。`decode_page` 自己的上限是 16 MB，用来挡住炸弹；6 MB 到 16 MB 之间的解压结果到不了后面的抽取。curl 的 `--max-filesize` 仍然只限制压缩后的字节。
- 没有 `Content-Encoding` 且以 `1f 8b` 开头才按 gzip 猜。头在但值为空，当成 identity，不猜魔数。逗号分隔或多个 `Content-Encoding` 头一律「读不了的压缩格式」，两个相同的值也算多个。
- `Content-Type` 里写了 `charset=` 但 `for_label` 不认识，就用 UTF-8，不再去看 `<meta>`。
- `deflate` 先走 `ZlibDecoder`，一个字节都没解出来才改用裸 `DeflateDecoder`。中途损坏不再回退。
- 拦截短语按任务卡原文做子串匹配，只把 ASCII 转成小写。「请开启JavaScript」这种没有空格的写法对不上「请开启 javascript」。长文测试里同时放了「验证码」和「安全检测」，用来守住 600 字这条线（「验证码」本身不在短语表里）。
- encoding_rs 的 UTF-16 只有解码器，`UTF_16LE.encode` 会原样返回 UTF-8。UTF-16LE 夹具是用 `str::encode_utf16` 拼的小端字节，再加 `FF FE`。
- 旧测试 `non_utf8_body_decodes_without_panic` 改为调用 `decode_page`。那一页只有三个非法字节，替换字符占比低于 20%，仍然能抽出标题里的 `OK`。
