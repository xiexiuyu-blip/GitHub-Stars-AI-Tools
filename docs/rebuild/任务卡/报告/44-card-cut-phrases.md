# 交回报告 · 卡 44-card-cut-phrases

## 一句话结论

部分完成：代码和 11 个新增测试已完成，格式与编译通过，模块测试 94 通过、1 个既有用例因沙盒禁止写 `/tmp` 失败；任务卡的关键词第 6 个示例有长度矛盾，按明确规则保留未超长短语，待总控确认勘误。

## 验收逐条

| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| 1. `tools/cards/run.sh rust-fmt`、`rust-check`、`rust-test card_pipeline` 通过；沙盒里跑不了的测试如实写进报告，总控在沙盒外跑完整套。 | 部分 | 最终 `rust-fmt`、`rust-check` 均退出 0；`rust-test card_pipeline` 运行 95 项：94 通过、1 失败、0 忽略、1285 筛除，测试耗时 5.11 秒，退出 101。唯一失败为既有 `prompt_with_two_categories_and_one_scenario_redacts_secret`，`apps/desktop/src-tauri/src/card_pipeline.rs:4049` 写 `/tmp/fsl-05-prompt-system.txt` 被拒绝，错误为 `PermissionDenied / Operation not permitted`。该测试原文未改，需总控在沙盒外复跑。 |
| 2. `git diff --stat` 只出现 `apps/desktop/src-tauri/src/card_pipeline.rs`。 | 是 | 实际输出仅该文件：`349` 行差异，`1 file changed, 343 insertions(+), 6 deletions(-)`；`git status --short` 也仅该文件为 `M`。本报告按契约被 git 忽略。 |
| 3. 报告里写：条号 → 文件:行；改了哪些既有测试；新增测试名；上面第 5 条的「改坏对照」；没把握的地方。 | 是 | 以下给出实现与测试条号映射、完整新增测试名、唯一既有测试补强、四组实际运行的改坏对照，以及受限项和示例差异。 |

下表中的 `card_pipeline.rs` 均指 `apps/desktop/src-tauri/src/card_pipeline.rs`，行号对应最终文件。

