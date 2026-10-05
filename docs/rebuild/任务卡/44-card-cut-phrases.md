---
编号: 44-card-cut-phrases
允许改动:
  - apps/desktop/src-tauri/src/card_pipeline.rs
---

# 卡 44 · 理解卡：能力名 / 关键词 / 「怎么试」超长时不再留半截话（后端，只改 card_pipeline.rs）

> 状态：待派发｜执行者：Codex｜只改 `card_pipeline.rs` 一个文件。**不改提示词、不升 `PROMPT_VERSION`**，已经整理好的老卡不动。

## Fox 能看到的效果
资料库里「能做什么」的小标签、关键词、「怎么试」里，不再出现「兼容 OpenAI、Anthropic、Gemi」「Excel/PPT/CSV 直转 Markdow」「decision intelligenc」这种被拦腰砍断的话。以后新整理的卡、以及点「润色」重新整理的卡：该断的地方断在顿号 / 逗号 / 英文单词之前；实在要硬截，就补一个「…」，让人一眼看出是缩写过的。（老卡不会自动变；哪张碍眼，点那张的「润色」。）

## 为什么有这张卡
总控读了 v3 全量整理的前 262 张真卡：
- 能力名恰好卡在 24 字的有 10 个，其中 7 个是明显的半截话：`兼容 OpenAI、Anthropic、Gemi`、`Excel/PPT/CSV 直转 Markdow`、`把 Word、PPT、Excel 转成 Mark`、`实时跟踪 43+ 款 AI 编程工具的 toke`、`把 GitHub Issue 直接变成 AI 会`、`聚合Reddit、X、YouTube等平台的热门`、`复用本地已登录的 AI 编程助手，无需 API `；
- 关键词恰好卡在 20 字的有 3 个：`decision intelligenc`、`Knowledge Distillati`、`scientific-illustrat`；
- 「怎么试」恰好卡在 300 字的有 1 个，一条命令断在 `…/renders/sto`。

原因一样：`normalize_capabilities`、`normalize_how_to_try`、`normalize_keywords` 用的是 `truncate_chars`（数到上限直接硬砍）。给人读的其他字段（一句话、它是什么、解决的问题…）早就改用 `truncate_text`（先在句末 / 逗号处断开，实在不行才硬截并补「…」，括号成对），这三处漏改了。另外 `truncate_text` 自己的硬截也会把英文单词劈成两半（`Markdo…`），这次一并补上。

## 背景与先读什么
- `apps/desktop/src-tauri/src/card_pipeline.rs`：`truncate_chars`（约 2632 行）、`truncate_text`（约 2647 行）、`hard_cut_with_closed_brackets`（约 2701 行）、`normalize_how_to_try`（约 2298 行）、`normalize_capabilities`（约 2324 行）、`normalize_keywords`（约 2391 行）、`normalize_scenario_evidence`（约 2279 行，**不动**）。
- 既有测试，改完必须仍然通过：`truncation_prefers_sentence_ends_then_commas_and_never_leaves_half_a_bracket`（约 3344 行）、`validate_repairs_how_to_try_capabilities_and_overlong_fields`（约 3740 行，断言 45 个「名」的能力名被截成 24 字）、`validate_dedupes_keywords_and_clamps_confidence`（约 3864 行）、以及 5840–5940 行一带的 `truncate_text_*` 括号用例。

## 要做的事

### 1. `truncate_text` 的硬截不劈开英文单词 / 数字串
只改 `truncate_text` 里「句末、逗号两种断法都没找到、落到硬截并补「…」」的那一支（句末 / 逗号两支、括号补全、`hard_cut_with_closed_brackets` 都不动）：

硬截点（保留前 `max_chars - 1` 个字）如果正好落在一串连续的 ASCII 字母数字中间（硬截点前一个字和后一个字都是 ASCII 字母或数字），就退回到这一串的开头；退回后把保留部分末尾的空白和 `，、,` 去掉，如果还剩 **至少 `max_chars / 2` 个字**，就用它，后面补「…」；不够就还是原来的硬截（不能把整句话砍掉大半）。

`max_chars = 24` 时的逐字期望：

| 输入 | 输出 |
|---|---|
| `Excel/PPT/CSV 直转 Markdown 并保留表格结构` | `Excel/PPT/CSV 直转…` |
| `把 Word、PPT、Excel 转成 Markdown 格式` | `把 Word、PPT、Excel 转成…` |
| `实时跟踪 43+ 款 AI 编程工具的 token 用量` | `实时跟踪 43+ 款 AI 编程工具的…` |
| `名` × 45 | `名` × 23 + `…`（和今天一样，共 24 字） |

`max_chars = 20` 时：`a` × 40 → `a` × 19 + `…`（退回后不够一半，维持原样）。

（以上是总控按规则手算的；如果和你实现出来的不一致，先想是不是规则理解错了，确实是总控算错就在报告里点明。）

### 2. 能力名（`normalize_capabilities`）
- 两处 `truncate_chars(…, name_max)`（字符串形式的一项、对象形式的 `name_zh`）改成 `truncate_text(…, name_max)`。
- 截完之后再收一道尾（写成一个小函数，名字自定）：若结果以「…」结尾，把「…」前面的空白和 `，、,；;。` 去掉再补回「…」；若不以「…」结尾，去掉末尾的 `，、,；;。！？` 和空白。**没超长的名字一个字都不变。**
- 保留 `evidence_line` 的既有处理，不动 `max_items`。

