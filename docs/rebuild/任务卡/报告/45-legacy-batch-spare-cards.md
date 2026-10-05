# 交回报告 · 卡 45-legacy-batch-spare-cards

## 一句话结论

任务范围内的旧版覆盖保护已完成；格式、编译和 storage 58 项测试通过，lib 48 项通过、1 项因既有测试写死 `/tmp` 路径而被沙盒拒绝，需总控在沙盒外复跑。

## 验收逐条

| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| 1. `tools/cards/run.sh rust-fmt`、`rust-check`、`rust-test storage`、`rust-test lib` 通过（沙盒里跑不了的写进报告，总控在沙盒外跑完整套）。 | 部分 | 最终 rust-fmt、rust-check 退出 0；最终 storage 58 通过 / 0 失败；lib 48 通过 / 1 失败，失败为 `library_view::tests::imported_placeholder_cards_are_clean_in_library`，`apps/desktop/src-tauri/src/library_view.rs:3484` 写 `/tmp/fsl-15-library.txt` 时 PermissionDenied。 |
| 2. `git diff --stat` 只出现 `apps/desktop/src-tauri/src/storage.rs` 和 `apps/desktop/src-tauri/src/lib.rs`。 | 是 | 最终 diff：lib.rs 10 行变动，storage.rs 185 行变动；2 files changed, 190 insertions(+), 5 deletions(-)。报告为忽略文件。 |
| 3. 报告里写：条号 → 文件:行；新增的测试名；上面「改坏对照」；非测试写入点清单；没把握的地方。 | 是 | 下列实现映射、三项新增测试、三种实际改坏检查、10 条非测试直接 SQL 写入清单及限制说明。 |

### 要做的事：条号 → 文件:行

| 条号 | 文件:行 | 实际行为 |
|---|---|---|
| 1. 候选查询 | `apps/desktop/src-tauri/src/storage.rs:941` | 先要求 card_json 为 NULL 或空字符串，再套用原有缺文档、缺 README、哈希变化规则；保留账号、active 和 starred_at DESC。 |
| 2. 只读判断 | `apps/desktop/src-tauri/src/storage.rs:989` | `has_card_document` 用 SELECT EXISTS，仅判断 card_json IS NOT NULL 且不是空字符串；查询失败向上返回错误。 |
| 3. 批量跳过 | `apps/desktop/src-tauri/src/lib.rs:5552` | 每项处理闭包先调用判断，再读取 README 来源；命中即 Skipped。此判断不依赖 onlyMissing 或 repositoryIds，两种 onlyMissing 和指定 ID 路径共用它。 |
| 4. 单个拒绝 | `apps/desktop/src-tauri/src/lib.rs:5292` | 取得 storage 后立即调用判断；命中返回中文 Err，发生在账号验证、README 来源读取、补抓、AI 配置加载和业务写入之前。 |
| 5. 写入保险 | `apps/desktop/src-tauri/src/storage.rs:3484` | UPSERT 的冲突 UPDATE 只允许 card_json 为 NULL 或空字符串。判断后发生并发写卡，也由写入时 SQL 拦住；返回类型与 Ok 行为不变。 |
| 全部已是最新时的提示 | `apps/desktop/src-tauri/src/lib.rs:5693` | 零候选或本轮全部跳过时，提示「中文说明已经是最新的，不用再整理。」；额度或断路器停止提示仍优先。 |

### 测试：条号 → 文件:行

