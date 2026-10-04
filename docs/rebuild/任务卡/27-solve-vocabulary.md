---
编号: 27-solve-vocabulary
允许改动:
  - apps/desktop/src-tauri/src/solve.rs
---

# 卡 27 · 找方案：分词不再切碎、任务词和英文同义词补全、「何时用」也算证据

## 为什么有这张卡
2026-10-04 一个站在 Codex 角度的审查员，在全量整理完成后的真实数据上跑了 6 个真实任务，查「找方案」的结果。发现 3 处根因，都在 `solve.rs`：

1. **分词切出碎片**。
   - 「把群聊记录整理成日报」→ `intents=[群聊]`，`extras=[记录, 录整, 整理, 理成, 成日, 日报]`。「录整」「理成」「成日」是把汉字两两硬切出来的碎片，不是词。
   - 「批量改图尺寸和压缩」→ `intents=[]`，`extras=[批量, 量改, 改图, 图尺, 尺寸, 寸和, 和压, 压缩]`。没有意图词时要求「至少对上一半额外词」，碎片拖垮了这一半，**一个结果都没有**。
2. **词表缺任务词**。「日报」「压缩」「尺寸」不在词表里，只能当 `extras`（权重只有 0.3），所以「群聊日报」的最佳答案（`wechat-group-daily`，名称和一句话里都有「日报」）虽然排第 1，但和一堆只含「群聊」的仓库挤在同一档里（真实记录：第 1 名 23.5 分，第 2~8 名 6~16 分，其中只有「群聊」的都被标成「全部对上」），而同时对上「群聊」和「日报」的第二个好答案（`wechat-intelligence-hub`）排第 6，只有 9 分。
3. **理解卡的 `use_when` 和 `what_it_is` 没参与匹配**。`use_when`（「何时用」，474 / 498 张卡都有）和 `what_it_is`（一段话说明）恰恰是「这个工具适合做什么」最直接的话，例如「想给视频加字幕」「需要为视频添加字幕时」。现在 `parse_card` 只读了 `problems_solved` 和 `capabilities`。

这张卡**只改后台的匹配逻辑**。`SolveHit` / `SolveResult` 的字段形状、`HIT_LIMIT`、打分公式（档位权重、idf、README 证据、同义词按「描述」档、多个意图词的乘数）**都不变**——我们在真实数据上试过很多种加权（多字段一致加分、查询里最靠右的词加权、「Codex 能驱动」形态先验、复合词回退），没有一种能稳定变好，所以这次**不要动打分**，只修「词」的质量。

## Fox 能看到的效果
- 问「把群聊记录整理成日报」，第 1 名仍是 `wechat-group-daily`，第 2 名是 `wechat-intelligence-hub`（现在排第 6），只对上「群聊」或只对上「日报」的排在它们后面，并且界面会标「只对上…」，不再显示成「全部对上」。
- 问「批量改图尺寸和压缩」不再什么都没有：有对上「压缩」「尺寸」的仓库（哪怕只是部分匹配，界面会写「只对上…」）。
- 卡片里「何时用 / 说明」写到的任务也能被找到。

## 背景与先读什么
- 全部在 `apps/desktop/src-tauri/src/solve.rs`（约 3950 行，大半是测试）：
  - `BASE_LEXICON`（第 15 行，`pub(crate)`）：**注意 `library_view.rs`（第 551 行 `split_known_words`）也在用它**。这张卡**不许改 `BASE_LEXICON`**，新词放进新常量（见「要做的事 2」），只在本文件的 `lexicon()` 里并入。
  - `lexicon`（约第 901 行）、`concepts`（约第 940 行）、`tokens`（约第 959 行）、`split_token_segments`、`is_token_separator`、`is_han`、`STOP_WORDS`（第 80 行）。
  - `solve_query`（第 307 行）：现在先 `concepts` 再 `load_repos`；做 df / 凝固度要用仓库文本，所以要改成**先 `load_repos`**（空查询的报错 `先写一句你想做的事` 仍然最先返回）。
  - `TextBlocks`（第 226 行）、`TextBlocks::contains_token`（第 291 行，只看内容块：name / one_liner / capabilities / problems / keywords / summary / description / topics，不含场景）、`load_repos`（第 492 行）、`parse_card`（约第 1311 行）、`CardBits`、`RepoRecord`。
  - `SYNONYMS`（第 120 行）、`synonym_list`、`synonym_needs_boundary`（≤ 3 个字符的同义词要词边界）。