| 任务条号 | 做到了吗 | 文件:行及行为 |
|---|---|---|
| 要做的事 1：硬截不劈开 ASCII 字母数字串，退回后至少保留一半，否则沿用原硬截 | 是 | `card_pipeline.rs:2705`：只在硬截分支检查截点两侧、向前退到 ASCII 字母数字串开头、清掉末尾空白和 `，、,`，再比较 `max_chars / 2`；原句末、逗号和括号处理未改。 |
| 要做的事 2：两种能力名形式改用 `truncate_text`，清理截断尾巴，不改短名字、证据和条数 | 是 | `card_pipeline.rs:2342`、`:2366`：两处分支均使用 `truncate_text`；`:2392`：新辅助函数只清理超长名字；`:2370` 的 `evidence_line` 处理和 `max_items` 原逻辑保留。 |
| 要做的事 3：「怎么试」只替换截断函数 | 是 | `card_pipeline.rs:2321`：改成 `truncate_text`；数组仍在 `:2317` 用「；」连接，其他代码未改。 |
| 要做的事 4：超长关键词按最后一个词边界断开，无边界或过短丢弃，不补省略号，保持原过滤和去重 | 是 | `card_pipeline.rs:2438`：最多收集 `max_len + 1` 个字符判断超长，并包含索引 `max_len` 的边界；`:2440` 找最后一个边界，`:2445` 去掉末尾边界符号及空白；`:2450` 沿用 `min_len` 和精确去重。未超长词在 `:2448` 保留原样。 |
| 测试 1：三条英文能力短语、45 个「名」、40 个 `a` 逐字断言，保留既有截断测试 | 是 | `card_pipeline.rs:3434`：五项逐字断言全部通过；`:3462` 另测数字、混合字母数字、去空白后的半长阈值、完整单词以及转义符后多字节字符。既有截断和括号测试均未改并通过，包括 `:6249` 的 30000 组随机测试。 |
| 测试 2：对象与字符串形式的五条能力名输出一致，对象证据仍为 3；既有 45 个「名」断言仍为 24 字并补省略号断言 | 是 | `card_pipeline.rs:3501`：整张卡经 `validate_card`，两种形式各跑五条输入，逐字比较名称并检查每条对象的 `evidence_line: 3`；`:3554` 另测短名字、恰好 24 字和多种尾标点；`:4109` 给既有 24 字断言补末尾「…」断言。 |
| 测试 3：关键词前五条逐字断言，第六条整张卡验证截断后去重 | 部分 | `card_pipeline.rs:3587` 前五条全部通过；`:3601` 覆盖八种边界、边界恰好位于索引 20、最后边界优先、连续边界清理和不足 `min_len` 丢弃；`:3629` 用 21 字的 `decision intelligence`、22 字的 `decision methodologies` 和短词 `decision` 验证最终只剩一个 `decision`。原第六条的 `decision platform` 仅 17 字，不应截断，`:3641` 明确验证原两条输入最终为 `["decision", "decision platform"]`。 |
| 测试 4①：数组两条命令，总长超过 300，保留完整第一条及分号 | 是 | `card_pipeline.rs:3652`：第一条 160 字，第二条 305 字，总长 466 字；结果逐字等于第一条加「；」（161 字），并断言不超过 300 字。 |
| 测试 4②：无标点单条长命令，英文长词跨过 299/300 边界，保留前文并补「…」 | 是 | `card_pipeline.rs:3666`：普通前缀 192 字、长词 280 字，总长 472 字；结果逐字等于去掉前缀尾空白后加「…」（192 字），不会留半个英文词。 |
| 测试 5：改回 `truncate_chars` 或删除退到单词开头时，测试必须失败 | 是 | 四组均实际临时修改源文件并经 `tools/cards/run.sh rust-test` 运行，失败断言见下表；每组结束恢复原实现，最终模块测试在恢复后复跑。 |
| 不可信内容的恶意单行夹具和耗时断言 | 是 | `card_pipeline.rs:3682`：十万个未结束标签、十万个开括号、十万段反斜杠紧跟中文，全部没有夹入换行；检查四种规整函数长度和无边界关键词丢弃，合计耗时断言小于 2 秒，测试通过。 |

## 改了哪些文件

- `apps/desktop/src-tauri/src/card_pipeline.rs`：仅修改三类字段的规整与 `truncate_text` 硬截分支；新增能力名尾巴清理函数、11 个测试，补强 1 个既有测试。
- `.card-report.md`：本次交回报告，按命令契约允许写入，不进入代码 diff。

唯一修改的既有测试是 `validate_repairs_how_to_try_capabilities_and_overlong_fields`（`card_pipeline.rs:4073`）：原 45 个「名」截断后仍为 24 字的断言保持不变，在 `:4109` 增加末尾必须是「…」的断言。没有修改、删除或放宽任何既有断言，也没有增加 `#[ignore]`。

新增测试共 11 个：

| 测试名 | 文件:行 |
|---|---|
| `truncate_text_backs_up_before_ascii_words` | `card_pipeline.rs:3434` |
| `truncate_text_backs_up_before_numbers_and_checks_the_trimmed_floor` | `card_pipeline.rs:3462` |
| `validate_truncates_capability_names_in_both_forms_and_keeps_evidence` | `card_pipeline.rs:3501` |
| `normalize_capabilities_tidies_cut_tails_and_keeps_short_names_unchanged` | `card_pipeline.rs:3554` |
| `normalize_keywords_cuts_long_terms_at_word_boundaries` | `card_pipeline.rs:3587` |
| `normalize_keywords_uses_the_last_boundary_including_the_limit` | `card_pipeline.rs:3601` |
| `validate_dedupes_keywords_after_word_boundary_cuts` | `card_pipeline.rs:3629` |
| `validate_keeps_short_keyword_phrases_after_a_long_term_is_cut` | `card_pipeline.rs:3641` |
| `normalize_how_to_try_keeps_the_first_command_and_semicolon` | `card_pipeline.rs:3652` |
| `normalize_how_to_try_cuts_before_a_long_command_word` | `card_pipeline.rs:3666` |
| `phrase_normalization_handles_hostile_single_line_inputs_promptly` | `card_pipeline.rs:3682` |