| 测试条号 | 新增测试名 / 文件:行 | 覆盖与结果 |
|---|---|---|
| 1. 候选 | `ai_document_candidates_exclude_nonempty_card_documents`；`apps/desktop/src-tauri/src/storage.rs:5776` | A–F 同一账号夹具在 storage.rs:5740；断言顺序 D、E、C，排除 A（卡且哈希不符）、B（卡且无 README）、F（旧文档哈希相符）。通过。 |
| 2. 保险 | `legacy_ai_document_save_preserves_cards_and_updates_legacy_documents`；`apps/desktop/src-tauri/src/storage.rs:5811` | A 的 card_json 为 `{"x":1}`、prompt_version 为 card-v3；旧保存返回 Ok，逐列比较 summary_zh、readme_zh、keywords_json、suggested_tags_json、model、prompt_version、source_hash、input_tokens、output_tokens、generated_at、card_json 均不变。再断言 C（NULL）及 D（空字符串）的旧字段和时间正常更新。通过。 |
| 3. 判断方法 | `has_card_document_requires_nonempty_card_json`；`apps/desktop/src-tauri/src/storage.rs:5793` | A/B 为 true，C/D/E 为 false。B 的旧 prompt_version 搭配非空 card_json 仍受保护，C 的 card-v3 搭配 NULL 仍按旧文档处理，证明判据只依赖 card_json。通过。 |
| 4. 两处 worker | `apps/desktop/src-tauri/src/lib.rs:5292`、`:5552` | 只调用同一个已测试的 storage 方法，没有在 worker 重写判据。单个入口在所有请求前拒绝；批量的逐项判断在该项 README 网络请求、AI 请求及 README/AI 文档写入前跳过。未启动 App 做 worker 实测，按卡片要求以调用位置为证据。全局初始化限制见下文。 |
| 5. 改坏对照 | 见「验证」中的实际执行记录 | 三种改坏均编译成功、对应测试失败；全部已恢复并复验。 |

既有 `ai_document_candidates_only_include_missing_or_changed_sources`（storage.rs:5699）没有修改、删除或放宽，实际通过；既有详情持久化和 embedding 失效测试也包含在 storage 的 58 项通过结果中。

## 改了哪些文件

- `apps/desktop/src-tauri/src/storage.rs`：候选过滤、只读判断、UPSERT 写入保险、A–F 临时库夹具及三项回归测试。
- `apps/desktop/src-tauri/src/lib.rs`：单个入口拒绝、批量逐项跳过及全部已是最新时的中文提示。
- `.card-report.md`：交回报告；已忽略，不在 git diff --stat 中。

## 验证

- `tools/cards/run.sh rust-fmt-fix` → 两次退出 0；最终仅两份允许文件有 diff。
- `tools/cards/run.sh rust-fmt` → 两次退出 0，包括恢复改坏检查后的最终版本。
- `tools/cards/run.sh rust-check` → 两次退出 0；最终耗时 4.21 秒，6 条既有 dead_code 警告，涉及未修改文件。
- `tools/cards/run.sh rust-test storage` → 首次及最终均 58 通过 / 0 失败 / 0 忽略；最终测试耗时 4.70 秒。
- `tools/cards/run.sh rust-test lib` → 48 通过 / 1 失败 / 0 忽略；耗时 5.64 秒，退出 101。唯一失败是既有 `/tmp` 写入被拒绝，未修改或跳过该测试。
- `git diff --check` → 退出 0，无空白错误。
- `git diff --stat`、`git status --short` → 仅 storage.rs、lib.rs 两个受跟踪文件修改。
- `cmp apps/desktop/src-tauri/src/storage.rs "$TMPDIR/card45-storage-good.rs"` → 退出 0，证明临时改坏全部恢复到保存的完成版。

### 实际执行的改坏对照

每次仅临时改坏一处，在上一种恢复后执行下一种；备份只放 `$TMPDIR`，没有删测试或放宽断言。

| 临时改坏 | 实际运行命令 | 输出摘要 / 会失败的测试 |
|---|---|---|
| 去掉候选查询的 card_json 条件 | `tools/cards/run.sh rust-test ai_document_candidates_exclude_nonempty_card_documents` | 0 通过 / 1 失败，退出 101；实际 A、B、D、E、C，期望 D、E、C，证明卡被误选。 |
| 去掉 UPSERT 保险 WHERE | `tools/cards/run.sh rust-test legacy_ai_document_save_preserves_cards_and_updates_legacy_documents` | 0 通过 / 1 失败，退出 101；首先在 summary_zh 的逐列不变断言失败，「替换摘要」覆盖了「新版摘要」。 |
| 判断方法只留 IS NOT NULL，去掉非空字符串判断 | `tools/cards/run.sh rust-test has_card_document_requires_nonempty_card_json` | 0 通过 / 1 失败，退出 101；D 的空字符串返回 true，预期 false。 |