- 真实卡片的字段（开发库里的一张卡）：`use_when`: `["需要给视频添加字幕时", ...]`（字符串数组）、`what_it_is`: 一段话（字符串）、`problems_solved`: 字符串数组、`capabilities`: `[{"name_zh": ...}]`。

## 要做的事

### 1. 分词：词表词照旧，剩下的汉字用「词典动态规划」切，只产出真的有用的词
`concepts(query, lexicon, repos)`（签名可以按需要调整，函数名保持）：

1. **词表词**（`lexicon`）：最长优先匹配、从句子里挖掉换成空格——现有逻辑**不变**，结果进 `main`（意图词）。
2. 剩下的文字仍按 `is_token_separator` 切成片段。每个片段里：
   - 连续的**非汉字**部分（英文 / 数字，长度 ≥ 2，不在 `STOP_WORDS`）→ `extras`，和现在一样；
   - 连续的**汉字段**用下面的动态规划切词，切出的词（长度 2 或 3、不在 `STOP_WORDS`）进 `extras`；**切不出词的汉字直接丢弃，不再产生两两碎片**。`tokens` 函数里「汉字两两切」的逻辑删掉。
3. **候选词**：汉字段里所有长度为 2 或 3 的子串 `w`，同时满足：
   - **收藏里出现过**：`df(w) ≥ 1`。`df(w)` = 内容块（见上，不含场景）里含 `w` 的仓库数（用现成的 `contains_token`）；
   - **凝固**（cohesive）：把所有仓库的内容块（name / one_liner / capabilities / problems / keywords / summary / description / topics，已经是小写）用换行拼成一个大字符串，数 `w` 出现的总次数 `T`；若 `T ≥ 3`，并且 `w` 左边（或右边）紧挨着的**汉字**里最常见的那个占 `T` 的 **75% 以上**，说明 `w` 几乎总是更长词的一截（「话机」几乎总在「对话机器人」里），**丢掉**。`T < 3` 不做这个检查。紧挨着的不是汉字（标点、空格、字母）不计入任何汉字的次数，但仍计入 `T`。
4. **动态规划**（让 `best[i]` = 切完汉字段前 `i` 个字的最高分，`best[0] = 0`）：每个位置 `i` 取两种走法的最大值：
   - 跳过第 `i` 个字：`best[i-1] + 0.0`（该字是连接词 / 虚词：`的了和与及或并把被给让从向在就也都还将帮请先再又吧呢吗啊呀成为以个这那一些想要做用来去到可能会是有对` 之一），否则 `best[i-1] - 0.6`；
   - 以第 `i` 个字结尾取一个候选词 `w`（长度 2 或 3）：`best[i-len] + 1.0 + 0.15 × min(ln(1 + df(w)), 4.0)`。
   - 只有**严格大于**跳过走法时才取词（同分选跳过）。回溯得到词序列，保持在句子里的先后顺序。
5. `extras` 去重保序，且不含已在 `main` 里的词。
6. **不变**：`main.is_empty()` 时「至少对上一半额外词」的规则；`main` 和 `extras` 都空时返回空结果；`SolveResult.intents` / `extras` 字段。
7. 性能：一次查询里 df / 凝固度只算查询里出现的候选词，不要对全部 n-gram 建表；整个 `solve_query` 在 500 个仓库上额外耗时应该在几毫秒内（不要求测，但别写成 O(候选 × 全文) 的嵌套重扫以外的更坏做法）。
8. 注意 `solve_query` 现在先 `concepts` 再 `load_repos`，要改成先 `load_repos`（空查询报错仍最先）；没有任何概念时仍然在读 README 之前就返回空结果。

### 2. 词表：新增 `TASK_LEXICON_EXTRA`（只在本文件的 `lexicon()` 里并入）
新增常量（不要改 `BASE_LEXICON`），在 `lexicon()` 里放在 `BASE_LEXICON` 之后、场景词之前并入（沿用现有去重和「按长度从长到短排序」）：

```
日报 周报 月报 摘要 二维码 表情包 头像 水印 抠图 改图 修图 压缩 尺寸 缩放 裁剪
转写 录音 会议 会议纪要 文字稿 提示词 播客 分镜 剪映 数字人 旁白 录屏 快手 知乎
飞书 钉钉 短剧 剧本 动画 演示文稿 幻灯片 流程图 思维导图 架构图 看板 用户画像
聊天记录 草稿箱 社群 朋友圈 知识图谱
```