## 验证

所有构建与测试均在工作区根目录经 `tools/cards/run.sh` 运行。

- `tools/cards/run.sh rust-fmt-fix` → 退出 0，完成 Rust 格式整理；改坏对照恢复后再次运行，退出 0。
- `tools/cards/run.sh rust-fmt` → 最终退出 0，无格式差异。
- `tools/cards/run.sh rust-check` → 最终退出 0，完成耗时 2.78 秒；6 条警告均在未修改文件，涉及 `db/migrations.rs`、`scenarios.rs`、`spend_guard.rs`、`v2_ops.rs`；最终本卡代码没有新增警告。
- `tools/cards/run.sh rust-test card_pipeline`（首轮）→ 93 通过、2 失败、0 忽略；一个失败为既有 `/tmp` 权限问题，另一个为我新增去重夹具错误：`decision platform integration` 的最后边界在第二个词之后，实际应为 `decision platform`。修正自建夹具为 `decision methodologies`，没有更改既有断言。
- `tools/cards/run.sh rust-test card_pipeline::tests::validate_dedupes_keywords_after_word_boundary_cuts`（修正夹具后）→ 1 通过、0 失败、1379 筛除，退出 0。
- `tools/cards/run.sh rust-test card_pipeline`（最终恢复实现后）→ **94 通过、1 失败、0 忽略、1285 筛除**，退出 101，测试耗时 5.11 秒。**11 个新增测试全部通过**，任务卡点名的原截断、能力长度、关键词规整和括号测试均通过；唯一失败是既有 `/tmp` 写入用例。
- `git diff --check` → 退出 0，无空白错误。
- `git diff --stat` → 仅 `apps/desktop/src-tauri/src/card_pipeline.rs`；343 行新增、6 行删除。
- `git status --short` → 仅 ` M apps/desktop/src-tauri/src/card_pipeline.rs`。
- `git diff --unified=0 -- apps/desktop/src-tauri/src/card_pipeline.rs` → 核对改动仅为本卡规整函数、硬截分支、测试时间类型导入、新增测试和一个既有测试的补强。`PROMPT_VERSION`、提示词、`normalize_scenario_evidence`、`truncate_chars`、`hard_cut_with_closed_brackets` 均无改动。

### 测试第 5 条：实际运行的改坏对照

每一组只临时改动本卡允许的源文件；最终实现备份放在本次 `$TMPDIR`，每组后恢复；没有提交改坏代码。下列失败均为预期结果，已逐条看过失败断言。