## 没做到的 / 偏离任务卡的地方

- lib 过滤套件无法全绿：既有测试在 `library_view.rs:3484` 写死 `/tmp`，不受本次专用 TMPDIR 影响。未越界修改测试，也未改写其输出路径；请总控在沙盒外复跑。
- 按任务卡不硬做需要 AppHandle 的 worker 单元测试；没有启动 App、读取真实库、抓真实 README 或调用真实 AI。界面按钮及事件提示尚需总控在开发版验收。
- 批量入口原有全局 GitHub 账号验证仍在 `lib.rs:5487`，早于候选选择和逐项判断；因此不声称整个批量调用完全不联网。保证的是已有卡不进入该项 README/AI 处理、不发该项 AI 请求。单个入口的判断则早于账号验证。
- 两处入口保留原有 `AppStorage::from_app_handle` 初始化；`storage.rs:709` 内含迁移初始化。本报告的「写库之前」指本次 README/AI 业务写入，没有把既有初始化说成无写入操作。
- 按详细条款保留旧文档在哈希变化或 README 缺失时的更新，不把开头「没有任何说明」的概括扩展为禁止更新旧文档。未引入 trim 或 prompt_version 判据，空字符串仍算没有新版说明。
- 全仓核查发现两处卡外旧写入路径的并发风险，只列出，未修改；详见下文。

## 需要总控或 Fox 决定的事

- 是否另开卡加固 `v2_ops::import_summary_cards`：读取空 card_json 后到 UPDATE 之间，如果新版流水线写入了未确认卡，当前 UPDATE 可以把它替换为占位卡。
- 是否保留或另行处置已禁用的 `tools/legacy/fill-missing-cards.mjs`：同样存在读取候选到写入之间的并发风险。此次仅阅读源码，未运行脚本。

## 给审核者的提示

### 非测试 repo_ai_documents 写入点清单

用全仓 SQL 写入模式搜索（`rg -n -i -U`，匹配 INSERT/UPDATE/REPLACE 后接 repo_ai_documents），再核对测试模块及所有表名引用。共 10 条非测试直接 INSERT/UPDATE 写入，没有 REPLACE；SQL 文档示例、测试夹具不算生产写入。涉及此表的迁移触发器只写 embedding 表/队列，CLI/MCP 的相关引用是读表或校验结构，不另计为写入点。