（这些都是任务名词，不放「X生成 / X编辑 / 视频字幕」这种由两个词表词拼成的复合词：复合词在卡片里往往不连着写，放进去会降低召回。）

### 3. 同义词：`SYNONYMS` 追加以下条目（逐条照抄；已有的条目不动）
全部小写。≤ 3 个字符的词（`asr` `stt`）按现有规则自动要词边界，不用另写。**只放意思很具体的英文词**，不要加 `report` `design` `review` `title` `script` `notes` `prompt` `daily` 这类到处都有的词（前期试过，会让一半仓库「对上」）。

| 中文 | 英文同义词 |
|---|---|
| 压缩 | compress, compression, minify, tinypng, squoosh, imagemagick |
| 尺寸 | resize, resizing, dimensions, aspect ratio |
| 缩放 | resize, resizing, scaling |
| 裁剪 | crop, cropping |
| 改图 | image editing, edit image, photo editing, image editor, retouch |
| 修图 | retouch, photo editing, image editing |
| 抠图 | background removal, remove background, matting, rembg |
| 水印 | watermark, watermarks |
| 日报 | daily report, daily digest, daily summary, daily brief |
| 周报 | weekly report, weekly digest, weekly summary |
| 月报 | monthly report, monthly digest |
| 转写 | transcribe, transcription, whisper, speech-to-text, asr, stt |
| 录音 | voice memo, audio recording, sound recording |
| 会议 | meeting, meetings |
| 会议纪要 | meeting notes, meeting minutes |
| 文字稿 | transcript, transcripts |
| 排版 | typesetting, typography |
| 标题 | headline, headlines |
| 爆款 | viral |
| 选题 | topic ideas, content ideas |
| 脚本 | screenplay |
| 直播 | live stream, livestream, live streaming, live commerce |
| 私信 | direct message, direct messages |
| 涨粉 | audience growth, growth hacking |
| 投放 | ad campaign, ad placement |
| 竞品 | competitor, competitors, competitive analysis |
| 群聊 | group chat, group chats, chat history, wechat group |
| 聊天记录 | chat history, chat log, chat logs, chat records |
| 表格 | spreadsheet, xlsx |
| 简历 | resume |
| 淘宝 | taobao, tmall |
| 亚马逊 | amazon |
| 头像 | avatar, profile picture |
| 表情包 | sticker, stickers, meme |
| 播客 | podcast |
| 分镜 | storyboard |
| 数字人 | digital human, talking avatar, heygen |
| 录屏 | screen recording, screen recorder |
| 思维导图 | mind map, mindmap |
| 流程图 | flowchart, flow chart, mermaid |
| 幻灯片 | slides, slideshow |
| 演示文稿 | presentation, slides, pptx |
| 二维码 | qr code, qrcode |
| 提示词 | prompt engineering, prompt library, prompt templates |
| 知识图谱 | knowledge graph |
| 看板 | kanban |
| 飞书 | feishu |
| 钉钉 | dingtalk |
| 知乎 | zhihu |
| 快手 | kuaishou |
| 剪映 | capcut, jianying |
| 旁白 | narration, voiceover |
| 短剧 | short drama, micro drama |
| 用户画像 | user persona, user profile |
| 架构图 | architecture diagram |
| 朋友圈 | wechat moments |

### 4. `use_when` 和 `what_it_is` 参与匹配
- `CardBits` / `parse_card` 多读两项：`use_when`（字符串数组，读法同 `problems_solved`）和 `what_it_is`（字符串；缺失当空串）。经过 `crate::card_view::sanitize` 清洗后的值（占位卡清洗已在 `parse_card` 里做，沿用）。
- `RepoRecord.problems` = `problems_solved` 后面接上 `use_when`（去重保序）。`TextBlocks.problems` 用它拼；所以「解决的问题」档（2.0 × idf）现在也包含「何时用」；`why` 里「问题」一类的标签自然包含它们，不另加新标签。
- `TextBlocks.summary` = `summary_zh` + 一个空格 + `what_it_is`（小写）。「摘要」档（1.5 × idf）现在也包含 `what_it_is`。
- 其余档位、权重、idf、README 证据、同义词匹配的字段范围都不变。

