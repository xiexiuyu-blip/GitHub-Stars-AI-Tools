---
编号: 20-solve-evidence
允许改动:
  - apps/desktop/src-tauri/src/solve.rs
---

# 卡 20 · 找方案：场景名不算「对上」的证据；没有理解卡的仓库也能被找到；中英文都认

## 为什么有这张卡
2026-10-03 一个「站在 Codex 角度」的审查员代入 Fox 的真实任务，在开发库副本上跑了「找方案」，发现挑工具这一步本身就错了（都在 `solve.rs`）：

1. **场景名被当成证据**。`load_repos` 把仓库所属的业务场景名（如「抖音·短视频」「视频·字幕·语音」）放进 `TextBlocks.scenarios`，`field_score` 里 `scenarios` 命中算 2.0 倍权重，`matched` / `full_match` 也因此成立。结果：问「短视频加字幕」，8 个命中里 6 个（爬虫、下载器，连默认勾选的 AiToEarn 在内）**只因为它们被归在「抖音·短视频」场景里**就显示「全都对上了」；真正做字幕的剪映 Skill、ChatCut 插件反而被挤出前 8。问「电商详情页图片」，前 3 名是视频引擎、收藏夹、画布插件，没有一个是做电商详情图的，而且不提示「缺口」。
2. **没有理解卡的仓库永远进不了结果**。`solve_query` 里 `.filter(|(index, _)| repos[*index].has_card)`，318 个没有卡的仓库只能被计入 `pending_matches`。但其中有最对口的——例如 `autoclip`（AI 视频剪辑，生成字幕）、`gpt-image-2.5-for-e-commerce`（电商图），它们的名称 / 描述 / 话题明明就能对上。
3. **只认中文**。318 个没有卡的仓库里 237 个只有英文描述，「字幕」对不上 `subtitle` / `caption`，「封面」对不上 `cover` / `thumbnail`。
4. **「为什么相关」是废话**。`why_text` 取第一个含任一意图词的字段，多数时候就是一句话本身（和「它是什么」逐字重复）。

这张卡**只改后台的匹配逻辑**，`SolveHit` / `SolveResult` 的字段形状**不变**（前端不改）。

## Fox 能看到的效果
- 问「短视频加字幕」，结果里是真的做字幕 / 剪辑的工具，不再是一串只因为「在短视频场景里」就被标成「全都对上了」的爬虫和下载器。
- 没整理（没有理解卡）的仓库，只要名称、描述、话题里真的对上，也会出现在结果里（带「待整理」标记，界面另一张卡处理）。
- 「为什么相关」说的是**哪个词对在哪里**（「名称里有『字幕』；能力里有『剪辑』」），不再重复一句话。
- 没有任何工具完整对上时，如实说有缺口（默认勾选的数量会变少，甚至为 0），不硬凑。

## 背景与先读什么
- 全部在 `apps/desktop/src-tauri/src/solve.rs`（2681 行，约 1400 行是测试）：`solve_query`（第 201 行）、`load_repos`（第 378 行，读库和拼 `TextBlocks`）、`score_repo` / `field_score`（第 506–605 行，打分）、`lexicon` / `concepts`（第 608–665 行，拆意图词，词表是 `BASE_LEXICON` + 场景名 + 场景关键词）、`why_text`（第 738 行）、`scenario_covers`（第 776 行，给界面推荐「去发现看看」的场景——**这个用途保持不变**，场景名在这里仍然有用）。
- `SolveHit` / `SolveResult` 字段（第 109–137 行）：`matched`（对上的意图词）、`missing`（没对上的）、`full_match`（意图词全对上）、`has_card`、`score`、`why`……**一个字段都不许增删、不许改名**。
- 仓库话题在 `repositories.topics_json`（JSON 字符串数组，495 个都有）；理解卡 `repo_ai_documents`（`one_liner_zh`、`summary_zh`、`keywords_json`、`card_json`）。
- 占位卡清洗：`crate::card_view::display_one_liner` / `card_view::sanitize`（`load_repos` 已经在用）。
- 现有测试（`mod tests`，从约第 880 行起）有造库夹具；改了行为的旧测试要**按新行为改断言**，不要为了让旧断言通过而保留旧行为（下面写明了哪些行为要变）。

## 要做的事