| 文件:行 / 入口 | 会不会改写已有非空 card_json 的文档 | 依据 |
|---|---|---|
| `apps/desktop/src-tauri/src/storage.rs:3471` / save_repository_ai_document | 不会 | 本卡新增 storage.rs:3484 的 UPSERT WHERE；已有卡时整个冲突 UPDATE 不执行，Ok 返回不变。 |
| `apps/desktop/src-tauri/src/card_pipeline.rs:1003` / persist_card | 会，未确认卡可重新生成 | IMMEDIATE 事务内先复核 confirmed（card_pipeline.rs:983），确认卡跳过。其余 UPSERT 更新摘要、关键词、建议标签、模型、提示版本、哈希、用量、时间、一句话、完整 card_json、schema 和 README 基准；冲突更新不改 readme_zh。这是新版整理路径。 |
| `apps/desktop/src-tauri/src/card_handler.rs:305` / confirm_card | 只改 card_json 的 review_status | IMMEDIATE 事务内读取卡，由 card_handler.rs:751 的 rewrite_review_status 保留其他键并改为 confirmed；其他文档列不动。 |
| `apps/desktop/src-tauri/src/card_handler.rs:345` / request_regenerate | 只改 card_json 的 review_status 与 source_hash | 同一事务内把 review_status 改为 suggested、source_hash 清空，再排入新版队列；其他文档列不动。 |
| `apps/desktop/src-tauri/src/readme_follow.rs:603` / write_evaluation，HashUnchanged | 只改 readme_basis | 补未记录的 README 摘录。IMMEDIATE 事务内复核，detail_sql 限定有效、未确认、非空卡。 |
| `apps/desktop/src-tauri/src/readme_follow.rs:614` / write_evaluation，HashChanged 无基准 | 只改 readme_hash、readme_basis | 补哈希和基准，不写卡内容；同上事务与条件。 |
| `apps/desktop/src-tauri/src/readme_follow.rs:636` / write_evaluation，变化不明显 | 只改 readme_hash | 记录已评估的新 README 哈希，保留卡内容；同上事务与条件。 |
| `packages/storage/migrations/fox/035_readme_basis.sql:7` / 035 迁移回填 | 只改 readme_hash | 限定非空 card_json；回填生成之前抓到的 README 哈希，没有匹配记录时子查询为 NULL。其他列不改。只读取该迁移，未改文件或真实库。 |
| `apps/desktop/src-tauri/src/v2_ops.rs:559` / import_summary_cards | 只改三列，但可能替换完整卡内容；需单独处理 | UPDATE 写 one_liner_zh、完整 card_json、card_schema_version。v2_ops.rs:531 只在读取候选时要求空卡；写入时仅用文本 LIKE 排除 confirmed，没有再次要求空卡。 |
| `tools/legacy/fill-missing-cards.mjs:69` / 旧补卡脚本 | 冲突时改六列，可能替换完整卡内容；需单独处理 | 更新 summary_zh、one_liner_zh、完整 card_json、card_schema_version、model、generated_at。候选在第 34 行要求空卡，写入 WHERE（第 78 行）仅用文本 LIKE 排除 confirmed；并发写入的未确认卡仍可被覆盖。脚本默认禁用，未执行。 |

### 单独标出的疑似覆盖风险

- `import_summary_cards` 的占位卡替换风险及旧补卡脚本的替换风险均来自「读候选时为空，写入时已经有卡」的时间窗口；这两处不会被本卡 storage 保存方法的 WHERE 保护，因为它们直接写 SQL。
- 本卡的保险确保并发出现的理解卡不会被旧保存覆盖；若卡是在 worker 检查通过后才生成，已经启动的旧 AI 请求仍可能花 token。任务卡未要求串行化两套流水线，此次未新增锁或改写队列。
- 最终源码只改允许的两个 Rust 文件；没有改提示词、PROMPT_VERSION 常量、前端、依赖、表结构或迁移。三种临时改坏已经恢复，正式/开发数据目录和凭据未访问。

---

## 总控审核·第 1 轮（2026-10-05 夜，Claude）