### 5. 不要改的
- `BASE_LEXICON` 一个字都不改（`library_view.rs` 在用）。
- 打分：`field_score` 档位和数值、`score_repo` 里的 1.5 / 1.0 / 0.5 / 0.3 / 乘数公式、README 证据常量、`HIT_LIMIT`、`preselected` 规则、`pending_matches`、`gap_intents`、`scenario_covers`——全部不变。
- 命令签名、`SolveHit` / `SolveResult` 字段（前端不改）。

## 不要做
- 不改前端、契约、`lib.rs`、`library_view.rs`、其他 Rust 文件；不加依赖（不要引入分词库）；不联网；不读 README 来分词。
- 不要「顺手」调权重、加形态先验、加复合词回退、加多字段一致加分。
- 不要把动态规划切出来的词放进 `main`（它们可能是切歪的词，进 `main` 会让所有仓库都显示「没对全」）。

## 验收
1. `tools/cards/run.sh rust-test` 全部通过。新增测试**至少 24 个**（小夹具：临时库 + 手写仓库 / 场景 / 卡；分词类测试可以直接造一个只含仓库文本的夹具），覆盖：
   - 分词（用夹具语料，让下列词在语料里出现过，如「整理」「记录」「批量」「机器人」「对话」）：
     - 「把群聊记录整理成日报」→ `intents == [群聊, 日报]`（顺序按词表长度排序后的结果为准，用 `contains` 断言即可），`extras` 含「记录」「整理」，**不含**「录整」「理成」「成日」「把群」；
     - 「批量改图尺寸和压缩」→ `intents` 含「压缩」「尺寸」「改图」，`extras` 含「批量」，**不含**「量改」「图尺」「寸和」「和压」；
     - **凝固度**：夹具语料里「对话机器人」至少出现 4 次（分散在不同仓库）、「话机」只出现在这里；问「对话机器人」→ `extras` 不含「话机」「器人」；
     - 动态规划：连接词不产生词（「给老板做一份季度汇报」里不出现含「给」「做」「份」的词）；候选词 `df == 0` 的不出现；长度为 1 的不出现；
     - STOP_WORDS 里的词不出现在 `extras`；`main` 里的词不出现在 `extras`；
     - 纯英文 / 数字片段（如「做 sora 视频」里的 `sora`）仍进 `extras`；
     - 空白查询仍返回 `先写一句你想做的事`；`main` 和 `extras` 都空仍返回空结果且不读 README。
   - 词表：`日报` `压缩` `尺寸` 现在是 `intents`（不是 `extras`）；`BASE_LEXICON` 的内容**没变**（测试里断言它不含 `日报` `压缩` `尺寸`，且仍含 `小红书` `封面`）。
   - 同义词：上表里至少 12 条各有一个断言（包含：压缩 / 尺寸 / 日报 / 转写 / 抠图 / 水印 / 播客 / 流程图 / 二维码 / 飞书 / 剪映 / 数字人）；`asr` 的词边界（`ceasr` 之类不命中）；README 提到 `compress` 两次以上时「压缩」靠 README 证据对上。
   - `use_when`：一张卡里「字幕」只出现在 `use_when`（「需要给视频添加字幕时」）、其他字段都没有 → 问「字幕」能对上，`matched` 含「字幕」；`what_it_is` 同理（只出现在 `what_it_is` 里 → 对上，档位是 1.5 × idf）；占位卡不受影响。
   - 打分回归：同一张卡上，原来的断言（场景名不算证据、没卡的也能进结果、`preselected` 只勾 `full_match`、同义词词边界、`why` 写法）全部仍然成立；旧测试里依赖「汉字两两切碎片」或「词表没有日报 / 压缩 / 尺寸」的断言，按新行为改，并在报告里列出改了哪几条。
2. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多。
3. `git diff --stat` 里只出现 `solve.rs`。
4. 报告里贴出：用你自己造的夹具（至少含：一个名称和一句话里有「日报」「群聊」的仓库、一个只含「群聊」的仓库、一个只含「日报」的仓库、一个只在 `use_when` 里提到「字幕」的仓库、一个英文 README 里提到 `compress` 的仓库）对下面 6 句分别列出 **改前 / 改后的 `intents` 和 `extras`**：「做小红书封面」「公众号文章配图」「给短视频加字幕」「做电商商品详情页」「把群聊记录整理成日报」「批量改图尺寸和压缩」；以及「把群聊记录整理成日报」改后前 3 名的 `matched` / `full_match` / `score`。

## 停止条件
- 要改 `BASE_LEXICON`、契约、前端或卡外文件；要新依赖；发现没有改打分就做不到「验收」里某条——停手并写进报告，不要硬凑。