| 临时改坏的内容 | 实际运行命令 | 被抓住的测试及输出摘要 |
|---|---|---|
| 能力名两处 `truncate_text(name, name_max)` 均改回 `truncate_chars`，保留其余逻辑 | `tools/cards/run.sh rust-test card_pipeline::tests::validate_truncates_capability_names_in_both_forms_and_keeps_evidence` | 0 通过、1 失败，退出 101；`validate_truncates_capability_names_in_both_forms_and_keeps_evidence` 得到 `兼容 OpenAI、Anthropic、Gemi`，而预期是 `兼容 OpenAI、Anthropic`。该测试依次覆盖字符串和对象两种路径。 |
| 「怎么试」改回 `truncate_chars(&text, max_len)` | `tools/cards/run.sh rust-test card_pipeline::tests::normalize_how_to_try_` | 0 通过、2 失败，退出 101；`normalize_how_to_try_keeps_the_first_command_and_semicolon` 的分号结尾断言失败，`normalize_how_to_try_cuts_before_a_long_command_word` 的省略号结尾断言失败。 |
| 关键词截词逻辑改回 `truncate_chars(trimmed, max_len)` | `tools/cards/run.sh rust-test card_pipeline::tests::normalize_keywords_` | 0 通过、2 失败，退出 101；`normalize_keywords_cuts_long_terms_at_word_boundaries` 得到 `decision intelligenc` 而预期 `decision`；`normalize_keywords_uses_the_last_boundary_including_the_limit` 得到 `scientific illustrat` 而预期 `scientific`。 |
| 删除硬截分支中整段退回单词开头与半长阈值判断 | `tools/cards/run.sh rust-test card_pipeline::tests::truncate_text_backs_up_` | 0 通过、2 失败，退出 101；`truncate_text_backs_up_before_ascii_words` 得到 `Excel/PPT/CSV 直转 Markdo…`；`truncate_text_backs_up_before_numbers_and_checks_the_trimmed_floor` 留下 `123456…`。 |

## 没做到的 / 偏离任务卡的地方

- 没有获得 `rust-test card_pipeline` 全绿：既有 `prompt_with_two_categories_and_one_scenario_redacts_secret` 硬写系统 `/tmp`，当前沙盒明确禁止，实际返回 `Operation not permitted`。没有修改它的路径、忽略它或绕过限制。需要总控在沙盒外复跑该模块及完整 Rust 套件。
- 关键词第 6 个示例不能同时满足原期望和第 4 节明确算法：`decision platform` 为 **17 字**，没有超过默认 20 字，应原样保留；与被截成 `decision` 的 21 字短语不是精确重复项。因此原两条输入的最终结果是 `["decision", "decision platform"]`，不声称达到了示例里的「只留一个 decision」。采用保守做法，保留既有短词行为；用 `decision methodologies`（22 字）作为真正会截成同一词的去重夹具，并保留原两条输入的回归测试。没有修改任务卡或 schema。
- 第 1 节三个混排短语、45 个「名」和 40 个 `a` 的手算期望均正确，全部逐字断言通过；没有其他已知规则差异。
- 没有启动 App、读取或修改任何真实数据、调用真实模型；真实界面与老卡保留效果未亲手验收，交给总控按项目流程验证。

## 需要总控或 Fox 决定的事

- 总控确认关键词第 6 个示例的勘误：建议将 `decision platform` 替换为 `decision methodologies`，保持「只截超长关键词、原精确去重不变」的业务规则。当前实现按这条明确规则执行。
- 无新增依赖、迁移、付费、删数据或额外允许路径的请求。

## 给审核者的提示

- 起始 `git status --short` 为空，没有上一位开发者的半成品或越界改动。最终代码 diff 仍只涉及任务卡允许文件。
- 关键取舍是关键词示例的长度矛盾，见上文；代码层面没有其他已知不确定项。不要把 94/95 的模块结果写成全套通过。
- 只给超长能力名清理尾标点；原有首尾 `trim` 保留，未超长名称的标点和已有「…」不被新逻辑清理，恰好 24 字的名字也保留原样。
- 英文退回规则保留任务卡要求的半长保护，因此一个占满全文的 40 字母长串仍输出 19 字母加「…」，这是明确要求的回退行为。
- 原句末／逗号两支、括号补全及其 30000 组随机测试均未改。提示词仍为 `card-v3`，不会自动把老卡判成过期；没有老卡修复脚本或迁移。
- 四组改坏对照均已恢复，最终模块测试、格式和编译检查是在恢复后的实现上运行的。

---

## 总控审核（2026-10-05，Claude）

