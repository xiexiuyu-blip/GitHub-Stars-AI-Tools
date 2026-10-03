# 交回报告 · 卡 20-solve-evidence

## 一句话结论
做完了：场景名不再算「对上」，没有理解卡的仓库和英文同义词能进结果，「为什么相关」改成写明哪个词对在哪个字段。

## 验收逐条
| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| `tools/cards/run.sh rust-test` 全部通过。新增测试至少 16 个，用小夹具覆盖：场景名不算证据；内容命中 + 场景加分；没有卡的仓库（autoclip-subtitle / subtitle / video-editing）问「短视频加字幕」能出现且 `matched` 含「字幕」；同义词至少测字幕 / 封面 / 电商，以及 `srt`、`cut` 的词边界；`preselected` 只勾 `full_match`、最多 3 个、没有就空、没卡的 `full_match` 可勾；`pending_matches` 只计没出现在 hits 里的没卡仓库；`why` 含「对上：」「（名称）」、超长在「、」处截断、不等于一句话；只有 extras 的旧行为不变；乱输入不默认勾选、不「全部对上」。旧测试里依赖「场景名算证据」或「没卡的进不了结果」的断言按新行为改，并在报告里列出。 | 是 | `tools/cards/run.sh rust-test` → `501 passed; 0 failed; 5 ignored; 0 measured; 1 filtered out`（那 1 条是脚本跳过的钥匙串用例；5 条 `ignored` 是原有的，本卡没有加 `#[ignore]`）。`solve::` 过滤再跑：`55 passed`。新增 21 个测试：`contains_word_rejects_partial_latin`、`scenario_alone_does_not_count_as_match`、`scenario_bonus_ranks_above_content_only`、`scenario_keyword_bonus_needs_content_first`、`cardless_english_subtitle_enters_hits`、`synonym_cover_matches_thumbnail`、`synonym_ecommerce_matches_shopify`、`synonym_poster_group_matches_banner`、`short_synonym_cut_and_srt_need_boundaries`、`preselected_caps_at_three_full_matches`、`preselected_can_include_cardless_full_match`、`preselected_empty_when_nothing_fully_matches`、`pending_matches_ignore_cardless_already_in_hits`、`why_points_at_name_before_capability`、`why_truncates_at_last_complete_separator`、`extras_only_why_keeps_legacy_wording`、`topics_match_without_a_card`、`cardless_one_liner_is_first_sentence`、`equal_scores_put_cards_before_stars`、`gibberish_query_does_not_preselect_or_full_match`、`evidence_short_video_subtitle_fixture`。改过的旧断言见下方。 |
| `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多。 | 是 | `rust-fmt` → 通过（退出码 0）。`rust-check` 改前、改后都是 4 条警告，都在 `migrations.rs` 的 `MigrationReport` 和 `v2_ops.rs` 的三个未使用函数上，`solve.rs` 没有新增警告。 |
| `git diff --stat` 里只出现 `solve.rs`。 | 是 | `git diff --stat` → `apps/desktop/src-tauri/src/solve.rs | 1083 ++++++++++++++++++++++++++++++++--`，`git status --short` 只有这一条 `M`。 |
| 报告里贴出：对一个自建夹具（只在「抖音·短视频」场景的爬虫、有卡的字幕工具、没有卡但描述里有 subtitle 的英文仓库）问「短视频加字幕」，三个仓库各自的 `matched` / `full_match` / `score` / `why`。 | 是 | 测试 `evidence_short_video_subtitle_fixture` 通过。三个仓库的结果见下一节。 |

### 夹具「短视频加字幕」

库里只有这 3 个在架仓库。意图词是「短视频」「字幕」。直接出现次数各为 1（英文同义词和场景名都不进 idf），idf = ln(2.5) ≈ 0.9162907318741551。

| 仓库 | 角色 | matched | full_match | score | why |
|---|---|---|---|---|---|
| `lab/page-fetcher`（id `crawler`） | 只归在「抖音·短视频」，名称 / 描述 / 话题里没有这两个词，也没有它们的英文同义词 | 未进 `hits`（内容没有对上，`score_repo` 返回空） | 未出现 | 无 | 无 |
| `lab/jianying-skill`（id `jianying`） | 有理解卡。一句话「给短视频加字幕」，能力「自动剪口播并生成字幕」 | 短视频、字幕 | true | 8 × ln(2.5) ≈ 7.330325854993241 | 对上：短视频（一句话）、字幕（能力：自动剪口播并生成字幕） |
| `lab/autoclip-subtitle`（id `autoclip`） | 没有卡。描述 `AI video clipping with subtitle generation`，话题 `video-editing` | 字幕（描述和名称里的 `subtitle`；「短视频」没对上） | false | ln(2.5) ≈ 0.9162907318741551 | 对上：字幕（名称） |

有卡的工具排在前面。爬虫没有被标成「全都对上」。

## 改了哪些文件
- `apps/desktop/src-tauri/src/solve.rs`：内容分和场景加分拆开；去掉「只返回有卡仓库」的过滤；读 `topics_json`；加 `SYNONYMS`、`contains_word`、没卡仓库的一句话截句，以及新的 `why`。测试按新行为改了断言，并新增 21 个。

## 验证
- `tools/cards/run.sh rust-fmt-fix` → 退出码 0（写完后整理格式）。
- `tools/cards/run.sh rust-fmt` → 退出码 0。
- `tools/cards/run.sh rust-check` → 通过，4 条警告，与改前相同。
- `tools/cards/run.sh rust-test solve::` → `55 passed; 0 failed`，含 3000 仓库的 1 秒性能用例。
- `tools/cards/run.sh rust-test` → `501 passed; 0 failed; 5 ignored; 1 filtered out`，约 36.5 秒。
- `git diff --stat` → 只有 `solve.rs`。

## 没做到的 / 偏离任务卡的地方
- 无。

旧测试按新行为改过的断言（没有删测试、没有放宽成永远通过）：

- `field_score_weights_take_the_max`：场景块单独含「封面」从 4.0 改为 0.0。补了一条：话题档等于描述档（权重 2.0 时都是 2.0）。
- `idf_matches_hand_calculation`：第四份文档的场景块改成「封面」，断言文档频率仍是 1。手算的 ln 值没变，用来钉住「场景不进 idf」。
- `fixture_five_prototype_queries`：问「做小红书封面」的命中从 `r01, r03, r02` 改为 `r01, r12, r02, r03`。`r01.why` 从「想快速做小红书封面」改为「对上：小红书（能力：生成小红书封面）、封面（能力：生成小红书封面）」。`full_match_count` 1→2，`preselected` 从 `[r01]` 改为 `[r01, r12]`，`pending_matches` 1→0。原来的「r12 不出现」改成：r12 无卡、`full_match`、一句话是描述全文。问「做电商详情页图片」的 `gap_intents` 从 `[图片]` 改为空：`r10` 的 `product main image`、`r04` 的 `wechat images` 用同义词对上了「图片」。这一问的 `full_match_count` 仍是 0，`preselected` 仍空。
- `pending_matches_need_one_field_to_hold_every_intent`：`pending_matches` 从 2 改为 0。`desc` / `name` / `split` 现在都在前 8 里。`split`「一个字段必须含全部意图词才计入 pending」改由新测试 `pending_matches_ignore_cardless_already_in_hits` 覆盖（它不在 hits 里，也不计入，pending 为 2）。
- `why_follows_problem_capability_one_liner_and_truncates`：`order` 的 why 改为「对上：封面（问题：这里有封面）」；`fallback` 改为「对上：封面（关键词）」；长问题不再整段截成 81 字加省略号，只在「问题：」后附前 20 字。
- `bad_card_json_does_not_fail`：`good` / `partial` 的 why 从「封面问题」改为「对上：封面（能力：封面能力）」；`types` 从「封面类型不对」改为「对上：封面（一句话）」。
- `removed_repositories_do_not_participate`：命中从 1 条改为 2 条（有卡的 `keep`，以及描述里有「封面」的没卡仓库 `pending`）。`pending_matches` 从 1 改为 0。`removed` / `gone` 仍不参与。`keep` 的分数断言没变。

## 需要总控或 Fox 决定的事
- 无。

## 给审核者的提示
- `contains_token`（算 idf）去掉了场景块，加上了话题。只挂在场景上的仓库不再抬高文档频率。同义词也不进 idf，只在直接没命中时补「对上」。
- 话题的直接命中权重卡里没写，取了和 GitHub 描述一样的 `1.0 × idf`，低于摘要的 `1.5`。同义词命中也是这一档，哪怕同义词出现在名称里（名称的 `3.0` 只给中文直接命中）。
- 场景加分是 `0.5 × idf`，加在内容分上面，不是再取最大。只加给已经对上的意图词；额外词不加。场景块现在含场景名和场景关键词，所以关键词也能触发这 0.5，但不能单独造成 `matched`。
- 同义词只在名称、描述、话题、摘要、关键词里找，不一句话 / 能力 / 问题里找。`why` 里同义词的字段顺序是名称 → 描述 → 关键词 → 摘要 → 话题。卡里写的是「描述 / 名称」，我把名称放前面，因为总优先级从名称开始，而且验收夹具的 `autoclip-subtitle` 要能写出「（名称）」。
- 长度 ≤ 3 的同义词走 `contains_word`。`i18n` 有 4 个字符，卡里点了名，也走词边界，避免命中 `i18next`。词边界只认 ASCII 字母数字，汉字可以紧贴英文。
- 没卡仓库的 `form` 固定为 `None`。仓库里没有 `repo_facts::infer_form`，卡 18 还没合并，没有为它等待。
- 没卡仓库的一句话：先折成一行，在 `。！？` 或「英文句号 + 空白 / 结尾」处收第一句（`3.5` 不会被切开）。超过 120 个字符时，在上限内最后一个 `，；、` 或空格处截断，只有截断才加「…」；上限内没有这些符号就在第 120 字处截断。
- `why` 的字段标签用短名：名称、能力、问题、一句话、关键词、摘要、描述、话题。能力 / 问题附上那一条的前 20 个字符。多条用「、」连接，超过 80 字就丢掉后面整段，不留半个「、」，也不加省略号。单条本身超过 80 字时，去掉括号里的附注，只留意图词；这是极端长词的退路，现有词表走不到。
- 同分排序在原有「星数、再 id」前面插入了「有卡的在前」。id 升序还留着，避免同星数时顺序乱跳。
- `pending_matches` 仍只看名称或描述是否直接含有全部中文意图词（不看同义词、不看话题），并且必须不在返回的 `hits` 里。`main` 为空时仍是 0。
- `scenario_covers`、`lexicon` / `concepts`、`SolveHit` / `SolveResult` 字段、`HIT_LIMIT` 都没改。
