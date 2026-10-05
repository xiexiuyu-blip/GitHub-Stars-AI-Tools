# 交回报告 · 卡 37-copy-cleanup-rust

## 一句话结论

部分完成：允许范围内的后端文案、两项 AI 探测移除和 CSV BOM 已完成；前端两行结果彻底移除、CSV 按钮提示受路径限制未完成，Rust 全套受沙盒限制未全通过。

## 验收逐条

| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| 1. `tools/cards/run.sh rust-fmt`、`rust-check`、`rust-test`、`web-build`、`web-test` 全部通过，警告数不增；上面各条有对应测试（新句子逐字断言）。 | 部分 | 格式、编译、前端构建通过；前端 649 通过。Rust 全套 1196 通过、56 因沙盒权限失败、5 忽略、1 过滤。本卡新增 23 个 Rust 用例均通过。编译 6 条警告、测试 1 条警告，与接手时遗留基线日志相同。详见「验证」。 |
| 2. `git diff --stat` 只出现「允许改动」里的文件。 | 是 | 实际运行 `git diff --stat`、`git diff --name-only`：只出现下述允许的 8 个文件；`git diff --check` 无输出、退出 0。 |
| 3. 报告里写：条号 → 文件:行；「运行检查」改完后每一项检查的名字和文案；没把握的地方。 | 是 | 下列条号映射、完整结果文案表和「没做到的 / 偏离任务卡的地方」。 |

### 要做的事：条号 → 文件:行

以下 Rust 文件路径均相对 `apps/desktop/src-tauri/src/`。

| 条号 | 结果 | 文件:行与证据 |
|---|---|---|
| 4：地图错误改为按类型看 | 是 | `v2_ops.rs:827`、`:836`、`:838`，分别为「按类型看读取失败 / 查询失败 / 行读取失败」。`:2306` 用损坏的内存夹具触发读取失败、行读取失败；`:2333` 核对全部三个新前缀及旧前缀消失。 |
| 4：方向排名改为场景排名 | 是 | `library_view.rs:966`、`:971`、`:974`；`:3530` 实际触发准备失败、读取失败，`:3557` 核对三条新前缀。动态数据库错误原样追加。 |
| 4：开始整理时的 AI 设置入口 | 是 | `card_handler.rs:494` 改为「设置 → AI 设置」，`:489` 的未确认提示统一为「一次性全部整理」；`:2262` 逐字断言未确认、未配置、无待整理数据三种拒绝，并验证没有开启整理。确认要求及入队逻辑未改。 |
| 4：本地接口入口提示 | 是 | `local_api.rs:490` 改为「令牌还没生成，请在「自动整理与备份」里重新开启本地接口」；`:889` 验证 503 和完整 JSON 错误文字。 |
| 4：每天额度及 AI 精选拒绝正文 | 是 | `spend_guard.rs:222`、`:451`、`:459`、`:464`、`:501`、`:586` 只换字符串；`:1899`、`:1928`、`:1953`、`:1971` 逐字核对。AI 精选暂停和超额正文原本已无指定旧词，保留并逐字断言。 |
| 4：额度判断与保护入口不变 | 是 | `spend_guard.rs:53` 的 `REFUSAL_PREFIX = "AI 额度："` 保留；只读文本比较显示：去掉字符串后的正式代码与 HEAD 逐字一致，`is_refusal`、`probe`、`rerank_cap`、`account_slow_failure` 分别与 HEAD 逐字一致；额度模块 58 用例通过。 |
| 4：运行检查文案和探测 | 是 | `lib.rs:3454` 的 worker 仅有 `:3511` 一次 `run_ai_connection_probe` 调用，删除两项旧探测及只供它们使用的辅助函数；原连接探测函数 `:3335` 与 HEAD 逐字一致。`:3615` 起集中结果文字，`:8210` 起逐字测试；`:8614` 检查调用数量和旧探测不存在。 |
| 4：前端去掉两项旧结果名 | 部分 | `apps/desktop/src/lib/automation-view.ts:722` 两个旧名称改为「已停止检查」；`:715` 费用提示改为「一次 AI」；`.test.ts:1032`、`:1048` 逐字断言。兼容键保留，设置页仍显示两行占位结果，见下文。 |
| 6：CSV UTF-8 BOM | 是 | `v2_ops.rs:1046` 定义 `EF BB BF`，`:1072` 在 CSV 表头之前写一次；`write_export_file`（`:1118`）与 HEAD 逐字一致，Markdown 导出未改。`:2162` 检查 BOM、表头、7 条数据与行数、中文、Markdown 无 BOM；`:2212` 逐字节检查引号、逗号及 CRLF 单元格；`:2241` 检查空库；`:2268` 检查失败后旧文件不变、临时文件无残留。 |
| 6：导出按钮说明「用 Excel 打开中文不会乱码」 | 否 | 按钮在不允许修改的 `apps/desktop/src/pages/v2/toolbox.tsx:662`。目前已有「CSV，可用 Excel 打开」，未增加中文编码说明；Rust 注释中的说明不算界面完成。 |