### 1. 场景名不算「对上」的证据
- `TextBlocks.scenarios` 保留，但**只作为加分项**：`field_score` 拆成「内容分」和「场景加分」。意图词是否算 `matched`（`score_repo` 里 `gained > 0`）**只看内容字段**：名称、一句话、关键词、能力、解决的问题、摘要、GitHub 描述、**话题（新增）**。
- 一个意图词已经 `matched`（内容里有）之后，如果仓库所属场景名 / 关键词也含这个词，可以再加 `0.5 × idf` 的小加分（让同一方向的排前面）；内容里没有，场景再对也**不算** `matched`，加分也不给。
- `contains_token`（用来算 idf 的）可以保留场景块，也可以去掉——自己定，但要保证上一条成立；写进报告。
- `full_match` 的含义不变：`!main.is_empty() && missing.is_empty()`，但现在 `missing` 只由内容字段决定。

### 2. 没有理解卡的仓库也能进结果
- 去掉 `solve_query` 里 `.filter(... has_card)`。没有卡的仓库用 `RepoRecord.blocks` 里的名称、描述、话题参与打分（它们的一句话 / 能力 / 问题 / 关键词 / 摘要块是空串，不会命中）。
- 没有卡的命中：`one_liner` 用 GitHub 描述的第一句（折成一行；以 `。！？` 或英文句号加空格结束；最多 120 个字符，超出在 `，；、` 或空格处截断，截断才加「…」）；`has_card` 为 `false`；`form` 为 `None`（或用 `repo_facts::infer_form`，**如果卡 18 已经合并了的话**，没有就留空，不要为这个等卡 18）。
- **排序**：有卡的命中和没卡的命中按**同一套分数**排，不要给没卡的额外加分或减分（它们本来就只有名称 / 描述 / 话题可命中，分数自然偏低）。同分时：有卡的在前，再按星数。
- **默认勾选**（`preselected`）：仍然只勾 `full_match` 的前 3 条，**没有卡的也可以被勾**（它们只有在名称 / 描述 / 话题里把所有意图词都对上才是 `full_match`）；没有 `full_match` 就一个都不勾。
- `pending_matches` 的含义改为：「**没有卡、名称或描述里把全部意图词都对上、但没有出现在返回的 `hits` 里**的仓库数」（因为现在没卡的能直接进结果了，这个数只剩「排在 8 名之外的」）。`main.is_empty()` 时仍是 0。
- `gap_intents`、`suggested_scenario_id`、`suggested_scenario_count`、`full_match_count` 的算法不变（但因为 `matched` 变严了，它们的值会跟着变）。

### 3. 中英文双语：意图词的英文同义词
- 新增常量 `SYNONYMS: &[(&str, &[&str])]`：中文意图词 → 英文词（全小写）。至少包含这些（可以再加，但每条都要有测试或夹具里会用到的理由，**别堆几百条**）：
  - `字幕` → `subtitle`, `subtitles`, `caption`, `captions`, `srt`
  - `封面` → `cover`, `thumbnail`, `poster`
  - `海报` → `poster`, `banner`
  - `配图` → `illustration`, `infographic`, `image generation`, `text-to-image`
  - `图片` → `image`, `images`, `picture`
  - `视频` → `video`, `videos`
  - `短视频` → `short video`, `short-video`, `shorts`, `reels`, `tiktok`, `douyin`
  - `剪辑` → `video editing`, `video editor`, `clip`, `cut`, `edit video`
  - `配音` → `voiceover`, `voice-over`, `tts`, `text-to-speech`, `dubbing`
  - `口播` → `talking head`, `voiceover`
  - `小红书` → `xiaohongshu`, `xhs`, `rednote`
  - `抖音` → `douyin`, `tiktok`
  - `公众号` → `wechat`, `weixin`, `official account`
  - `微信` → `wechat`, `weixin`
  - `电商` → `ecommerce`, `e-commerce`, `shopify`, `taobao`, `amazon`
  - `详情页` → `detail page`, `product page`, `listing`
  - `主图` → `product image`, `main image`
  - `文案` → `copywriting`, `copy writing`, `marketing copy`
  - `ppt` → `slides`, `presentation`, `pptx`, `powerpoint`
  - `文档` → `document`, `documents`, `docx`
  - `翻译` → `translate`, `translation`, `i18n`
  - `采集` / `爬虫` → `crawler`, `scraper`, `scraping`, `spider`
  - `总结` → `summarize`, `summary`
  - `知识库` → `knowledge base`, `rag`
  - `自动化` → `automation`, `workflow`