- **执行者与过程**：Codex（`gpt-6.1-sol`，xhigh），第 1 轮 23:23–23:38，一次过，38 条命令，没有撞上「model at capacity」。
- **范围**：只改 `storage.rs`（+185，其中生产代码约 25 行、其余是 3 个新测试）和 `lib.rs`（+10），`check-scope.sh` 通过。生产 diff 我逐行对过：① 候选查询加 `AND (ai.card_json IS NULL OR ai.card_json = '')`，没有文档的（`LEFT JOIN` 出来是 NULL）照旧是候选；② `has_card_document`：`SELECT EXISTS(… card_json IS NOT NULL AND card_json != '')`；③ 批量 worker 的检查放在每个收藏处理的最前面（`lib.rs` 约 5552，读 README 来源、补抓 README、调 AI、写库之前），`onlyMissing` 两种取值和 `repositoryIds` 重试都挡得住；单个 worker 在补抓 README、调 AI 之前就返回中文提示（约 5292）；④ `save_repository_ai_document` 的 `ON CONFLICT … DO UPDATE` 加 `WHERE card_json IS NULL OR card_json = ''`，没写进去不算错误；⑤ 完成语由「`only_missing && progress_total == 0`」改成「全部都是跳过」（`lib.rs` 约 5693），否则 `onlyMissing=false` 全部跳过时会说「增量 AI 完成：生成 0 个」。`PROMPT_VERSION`、提示词、表结构、前端、其它文件一个字没动。
- **我的「改坏实现」检查**：脚本在草稿目录 `mutate-card45.sh`——在拷贝里一处一处改、只跑相关测试、不动仓库。共 **8 种全部抓住**：候选查询去掉 card 条件（候选测试）、写入保险去掉 `WHERE`（保险测试）、判断方法只判 `IS NOT NULL`（判断方法测试）、候选查询只判 `IS NULL`（候选测试）、写入保险只判 `IS NULL`（保险测试）、判断方法永远 `false`（判断方法测试）、候选查询让「有说明但没 README」的回来（候选测试 + 保险测试，后者见下）、判断方法反过来（判断方法测试）。原样跑 10 项全过。
- **发现的毛病（交第 2 轮修）**：改坏第 7 种时，保险测试多失败了一个——`UNIQUE constraint failed: github_accounts.login`。原因：3 个新测试共用夹具函数里写死的 `temp_storage("legacy-ai-document")`，库文件名是「标签 + 纳秒时间戳」，macOS 时钟只有微秒精度，几个测试同时起跑时拿到同一个库文件，第二个的夹具插入撞上第一个写的数据。现有 16 个 `temp_storage("…")` 标签各只用一次，新夹具破了这个惯例；整套跑时概率小，但会让合并 / 发布的检查偶发失败。第 2 轮只让夹具带上标签参数、三个测试各用自己的标签，断言和生产代码不动。交接 `续作/45-legacy-batch-spare-cards-r2.md`。
- **日志审计**：38 条命令里，命中正式 / 开发数据目录、钥匙串、凭证目录、git 写命令、联网、提权 / 删除的数量全是 0；改过的文件只有 `.card-report.md`、`lib.rs`、`storage.rs`。日志里出现 4 次 `com.foxwork` 都是提示词 / 文档文字，不是命令。
- **Codex 点出的另外两处写 `repo_ai_documents` 的旧代码，我的判断：不另开卡**。`v2_ops::import_summary_cards`（只由 `prepare_knowledge` 命令调用）和已禁用的 `tools/legacy/fill-missing-cards.mjs` 都是「先读到卡是空的、再写」的两步，中间若刚好有新版整理写进一张未确认的卡，会被占位卡换掉。但：开发库 501 篇文档现在全都有卡，候选是 0；`prepare_knowledge` 前端没有任何调用点（`apps/desktop/src` 里搜不到）；脚本默认禁用。记进「已知坑」，以后若给 `prepare_knowledge` 做入口再加固。
- **没验证的**：两处 worker 里的检查需要 `AppHandle` 和网络，没有单元测试，靠读代码（上面第 ③ 条）确认「在任何网络请求和写库之前」；数据库层的保险（第 ④ 条）有测试守着，即使 worker 检查被删，旧字段也写不进有 card 的文档。

## 第 2 轮

本节为续作 `docs/rebuild/任务卡/续作/45-legacy-batch-spare-cards-r2.md` 的实际交回结果。上文保留第一轮报告原文，其中测试数字、两文件 diff 和改坏检查都是第一轮的历史记录，不代表本轮重新执行。

### 一句话结论

完成：只替换 storage.rs 新增测试代码中的五行，让三个测试使用不同的临时库标签；格式、编译通过，document 测试连续 20 轮全部通过，每轮 19 项、零失败。

### 验收逐条