- **执行者与过程**：Codex（`gpt-6.1-sol`，xhigh），一轮，18:55–19:11（61 条命令）；没有撞上「model at capacity」。卡里让它只跑 `rust-fmt` / `rust-check` 和模块测试，完整套件由我在沙盒外跑。
- **起因**：v3 全量整理进行到 262 张时，我读真卡数出来的：能力名恰好卡在 24 字的 10 个（7 个明显是半截话）、关键词恰好卡在 20 字的 3 个、「怎么试」恰好卡在 300 字的 1 个。三处用的是硬砍的 `truncate_chars`，其余给人读的字段早就改用 `truncate_text`。
- **总控勘误**：任务卡关键词第 6 个示例是我写错的——`decision platform` 只有 17 字，没超长，不该被截。Codex 发现后保守处理（没超长的原样保留），并用 `decision methodologies` 做真正会截成同一个词的去重夹具；报告里如实写了「没有声称达到示例」，任务卡已改正。
- **范围**：只改 `card_pipeline.rs`（+343 −6；生产代码约 65 行，其余是 11 个新测试 + 1 条既有测试补一句断言），`check-scope.sh` 通过。生产 diff 逐行对过：① `truncate_text` 只在「句末 / 逗号都没找到、落到硬截」那一支加了「不劈开英文单词 / 数字串、退回后至少保留一半」，`max_chars == 0`、`keep == 0` 的边界都有保护，括号补全不动；② 能力名两处都换成 `truncate_text`，再由 `trim_truncated_capability_name` 收尾（只处理超长的，没超长的原样）；③ 「怎么试」一行换函数；④ 关键词多读一个字符判断超长，找最后一个词边界断开、不补「…」，没边界或过短就丢，去重照旧。`PROMPT_VERSION`（仍是 `card-v3`）、提示词、schema、`normalize_scenario_evidence`、`truncate_chars`、`hard_cut_with_closed_brackets` 一个字没动。
- **测试**：我在卡工作区沙盒外跑 `run.sh rust-test`：1374 通过 / 0 失败 / 5 忽略 / 1 个（钥匙串用例）被过滤（82.6 秒），`rust-fmt` 通过；`merge-card.sh` 合并时在集成线上跑完整套（含钥匙串用例）：**1375 通过 / 0 失败 / 5 忽略**，前端 34 个文件 **853** 项通过。Codex 在沙盒里跑 `card_pipeline` 模块是 94 / 95——唯一失败是既有 `prompt_with_two_categories_and_one_scenario_redacts_secret` 要写系统 `/tmp`，被沙盒拒绝，沙盒外是通过的。11 个新增测试、既有测试一个没删没放宽（只在 45 个「名」那条补了「末尾是…」的断言）。Codex 自己加了一条「十万字符恶意单行输入」的耗时断言（< 2 秒），本机实测 0.04 秒。
- **我的「改坏实现」检查**：脚本在草稿目录 `fox-mutate-c44.py`，每次只改一处、跑 `card_pipeline` 模块、立刻还原。共 **18 种，全部抓住**（另有 Codex 自己做的 4 组）：硬截回退 6 种（半长保护改 `>`、去掉半长保护、只看前一个字、只认字母不认数字、不去尾部空白、半长改三分之一）、能力名 5 种（「…」前不收尾、去掉「！？」、没超长的也收尾、只改字符串形式、只改对象形式）、关键词 6 种（超长判断失效、取第一个边界、不去末尾边界符号、少两种边界字符、没边界时硬砍、不再去重）、「怎么试」1 种。还原后文件哈希一致、工作区干净。
- **日志审计**：61 条命令，命中正式 / 开发数据目录、钥匙串、凭证目录、git 写命令、联网、提权 / 删除的数量全是 0；改过的文件只有 `.card-report.md` 和 `card_pipeline.rs`。
- **没验证的**：真实界面和真实数据（Codex 不能启动应用）。**已经整理好的老卡不动**：那十来张的半截话仍在，而界面里**没有「单张重做」的按钮**（现有的只有 `missing` / `outdated` / `readme_changed` 三种批量范围），所以要处理老卡得另做一张小卡（后端按仓库排队 + 详情页一个按钮）。发布要等整理跑完（含 Rust，开发版会重启）。