- 用法：意图词在内容字段里**没有直接命中**时，再看它的英文同义词是否出现在**名称、描述、话题、摘要、关键词**这几个文本块里（这些块已经是小写）；任一同义词命中就算这个意图词 `matched`，得分按「描述」那一档（`1.0 × idf`）算（比中文直接命中低一些）。同义词命中**不改变** `matched` 里记的意图词（仍然写中文意图词）。
- 同义词只能帮助「对上」，不能用来判断「缺口」之外的东西；`gap_intents` 仍然按 `matched` 算。
- 匹配用 `contains`，但**短词要防误伤**：长度 ≤ 3 的同义词（`srt`、`cut`、`tts`、`rag`、`i18n`、`xhs`）要求前后是非字母数字（词边界），`cut` 不能命中 `execute`。写一个 `contains_word(haystack, word)` 并单测。

### 4. 「为什么相关」说清哪个词对在哪里
- `SolveHit.why`（80 字符内，不重复一句话）。新写法：对每个 `matched` 意图词，记它命中的**最有说服力的字段**，按这个优先级：名称 → 能力 → 解决的问题 → 一句话 → 关键词 → 摘要 → GitHub 描述 → 话题 → （同义词命中）描述 / 名称。输出形如：`对上：字幕（名称）、剪辑（能力：自动剪口播并生成字幕）`——能力 / 问题命中时附上那一条的前 20 个字符。多个意图词用「、」连接，整体超过 80 个字符就截断到最后一个完整的「、」前，不要半个词。
- 没有意图词（只有 `extras`）时沿用现有行为（取第一个含这些词的字段）。
- 一个意图词都没命中的仓库不会出现在结果里（`score_repo` 本来就返回 `None`），所以 `why` 不会空。

### 5. 不要改的
- `scenario_covers` / `suggested_scenario_id`：场景名在「建议去发现看看哪个场景」这件事上仍然是对的证据，保持不变。
- `lexicon` / `concepts` 拆意图词的方式不变（词表里的场景名仍然参与**拆词**，只是不再参与**判定对上**）。
- 命令签名、`SolveHit` / `SolveResult` 字段、`HIT_LIMIT = 8` 不变。

## 不要做
- 不改前端、契约、`lib.rs`、其他 Rust 文件；不加依赖；不联网。
- 不要读 README 正文参与匹配（太重，另有打算）。
- 不要为了让总分好看而调旧权重；只做上面写的改动。

## 验收
1. `tools/cards/run.sh rust-test` 全部通过。新增测试**至少 16 个**，用小夹具（临时库 + 手写仓库 / 场景 / 卡）覆盖：
   - 场景名不算证据：仓库只在「抖音·短视频」场景里、名称 / 描述 / 话题里没有「短视频」「字幕」→ 问「短视频加字幕」不出现、或出现但 `full_match == false` 且 `matched` 不含场景独占的词；
   - 内容命中 + 场景加分：同一个词内容里有、场景里也有，排在只有内容的前面；
   - 没有卡的仓库：名称 `autoclip-subtitle`、描述 `AI video clipping with subtitle generation`、话题 `video-editing` → 问「短视频加字幕」能出现，`has_card == false`，`one_liner` 是描述的第一句，`matched` 含「字幕」（靠同义词 `subtitle`）；
   - 同义词：每组至少测 3 组（字幕 / 封面 / 电商）；`srt`、`cut` 的词边界（`execute` 不命中 `cut`）；
   - `preselected`：只勾 `full_match`，最多 3 个，没有就空；没卡的 `full_match` 可以被勾；
   - `pending_matches`：只计没出现在 hits 里的没卡的；
   - `why`：包含「对上：」「（名称）」等；超长截断在「、」；不等于一句话；
   - 没有任何意图词（只有 extras）的旧行为不变；乱输入一句没有任务词的话，仍不默认勾选、不「全部对上」。
   - 旧测试里依赖「场景名算证据」或「没卡的进不了结果」的断言，按新行为改，并在报告里列出改了哪几条。
2. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多。
3. `git diff --stat` 里只出现 `solve.rs`。
4. 报告里贴出：对一个自建夹具（包含：一个只在「抖音·短视频」场景的爬虫、一个有卡的字幕工具、一个没有卡但描述里有 subtitle 的英文仓库）问「短视频加字幕」，三个仓库各自的 `matched` / `full_match` / `score` / `why`。

## 停止条件
- 要改契约、前端或卡外文件；要新依赖；发现 `SolveHit` 字段必须增删才能做——停手并写进报告。