| 验收项（照抄续作） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| 1. `tools/cards/run.sh rust-fmt`、`rust-check` 通过；`tools/cards/run.sh rust-test document` 通过（这个过滤词会把三个新测试和既有的候选测试一起带上）。 | 是 | rust-fmt、rust-check 均退出 0；document 每轮 19 通过 / 0 失败 / 0 忽略 / 1364 被过滤，输出确认三个新测试和既有 `ai_document_candidates_only_include_missing_or_changed_sources` 均通过。 |
| 2. 把第 1 条里的测试命令**连跑 20 遍**不许失败：`for i in $(seq 1 20); do tools/cards/run.sh rust-test document \|\| break; done`，报告里写跑了几遍、有没有失败。 | 是 | 实际按同一命令连续运行 20/20 轮，默认每轮 8 个测试线程；每轮 19 项全部通过，合计 380 次测试执行、0 失败，循环退出 0。实际循环在失败时打印完整输出并退出，未使用重跑掩盖失败。 |
| 3. `git diff --stat` 仍然只有 `storage.rs`；**相对第 1 轮提交**（`git diff HEAD`），diff 只涉及夹具函数的签名、它里面那一行 `temp_storage(...)`、和三个测试里调用夹具的那三行（总共几行）。 | 是 | `git diff --stat`：仅 apps/desktop/src-tauri/src/storage.rs，1 file changed, 5 insertions(+), 5 deletions(-)；`git diff HEAD` 只有下面五处替换，lib.rs 无 diff。 |
| 4. 报告在第 1 轮报告基础上补一节「第 2 轮」：改了哪几行（文件:行）、连跑结果、新标签有没有和别的重复。 | 是 | 保留第一轮原文并追加本节；修改位置、20 轮结果及标签计数见下。 |

### 改了哪些文件：条号 → 文件:行

| 续作条号 | 文件:行 | 改动 |
|---|---|---|
| 1. 夹具签名 | `apps/desktop/src-tauri/src/storage.rs:5740` | `legacy_ai_document_storage` 增加 `name: &str` 参数。 |
| 1. 传递标签 | `apps/desktop/src-tauri/src/storage.rs:5741` | 原写死标签改为 `temp_storage(name)`。 |
| 2. 候选测试 | `apps/desktop/src-tauri/src/storage.rs:5777` | `ai_document_candidates_exclude_nonempty_card_documents` 传 `legacy-ai-candidates`。 |
| 2. 判断测试 | `apps/desktop/src-tauri/src/storage.rs:5794` | `has_card_document_requires_nonempty_card_json` 传 `legacy-ai-has-card`。 |
| 2. 保险测试 | `apps/desktop/src-tauri/src/storage.rs:5812` | `legacy_ai_document_save_preserves_cards_and_updates_legacy_documents` 传 `legacy-ai-save-guard`。 |
| 3. 其余不动 | `git diff HEAD` | 没有修改任何生产代码、lib.rs、temp_storage 本身、既有测试或断言；没有新增测试。 |

- `.card-report.md`：只在原报告末尾追加第 2 轮记录；报告被 git 忽略。

### 验证

- `tools/cards/run.sh rust-fmt-fix` → 退出 0；没有产生五处替换以外的源码 diff。
- `tools/cards/run.sh rust-fmt` → 退出 0。
- `tools/cards/run.sh rust-check` → 退出 0，3.11 秒；6 条既有 dead_code 警告，涉及未修改文件。
- `tools/cards/run.sh rust-test document` → 连续 20 轮，每轮 19 通过 / 0 失败 / 0 忽略，合计 380 次通过；测试阶段每轮 0.84–1.04 秒。
- `git diff --check` → 退出 0，没有空白错误。
- `git diff HEAD -- apps/desktop/src-tauri/src/storage.rs apps/desktop/src-tauri/src/lib.rs`、`git diff --stat`、`git status --short` → 只修改 storage.rs 的五行，5 行新增、5 行删除；lib.rs 无变动。
- 编辑前用 `rg -n 'legacy_ai_document_storage|ai_document_candidates_exclude_nonempty_card_documents|has_card_document_requires_nonempty_card_json|legacy_ai_document_save_preserves_cards_and_updates_legacy_documents|temp_storage\("' apps/desktop/src-tauri/src/storage.rs` 核对所有标签及三处调用。
- 编辑后用 `rg -o '(temp_storage|legacy_ai_document_storage)\("[^"]+"' apps/desktop/src-tauri/src/storage.rs | sed -E 's/^[^(]+\("([^"]+)"$/\1/' | sort | uniq -c` 核对：18 个直接或经夹具传递的标签计数均为 1；三个新标签各出现一次，与另 15 个标签无重复。

实际连续执行命令如下，完整的每轮输出写在本次专用 `$TMPDIR/card45-r2-document-1.log` 至 `card45-r2-document-20.log` 中，未在工作区创建临时文件；TMPDIR 会随本次任务清理。