### 运行检查：改完后的名字和文案

名字来自 `apps/desktop/src/lib/automation-view.ts:722`；以下列出所有固定结果句子，变量用花括号表示。失败的原始原因统一放在 `detail` 的「原因：{error}」里；应用设置成功时另显示「设置文件夹：{路径}」。处理建议也有逐字断言，位置见表。

| 名字 | 结果文案 | 建议 / 代码与测试位置（`lib.rs`） |
|---|---|---|
| 资料库文件 | 通过：「能正常读取，资料库里有 {total_count} 个收藏。」失败：「资料库文件读不出来。」或「打不开资料库文件。」 | 查看可恢复备份，必要时清空本机数据并说明会清空本机资料、GitHub 收藏不受影响，检查文件夹权限。代码 `:3615`、`:3623`；逐字断言 `:8210`。 |
| 应用设置 | 通过：「设置能正常保存。」失败：「找不到保存设置的文件夹。」「没能建好保存设置的文件夹。」「设置没能保存。」「设置能保存，但检查用的临时文件没能删掉。」 | 指向文件夹读写权限、重新打开应用、清理检查临时文件和再次检查；位置未知时原因是「没能确定设置文件夹的位置。」。代码 `:3648`；逐字断言 `:8243`；真实临时目录写入与清理测试 `:8310`。 |
| GitHub 令牌 | 通过：「GitHub 连接正常，当前账号 @{login}。」失败：「GitHub 令牌不能用。」或「还没有连接 GitHub 账号。」 | 到「设置 → GitHub」换令牌或连接账号。代码 `:3697`；逐字断言 `:8352`。 |
| 读取我的收藏 | 通过：「能读到你在 GitHub 上的收藏，这次试读了 {read_count} 个。」失败：「读不到你在 GitHub 上的收藏。」跳过：「GitHub 令牌不能用，这一项没法检查。」或「还没有连接 GitHub 账号，这一项没法检查。」 | 检查网络 / 代理、等待 GitHub 请求限制恢复；跳过时先连接账号或换令牌。代码 `:3717`；逐字断言 `:8352`。 |
| 下载项目说明 | 通过：「能下载项目说明，这次试了 {full_name}。」失败：「读不到资料库里的收藏。」或「下载项目说明失败。」跳过：「资料库文件有问题，这一项没法检查。」「GitHub 还没有连接好，这一项没法检查。」「资料库里还没有收藏，这一项没法检查。」「试了 {full_name}，它没有项目说明，这一项没法确认。」 | 分别指向资料库问题、连接 GitHub、同步、检查项目说明能否打开；无项目说明时说明不影响使用。代码 `:3750`；逐字断言 `:8432`。 |
| AI 服务 | 通过：「AI 服务连接正常，用的模型是 {model}。」失败：「AI 服务没连上。」或「没能读到 AI 设置。」跳过：「AI 还没有设置，这一项没法检查。」 | 到「设置 → AI 设置」填写 / 重新保存服务商、模型和密钥，检查余额与网络。代码 `:3804`；逐字断言 `:8512`。 |
| 已停止检查（兼容 `tagNetwork`、`recommendation` 两项） | 固定跳过：「这一项已经不检查了。」详情：「对应的功能在新界面里没有入口，检查它只会白白花 AI 的费用。」 | 无请求、无处理建议。代码 `:3833`；逐字断言 `:8559`，JSON 兼容结构测试 `:8574`。设置页仍把它们计入「没检查」数量。 |

费用提示原文：「如果已经设置了 AI，检查时会真实调用一次 AI，会产生很少的费用。」仅代表调用连接探测一次；本次验证没有调用真实模型。

## 改了哪些文件

