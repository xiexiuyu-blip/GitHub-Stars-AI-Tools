# 卡 37 交接：Claude 子任务被中途停掉，工作区里是半成品（总控，2026-10-05）

背景：卡 37（后端提示的旧叫法、运行检查去掉两项白花钱的探测、CSV 加 BOM）原先由一个 Claude 子任务在做；Fox 改了分工（Claude 不再写代码，Grok 额度用完后交给 Codex），所以它在中途被停掉。**工作区里有它没提交的半成品改动**（8 个文件：`card_handler.rs`、`lib.rs`、`library_view.rs`、`local_api.rs`、`spend_guard.rs`、`v2_ops.rs`、`lib/automation-view.ts`、`lib/automation-view.test.ts`），没有交报告，没跑完检查，不知道做到哪一条。

## 你这一轮要做的事
1. 先看清现状：`git status`、`git diff --stat`、逐文件 `git diff`。**`lib.rs` 的改动有 1100 多行，很可能把「运行检查」整块重写了或带进了无关改动**——对照任务卡第 4 条逐处核对，超出卡的范围、或明显不合理的，改回原样（`git show HEAD:<路径> > <路径>` 取回原文，再只保留卡里点名要改的）。
2. 对着任务卡 `docs/rebuild/任务卡/37-copy-cleanup-rust.md` 的「要做的事」第 4、6 条和「验收」逐条核对：哪些已经做了、哪些没做、哪些做错了；补完、修正。
3. **特别小心 `spend_guard.rs`**：只许改提示正文（把「工具箱」「全量整理（不限额）」「AI 额度」等旧叫法换成统一用词），**判断逻辑、`REFUSAL_PREFIX`、`is_refusal`、`rerank_cap`、`account_slow_failure` 一个字都不许动**；改完用 `git diff -- apps/desktop/src-tauri/src/spend_guard.rs` 自己再看一遍，确认只有字符串变了。卡 34 新加的 AI 精选那几条提示（`refusal_for_rerank` 里）也要换成统一用词。
4. 「运行检查」（`lib.rs` 的 `check_runtime_readiness_worker` 一带）：去掉「相似推荐」「标签网络」两项 AI 探测，只留「AI 连接」一次；句子改成大白话；前端 `lib/automation-view.ts(.test.ts)` 里对应的两项结果名一并去掉。**不要碰** `spend_guard::probe`。
5. CSV 加 UTF-8 BOM 只给 CSV（`v2_ops.rs` 的 `export_csv`），`write_export_file` 的临时文件 + 改名逻辑不动。
6. 现有测试的断言一个都不许放宽或删除（把期望字符串换成新文案是对的）。
7. 把 `rust-fmt`、`rust-check`、`rust-test`、`web-build`、`web-test` 都跑一遍；沙盒里跑不了的如实写进报告。
