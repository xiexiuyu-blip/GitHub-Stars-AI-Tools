# 交回报告 · 卡 15-card-sanitize

## 一句话结论
做完了：读取侧把旧占位卡洗成完整的一句话，并清掉三句固定废话；没有意图词时找方案不再标成「全部对上」，也不再靠一两个碎片硬凑。

## 验收逐条
| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| 1. `tools/cards/run.sh rust-test` 全通过，新增测试至少 24 个，覆盖任务卡列出的每一项；`rust-fmt`、`rust-check` 通过，警告数不比卡开始前多（4 条）。 | 是 | `rust-test`：444 passed，0 failed，5 ignored，1 filtered（钥匙串用例被 run.sh 跳过），27.47s。新增 `#[test]` 29 个（`card_view` 23、`library_view` 1、`solve` 4、`packs` 1）。另把原 `no_intent_still_marks_full_match` 改名为 `no_intent_does_not_claim_full_match`，分数断言保留，`full_match` / `preselected` 改成新规则。`rust-fmt` 通过。`rust-check` 仍是 4 条旧警告：`MigrationReport.applied/backup_path`、`v2_ops::ensure_ready`、`v2_ops::repair_model_json`、`v2_ops::restore_database_file`。 |
| 2. 逐项给出 `card_view.rs` 的函数位置；三个读取方的接入位置；没有意图词规则的位置。 | 是 | 见下方位置表。 |
| 3. 贴出两段真实摘要经 `import_summary_cards` 加本卡清洗后的 `oneLiner`、详情字段、`render_item_for_codex` 实际文本。 | 是 | 测试把实际文本写到 `/tmp/fsl-15-library.txt`、`/tmp/fsl-15-solve.txt`、`/tmp/fsl-15-packs.txt`。全文见下方。 |
| 4. `git diff --stat` 只出现允许改动的路径；`v2_ops.rs` 没有改动；`lib.rs` 只多一行 `mod card_view;`。 | 是 | `git diff --stat`：`lib.rs` +1、`library_view.rs`、`packs.rs`、`solve.rs`。`card_view.rs` 是新文件，尚未跟踪，`git status` 为 `?? apps/desktop/src-tauri/src/card_view.rs`，路径在允许清单里。`v2_ops.rs` 不在 diff 里。`lib.rs` 的 diff 只有 `mod card_view;`。 |

位置：

| 东西 | 位置 |
|---|---|
| `LEGACY_PROBLEM` / `LEGACY_CAPABILITY` / `LEGACY_HOW_TO_TRY` | `card_view.rs:8-10` |
| `is_legacy_placeholder` | `card_view.rs:15` |
| `derive_one_liner` | `card_view.rs:22` |
| `sanitize` | `card_view.rs:35` |
| `display_one_liner` | `card_view.rs:58` |
| 资料库 `into_item` 用 `display_one_liner`，`highlight` 用清洗后的卡；`has_card` 仍看库里的 `one_liner_zh` | `library_view.rs:741-746` |
| 资料库 `load_card` → `parse_card` → `sanitized_card` | `library_view.rs:779`、`961-976` |
| 找方案 `load_repos`：`display_one_liner` 写入 `one_liner` 和 `blocks.one_liner`，`parse_card` 取清洗后的问题/能力/形态 | `solve.rs:461-464`、`828-829` |
| 没有意图词：`score_repo` 要求命中 `ceil(额外词数/2)` 个不同额外词；有意图词时至少命中 1 个意图词 | `solve.rs:538-548` |
| 没有意图词时 `full_match` 一律为假（`preselected` 和 `full_match_count` 因此为 0） | `solve.rs:240` |
| 参考包唯一解析入口 `card_value` 过 `sanitize` | `packs.rs:623-634` |

核对文本（测试打印的实际结果）：

资料库 / 详情（`imported_placeholder_cards_are_clean_in_library`）：

```
awesome.oneLiner=Awesome LLM Apps 是一个可运行的 LLM 应用模板集合，包含100+ AI Agent和RAG应用。
awesome.highlight=None
awesome.hasCard=true
awesome.whatItIs=Awesome LLM Apps 是一个可运行的 LLM 应用模板集合，包含100+ AI Agent和RAG应用。每个模板均为原创、自包含的完整源代码，支持3条命令即可运行。覆盖AI Agents、多智能体团队、MCP、语音AI、RAG、微调等现代AI栈，支持Claude、Gemini、GPT、Llama等多种模型提供商。采用Apache-2.0许可证，可自由商用。
awesome.capabilities=[]
awesome.problemsSolved=[]
awesome.howToTry=None
awesome.confidence=Some(0.4)
awesome.cardReview=Some("suggested")

chatcut.oneLiner=ChatCut Agent Plugin 是一个连接 Codex 与 ChatCut 的插件…
chatcut.highlight=None
chatcut.hasCard=true
chatcut.whatItIs=ChatCut Agent Plugin 是一个连接 Codex 与 ChatCut 的插件，支持通过 AI 辅助编辑 ChatCut 视频项目，具体功能包括导入媒体、修改时间线、创建动态图形、生成素材、音频转录、添加字幕、导出视频以及验证编辑可见性。
chatcut.capabilities=[]
chatcut.problemsSolved=[]
chatcut.howToTry=None
```

找方案（`imported_placeholder_cards_are_clean_in_solve`）：

