# 卡 45 · 第 2 轮交接（只改第 1 轮新增的测试夹具，生产代码一个字不动）

第 1 轮的生产代码我（总控）逐行核对过，没有问题；我另外做了 8 种「改坏实现」检查，**全部被测试抓住**。但检查时发现新增测试**自己**有个毛病：

## 问题
`storage.rs` 里第 1 轮新增的 3 个测试（`ai_document_candidates_exclude_nonempty_card_documents`、`has_card_document_requires_nonempty_card_json`、`legacy_ai_document_save_preserves_cards_and_updates_legacy_documents`）共用夹具函数 `legacy_ai_document_storage()`，里面写死了 `temp_storage("legacy-ai-document")`。

`temp_storage` 的库文件名是「标签 + 纳秒时间戳」，而 macOS 的时钟只有微秒精度，所以几个**同标签**的测试同时起跑时，会拿到同一个库文件。我的检查里真的撞上了一次：第二个测试的夹具插入报 `UNIQUE constraint failed: github_accounts.login`（保险测试在没有任何生产改动的情况下失败）。文件里现有的 16 个 `temp_storage("…")` 标签都只被一个测试用，新夹具破了这个惯例。整套跑时概率小，但会让合并和发布前的完整测试偶发失败。

## 要做的事
只改 `apps/desktop/src-tauri/src/storage.rs` 里**第 1 轮新增**的那段测试代码：
1. `legacy_ai_document_storage` 加一个参数 `name: &str`，原样传给 `temp_storage(name)`。
2. 三个测试各传自己独一无二的名字，例如 `legacy-ai-candidates`、`legacy-ai-has-card`、`legacy-ai-save-guard`。先 `grep -n 'temp_storage("' storage.rs`，确认新名字和文件里别的标签都不重复。
3. 其余一概不动：不改 `temp_storage` 本身、不改别的既有测试、不改任何断言、不改生产代码、不改 `lib.rs`。

## 不要做
- 不要改生产代码（候选查询、`has_card_document`、`ON CONFLICT … WHERE`、`lib.rs` 两处检查）。
- 不要启动应用、不要碰数据目录和钥匙串。

## 验收
1. `tools/cards/run.sh rust-fmt`、`rust-check` 通过；`tools/cards/run.sh rust-test document` 通过（这个过滤词会把三个新测试和既有的候选测试一起带上）。
2. 把第 1 条里的测试命令**连跑 20 遍**不许失败：`for i in $(seq 1 20); do tools/cards/run.sh rust-test document || break; done`，报告里写跑了几遍、有没有失败。
3. `git diff --stat` 仍然只有 `storage.rs`；**相对第 1 轮提交**（`git diff HEAD`），diff 只涉及夹具函数的签名、它里面那一行 `temp_storage(...)`、和三个测试里调用夹具的那三行（总共几行）。
4. 报告在第 1 轮报告基础上补一节「第 2 轮」：改了哪几行（文件:行）、连跑结果、新标签有没有和别的重复。