- `apps/desktop/src-tauri/src/v2_ops.rs`：按类型看错误文字、CSV BOM 与回归用例。
- `apps/desktop/src-tauri/src/library_view.rs`：场景排名错误文字与夹具用例。
- `apps/desktop/src-tauri/src/card_handler.rs`：开始整理的当前名称 / AI 设置入口与拒绝用例。
- `apps/desktop/src-tauri/src/local_api.rs`：本地接口提示与 503 夹具用例。
- `apps/desktop/src-tauri/src/spend_guard.rs`：拒绝正文及逐字测试，保护逻辑不变。
- `apps/desktop/src-tauri/src/lib.rs`：运行检查文字、两项探测移除、纯文案函数及逐字测试。
- `apps/desktop/src/lib/automation-view.ts`：一次探测费用提示、两项旧名称改为停止检查。
- `apps/desktop/src/lib/automation-view.test.ts`：对应逐字断言。

接手草稿中 `lib.rs` 有运行检查之外的整批解析错误文案改动，已还原为 HEAD 原文；`card_handler.rs` 的「没有需要整理的仓库了」也还原，避免额外扩展。本轮没有改任务卡或其他受跟踪文件。

## 验证

所有命令在本工作区根目录实际运行；测试使用夹具，没有启动 App 或真实模型服务。

- `tools/cards/run.sh rust-fmt-fix` → 退出 0。
- `tools/cards/run.sh rust-fmt` → 退出 0，无格式差异。
- `tools/cards/run.sh rust-check` → 退出 0，6 条已有警告（`MigrationReport` 字段、`README_RULE_CHARS`、`reserve_at`、`ensure_ready`、`repair_model_json`、`restore_database_file`）。与接手时 `.tmp-card/baseline-check.txt` 的警告名称和数量一致；遗留基线不是本轮重跑结果。
- `tools/cards/run.sh web-build` → 退出 0，类型检查与打包通过，2186 模块，无构建警告。
- `tools/cards/run.sh web-test` → 退出 1，27 文件均未执行测试，Vite 创建 `/tmp/.../ssr` 缓存被拒（EPERM）。
- `TMPDIR="$PWD/.tmp-card" tools/cards/run.sh web-test` → 退出 0，27 文件、649 用例通过，0 失败。
- `TMPDIR="$PWD/.tmp-card" tools/cards/run.sh rust-test` → 退出 101，1196 通过、56 失败、5 忽略、1 过滤。56 份失败详情全部为 `Operation not permitted`：45 个 `ai`、5 个 `auth`、2 个 `local_api` 用例要绑定本机测试端口；另外 4 个用例硬编码写 `/tmp`：`card_pipeline::tests::prompt_with_two_categories_and_one_scenario_redacts_secret`、`library_view::tests::imported_placeholder_cards_are_clean_in_library`、`solve::tests::imported_placeholder_cards_are_clean_in_solve`、`solve::tests::vocabulary_queries_rank_group_daily`。未改测试、未加忽略、未绕过沙盒。测试构建 1 条原有 `README_RULE_CHARS` 警告。
- `TMPDIR="$PWD/.tmp-card" tools/cards/run.sh rust-test readiness` → 7 通过、0 失败。
- `TMPDIR="$PWD/.tmp-card" tools/cards/run.sh rust-test csv` → 5 通过、0 失败。
- `TMPDIR="$PWD/.tmp-card" tools/cards/run.sh rust-test spend_guard::tests` → 58 通过、0 失败。
- 在全套日志中逐一核对本卡新增 23 个 Rust 用例 → 全部 `ok`，包括空库 CSV、损坏库、中文提示、停止探测和设置临时文件清理。
- `git diff --stat` / `git diff --name-only` → 仅允许的 8 个文件；`git diff --check` → 退出 0。
- `git diff -- apps/desktop/src-tauri/src/spend_guard.rs` → 已逐处审查，正式代码只改 6 条字符串；原测试只替换期望文字，并追加测试。
- 只读源码比较（`python3 -c` 读取本卡文件与 `git show HEAD:<路径>`）→ 额度正式代码移除字符串后逐字一致；上述四个保护函数、`REFUSAL_PREFIX`、`run_ai_connection_probe`、`write_export_file` 与 HEAD 逐字一致。允许 Rust 文件的原有测试函数遗漏数均为 0；原有状态测试的 14 条断言仍为 14 条，未放宽。

本轮创建的 `.tmp-card/` 日志、夹具和缓存交回前清理；接手时已有的 7 份日志保留，未删除他人文件。

## 没做到的 / 偏离任务卡的地方