```
query=模板
oneLiner=Awesome LLM Apps 是一个可运行的 LLM 应用模板集合，包含100+ AI Agent和RAG应用。
why=Awesome LLM Apps 是一个可运行的 LLM 应用模板集合，包含100+ AI Agent和RAG应用。
fullMatch=false
preselected=[]
fullMatchCount=0

query=chatcut
oneLiner=ChatCut Agent Plugin 是一个连接 Codex 与 ChatCut 的插件…
why=ChatCut Agent Plugin 是一个连接 Codex 与 ChatCut 的插件…
fullMatch=false
preselected=[]

query=xyzzy不存在的东西
hits=0
preselected=[]
fullMatchCount=0
```

`render_item_for_codex`（`imported_placeholder_cards_render_without_boilerplate`）：

```
请参考这个 GitHub 项目：lab/awesome-llm-apps
- 链接：https://github.com/lab/awesome-llm-apps（★12 · 许可证：未知 · 最近更新：未知）
- 它是什么：Awesome LLM Apps 是一个可运行的 LLM 应用模板集合，包含100+ AI Agent和RAG应用。
- 怎么试：卡片里还没有记录，请打开项目主页查看
用法：先阅读项目主页和 README，再按我的需求借鉴它的做法；不要直接复制受限许可证的代码；不确定的地方先问我。

请参考这个 GitHub 项目：lab/chatcut-plugin
- 链接：https://github.com/lab/chatcut-plugin（★8 · 许可证：未知 · 最近更新：未知）
- 它是什么：ChatCut Agent Plugin 是一个连接 Codex 与 ChatCut 的插件…
- 怎么试：卡片里还没有记录，请打开项目主页查看
用法：先阅读项目主页和 README，再按我的需求借鉴它的做法；不要直接复制受限许可证的代码；不确定的地方先问我。
```

`lib.rs` diff：

```
diff --git a/apps/desktop/src-tauri/src/lib.rs b/apps/desktop/src-tauri/src/lib.rs
--- a/apps/desktop/src-tauri/src/lib.rs
+++ b/apps/desktop/src-tauri/src/lib.rs
@@ -3,6 +3,7 @@ mod auth;
 mod backup;
 mod capture;
 mod card_pipeline;
+mod card_view;
 mod db;
```

## 改了哪些文件
- `apps/desktop/src-tauri/src/card_view.rs`：新模块。判定占位卡、从 `what_it_is` 取第一句、清洗字段、给列表用的一句话。非占位卡 `sanitize` 返回借用，不复制。
- `apps/desktop/src-tauri/src/lib.rs`：只加了 `mod card_view;`。
- `apps/desktop/src-tauri/src/library_view.rs`：`parse_card` 清洗；`into_item` 的一句话走 `display_one_liner`，高亮走清洗后的卡。`has_card`、置信度、复核状态、SQL 筛选没改。
- `apps/desktop/src-tauri/src/solve.rs`：读卡时清洗；没有意图词时收紧命中和 `full_match`。
- `apps/desktop/src-tauri/src/packs.rs`：`card_value` 清洗后再交给原来的渲染。

## 验证
- `tools/cards/run.sh rust-fmt-fix` → 通过。
- `tools/cards/run.sh rust-fmt` → 通过（`--check`）。
- `tools/cards/run.sh rust-check` → 通过，4 条警告，都是改动前就有的，见验收第 1 条。
- `tools/cards/run.sh rust-test` → 444 passed，0 failed，5 ignored，1 filtered，27.47s。其中 `fixture_five_prototype_queries` 原断言仍通过。
- 占位卡回归用的是 `crate::v2_ops::import_summary_cards`，不是手写 JSON。同一测试读回库里的 `one_liner_zh`，Awesome 那条仍是硬截的 60 字（以「每个」结尾），和界面上的完整句不同，说明没有回写数据库。

## 没做到的 / 偏离任务卡的地方
- 「复制给 Codex」仍有一行「怎么试：卡片里还没有记录，请打开项目主页查看」。任务卡写占位卡因此没有「怎么试」那一行，同一段又要求 `resolve_how_to_try` 等下游不要改。现有 `render_codex` 在 `how_to_try` 为空时一定会写这句兜底。按「下游不用动」做了。三句占位废话（查看项目说明 / 打开仓库 README / 想知道这个收藏是做什么的）和「可借鉴的能力」那一行已经不在输出里。
- 原测试 `no_intent_still_marks_full_match` 断言的就是这张卡要修的行为。没有删测试、没有 `#[ignore]`、没有放宽分数断言；只把 `full_match`、`full_match_count`、`preselected` 改成新规则，并改了测试名。

## 需要总控或 Fox 决定的事
- 无必须停手的事。可选：若希望「怎么试」为空时整行不出现，要改 `packs.rs` 的渲染。那会作用到所有没写怎么试的卡，不只是占位卡，所以这张卡没动。

## 给审核者的提示
- 两段真实摘要都不到 400 字，所以详情里的 `whatItIs` 是全文，不是只留第一句。一句话才截到第一句。ChatCut 的第一句超过 60 字，在前 60 字里最后一个逗号前截断并加 `…`。
- 非占位卡的「去空白」沿用原来的 `nonempty_spaces`：只去掉两侧普通空格。制表符不去，避免改掉已有列表行为。有测试锁住 `\t一句话`。
- 占位卡的 `how_to_try` 若正好是「打开仓库 README」就写成 `null`；别的值则删掉这个键。真导入卡走 `null`。置信度保持 0.4，复核状态保持 `suggested`。
- 没有意图词时，`ceil(n/2)` 用的是 `usize::div_ceil`。6 个额外词时 floor 和 ceil 都是 3，所以另加了 5 个词的测试：命中 2 个不进，命中 3 个进。
- `why_text` 的挑选规则没加新分支。占位卡的问题和能力被清空后，`why` 自然落到清洗后的一句话。
- 库内数据、迁移、`contracts.ts`、前端、`v2_ops.rs` 都没改。