`name_max = 24` 时的逐字期望（第 1 条和 `evidence_line` 一起测）：

| 输入 | 输出 |
|---|---|
| `兼容 OpenAI、Anthropic、Gemini 等多家模型接口` | `兼容 OpenAI、Anthropic` |
| `Excel/PPT/CSV 直转 Markdown 并保留表格结构` | `Excel/PPT/CSV 直转…` |
| `复用本地已登录的 AI 编程助手，无需 API Key 即可调用` | `复用本地已登录的 AI 编程助手` |
| `把仓库批量整理成中文说明卡；同时保留原文的链接和更多内容` | `把仓库批量整理成中文说明卡`（句末「；」被收尾去掉） |
| `生成封面图` | `生成封面图`（没超长，不变） |

### 3. 「怎么试」（`normalize_how_to_try`）
`truncate_chars(&text, max_len)` 改成 `truncate_text(&text, max_len)`，没有别的改动。（数组形式用「；」连起来的既有行为不变，所以第一条命令完整、第二条被截时，会断在「；」之后。）

### 4. 关键词（`normalize_keywords`）
超过 `max_len`（默认 20）的关键词不再硬砍，也**不补「…」**（关键词是搜索用的词，带「…」没意义）：
- 在 `chars[..=max_len]` 里找最后一个「词边界」（空格、`-`、`_`、`/`、`·`、`、`、`，`、`,`），在它前面断开（保留 `chars[..p]`），去掉末尾的边界符号和空白；
- 断出来的部分少于 `min_len`（默认 2）个字，或者根本找不到边界 → 这条关键词丢掉；
- 其余逻辑（去空、`is_generic_keyword`、`min_len`、去重、`max_items`）和顺序不变；断出来的结果也要走去重。

`max_len = 20` 时的期望：

| 输入 | 输出 |
|---|---|
| `decision intelligence`（21 字） | `decision` |
| `Knowledge Distillation`（22 字） | `Knowledge` |
| `scientific-illustrator`（22 字） | `scientific` |
| `abcdefghijklmnopqrstuvwxy`（25 字，没有边界） | 丢弃 |
| `abcdefghijklmnopqrst`（恰好 20 字） | 原样保留 |
| `decision intelligence` 和 `decision platform` 同时出现 | 只留一个 `decision`（去重） |

## 测试（每一条都要能「改坏就失败」）
1. `truncate_text`：上面第 1 节的 4 条 + `a`×40 逐字断言；`truncate_text` 既有测试一个不改、不删、不放宽。
2. 能力名：对象形式和字符串形式各跑一遍同一批输入、得到同一批输出（对象形式带 `evidence_line: 3`，断言它还在）；上面第 2 节的 5 条逐字断言；既有 `validate_repairs_how_to_try_capabilities_and_overlong_fields` 里 45 个「名」的那条仍是 24 字（现在末尾会是「…」，在断言里补一句，并在报告里点名）。
3. 关键词：第 4 节的 6 条逐字断言（最后一条是整张卡走 `validate_card`，看 `keywords` 里只剩一个 `decision`）。
4. 「怎么试」：① 两条命令用「；」连着、总长超过 300 字、第二条命令跨过 300 字——结果不超过 300 字、以「；」结尾、第一条命令完整保留（第一条要 ≥ 150 字，才会走句末断法）；② 一条没有任何标点的长命令（≥ 400 字，其中有一个很长的英文词跨过 299/300 字的边界，前面有 ≥ 150 个字的普通内容）——结果不超过 300 字、以「…」结尾、不以半个英文词结尾。
5. 在报告里写明：把 `normalize_capabilities` / `normalize_how_to_try` / `normalize_keywords` 任一处改回 `truncate_chars`、或删掉第 1 节的「退回到单词开头」，分别会让哪几条测试失败。

## 不要做
- 不改 `PROMPT_VERSION`（仍是 `card-v3`；一改，已经整理好的 495 张都会被当成「旧版」）、不改提示词文字、不改 `docs/rebuild/spec/ai_card.schema.json`。
- 不改 `normalize_scenario_evidence`（场景引文的 60 字截法）、`truncate_chars` 本身（引文还在用）、`hard_cut_with_closed_brackets`、`inbox_fetch.rs` 里同名的 `truncate_chars`。
- 不写「修老卡数据」的脚本或迁移——老卡不动。
- 不动前端、不新增依赖、不新增数据库迁移、不改 `lib.rs`。
- 不放宽、不删现有断言；期望必须改的在报告里点名。
- 不碰正式库 / 开发库的数据目录和钥匙串；测试只用临时目录。

## 验收
1. `tools/cards/run.sh rust-fmt`、`rust-check`、`rust-test card_pipeline` 通过；沙盒里跑不了的测试如实写进报告，总控在沙盒外跑完整套。
2. `git diff --stat` 只出现 `apps/desktop/src-tauri/src/card_pipeline.rs`。
3. 报告里写：条号 → 文件:行；改了哪些既有测试；新增测试名；上面第 5 条的「改坏对照」；没把握的地方。

## 停止条件
- 需要改「允许改动」以外的文件，或必须改提示词 / schema 才能做完：停手，写进报告。