- 没做到 Rust 全套通过：56 项是上述明确的沙盒权限失败，需要总控在沙盒外复核；不能据此宣称完整 Rust 验收通过。
- 没做到设置页完全去掉两行结果：`apps/desktop/src/pages/settings.tsx:789`、`:790` 显式读取两项，`:827`、`:828` 还参与汇总；`apps/desktop/src/types.ts:319`、`:320` 声明必填。两文件均不在允许路径内，删除兼容键会导致类型错误或运行时读空对象。保守处理为不执行探测、名称显示「已停止检查」、保留 8 键结果；没有用隐藏 DOM 等方式绕过限制。
- 没做到 CSV 导出按钮或悬停提示补编码说明：按钮在不允许修改的 `pages/v2/toolbox.tsx:662`。
- 未启动界面、未在 Excel 中手动双击验证；字节、CSV 内容和类型检查已验证，实际界面与 Excel 体验留给总控。
- `lib.rs` 仍有 1103 行 diff：包含移除旧探测、把原构造文字提成无外部调用的纯函数以便逐字断言、追加 9 个运行检查测试。不是整块改写业务流程；运行检查之外的草稿修改已还原。具体错误原因仍保留服务商 / 系统原文，可能带技术术语，未擅自清洗错误内容。

## 需要总控或 Fox 决定的事

- 总控补充允许路径或在集成线处理 `pages/settings.tsx`、`types.ts`：去掉两行及汇总项后，才能同时删除后端兼容字段和前端兼容键。
- 总控补充允许路径或在集成线处理 `pages/v2/toolbox.tsx`：为 CSV 按钮补「用 Excel 打开中文不会乱码」。
- 总控在沙盒外跑完整 Rust 套件；本卡不需要新依赖、额度调整或 Fox 批准花钱。

## 给审核者的提示

- 六项实际检查，只有 AI 服务有一次连接探测。两项兼容结果固定跳过，因此当前界面最多显示 6 项通过、2 项没检查；这不是另外两次模型请求。
- CSV 的首行有 BOM；直接用字符串读取时开头为 U+FEFF，读取方若按首列名匹配需要识别 BOM。Markdown 不加 BOM，导出原子写入与改名未改。
- 查询 / 排名失败中的数据库原因由 rusqlite 生成；测试实际触发可稳定构造的错误，另外核对完整固定前缀。不尝试通过改 SQL 触发无法稳定制造的 `query_map` 即刻错误。
- 本卡没有改提示词、`PROMPT_VERSION`、迁移、`jobs.rs`、`ai.rs`、依赖或真实数据；未发生卡 34 / 35 冲突。

---

## 总控审核（2026-10-05，Claude；本卡是 Codex 试点）

- **执行者**：Codex（`gpt-6.1-sol`，xhigh），派发脚本 `tools/cards/dispatch-codex.sh`，权限用 Codex 自带的按路径配置（`codex-fs-profile.txt`）。接手的是被中途停掉的 Claude 子任务留下的半成品。
- **范围**：`check-scope.sh` 8 个改动文件全在允许范围内；其余 113 个「越界」全是 Codex 放在工作区里的 `.tmp-card/` 测试夹具（未跟踪，不提交；派发脚本已改成给每次运行建专用 `TMPDIR`）。
- **Codex 日志审计**：79 条命令，没有一条碰过正式 / 开发数据目录、钥匙串、ssh / aws 等密钥目录；`com.foxwork` 只出现在它读 `AGENTS.md` / 接手说明时的输出里；没有任何 git 写命令；沙盒拦截只有 `/tmp` 写入和本机端口绑定（报告里的 56 个失败）。
- **沙盒探测**（`codex sandbox`，不调模型）：写工作区、写 `$TMPDIR` 允许；写 `/tmp`、写 `.git` 文件、写 git 目录、`git add`、联网全部拒绝；探测后没有任何东西被暂存。
- **`spend_guard.rs`**：逐处读过 diff，正式代码只换了 6 句拒绝提示的文字（`{}` 占位符个数和参数没变），`REFUSAL_PREFIX`、`is_refusal`、`rerank_cap`、`account_slow_failure`、`probe` 没动；测试里旧期望只换成新文案，外加 4 个逐字断言。
- **「运行检查」**：worker 里只剩一次 `run_ai_connection_probe`；两项旧探测不再调 AI，固定回「已停止检查」。`lib.rs` 的 1100 行改动都在这一块及其测试的行号范围内。
- **没做、转卡 38**：设置页（`pages/settings.tsx`）和 `types.ts` 里那两行占位结果的去除；导出按钮（`pages/v2/toolbox.tsx`）的「用 Excel 打开中文不会乱码」说明。两处都不在本卡允许路径内，Codex 按「保守处理」没有绕。
- **沙盒里没跑过的 56 个 Rust 用例**：由 `merge-card.sh` 在沙盒外完整复跑，结果见合并日志。