```sh
document_passed=0
for document_round in $(seq 1 20); do
  document_log="$TMPDIR/card45-r2-document-$document_round.log"
  if tools/cards/run.sh rust-test document > "$document_log" 2>&1; then
    printf '第 %s/20 轮：' "$document_round"
    rg '^test result:' "$document_log"
    document_passed=$((document_passed + 1))
  else
    printf '第 %s/20 轮失败，完整输出如下：\n' "$document_round"
    cat "$document_log"
    exit 1
  fi
done
printf '连续完成 %s/20 轮，无失败。\n' "$document_passed"
```

### 没做到的 / 偏离任务卡的地方

- 本轮续作验收无未完成项，也无偏离。
- 按续作要求只运行格式、编译和 document 过滤套件；第一轮记录的 storage/lib 套件、改坏检查和写入点清单没有重新执行或扩展。
- 没有启动应用、读取正式库或开发库、访问钥匙串、发出真实 AI 请求或联网安装依赖。

### 需要总控或 Fox 决定的事

- 本轮无新增。

### 给审核者的提示

- 请以本轮 `git diff HEAD` 的五处替换核对范围；第一轮的生产实现已在 HEAD，本轮只隔离测试夹具。
- 三个测试标签不同，即使纳秒时间戳相同，也会落在不同文件名；没有修改全局 temp_storage 命名方式。
- 20 轮全部通过是实际运行结果；没有改断言、删测试、加 ignore 或跳过本轮过滤词下的用例。

---

## 总控审核·第 2 轮与发布（2026-10-05 夜，Claude）

- **第 2 轮**（Codex，23:46–23:51，一次过）：只改 `storage.rs` 的 5 行——夹具函数加 `name` 参数、三个测试各传 `legacy-ai-candidates` / `legacy-ai-has-card` / `legacy-ai-save-guard`；`git diff HEAD` 逐行对过，生产代码、`lib.rs`、断言一个字没动，`check-scope.sh` 通过。Codex 自己把 `rust-test document` 连跑 20 遍无失败；我在沙盒外又连跑 30 遍，0 次失败（修之前，3 个测试同时起跑时我撞过 1 次 `UNIQUE constraint failed`）。
- **合并**：`merge-card.sh` 在卡分支上跑完整检查——Rust **1378 通过 / 0 失败 / 5 忽略**（含钥匙串用例；比卡 44 合并时的 1375 多 3 项，就是本卡新增的 3 个测试）、前端 34 个文件 **853** 项通过——再 `--no-ff` 合进集成线（`5da1cfe`）。
- **发布 #15**（2026-10-05 23:55）：卡 44 + 卡 45 一起，`fox-rebuild` e84bbae → 5da1cfe（10 个提交，快进）；发布前的完整检查再跑一遍全绿。开发版此前被我关着（23:18 起），发布后由我在终端标签页重新编译启动（编译 15 秒，pid 90074），窗口是正常的「今日」页，没有卡在钥匙串。
- **发布后只读核对**（没点任何花钱 / 写入的按钮）：启动前后各做一份 `sqlite3 .backup` 对比——501 篇说明逐列没变（500 张 `card-v3` + 1 张旧版分块）、`ai_jobs` 账本 1168 行没变、预算控制文件逐字节没变、完整性检查 ok；新规则下旧版候选 **0**（旧规则是 500）。只点了导航（资料库 / 找方案 / 收集 / 发现 / 设置 / 今日），页面都能打开、数据正常；正式库目录 mtime 仍是 2026-10-01 12:30:17。
- **还没验证的**：旧版按钮在真界面上的新提示——我不点它们（写入 / 花钱类按钮只有 Fox 点；按代码，现在点下去不会再花 AI token，「抓取并分析」抓 README 那一步仍会联网）；10-05 22:31–23:18 事故留下的 56 万账本要到 10-06 22:31 之后才滚出 24 小时，在那之前每天额度只剩约 5 千，「每天自动整理」和后台排队任务不会领活（页面只给提示；「现在整理」不受限）。
