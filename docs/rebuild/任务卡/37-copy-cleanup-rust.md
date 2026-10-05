---
编号: 37-copy-cleanup-rust
允许改动:
  - apps/desktop/src-tauri/src/v2_ops.rs
  - apps/desktop/src-tauri/src/library_view.rs
  - apps/desktop/src-tauri/src/card_handler.rs
  - apps/desktop/src-tauri/src/local_api.rs
  - apps/desktop/src-tauri/src/spend_guard.rs
  - apps/desktop/src-tauri/src/lib.rs
  - apps/desktop/src/lib/automation-view.ts
  - apps/desktop/src/lib/automation-view.test.ts
---

# 卡 37 · 文案收尾（后端提示）：旧叫法、运行检查、CSV 编码

## 为什么有这张卡
同卡 36，处理后端（Rust）里用户看得见的提示：旧叫法、开发口吻的「运行检查」、CSV 在 Excel 里乱码。**只改文字、去掉两项白花钱的探测、给 CSV 加 BOM**；额度判断逻辑一个字不动。依据 `docs/rebuild/文案审读.md` 的统一用词表。

## Fox 能看到的效果
- 报错里不再出现「地图」「方向排名」「设置 → 引擎」「全量整理（不限额）」「AI 额度」这些旧叫法。
- 「运行检查」里的句子是大白话；它只检查 AI 连接一次（后面两项检查对应的功能页面已经看不到了，不再白花 token）。
- 导出的 CSV 用 Excel 双击打开，中文不乱码。

## 要做的事

### 4. 后端提示里的旧叫法（只改文字，测试里的期望句子一起改成新句，**不许放宽断言**）
- `v2_ops.rs` 约 847、856、858 行「地图读取失败 / 地图查询失败 / 地图行读取失败」→「按类型看读取失败 / 查询失败 / 行读取失败」；
- `library_view.rs` 约 966、971、974 行「方向排名…失败」→「场景排名…失败」；
- `card_handler.rs` 约 494 行（`start_full_scan` 没配置 AI 时的报错）「设置 → 引擎」→「设置 → AI 设置」；
- `local_api.rs` 约 490 行「请在工具箱里重新开启本地 API」→「请在「自动整理与备份」里重新开启本地接口」；
- `spend_guard.rs` 里额度拒绝的几条提示（约 424、432、437 行一带，**以及卡 34 新加的 AI 精选那几条，如果已经合并进来**）：「工具箱」→「自动整理与备份」，「全量整理（不限额）」→「一次性全部整理」，「AI 额度」→「每天额度」，「AI 自动整理」→「每天自动整理」。**`REFUSAL_PREFIX`（`AI 额度：`）和 `is_refusal` 的判断逻辑不许动**（前端和 `solve_rerank` 靠它认「额度拒绝」）：提示正文里换词，前缀不换；
- `lib.rs` 的「运行检查」（`check_runtime_readiness_worker` 约 3450 行起及其测试）：把「GitHub Stars API 可用…」「AI 标签网络未检测」「真实请求：GET /user/starred…」这类开发口吻的句子改成大白话（例：「GitHub 连接正常」「AI 服务连接正常 / 没连上：原因」）；**去掉「相似推荐」「标签网络」两项 AI 探测**（对应功能页面已经看不到了，每次点「运行检查」白花 token），只留「AI 连接」一次探测；前端对应的两项结果名（卡 33 在 `automation-view.ts` 里改过的「AI 打标签」「AI 找相似工具」）一并去掉；改测试。**不要碰** `spend_guard::probe` 本身。

### 6. 导出的 CSV 加 UTF-8 BOM（`v2_ops.rs` 的 `export_csv`）
Excel（尤其 Mac 上双击打开）会把没有 BOM 的 UTF-8 中文 CSV 当成别的编码，中文乱码。导出的 CSV 文件开头写 3 个字节的 BOM（`EF BB BF`），**只给 CSV**，Markdown 不加。`write_export_file` 的临时文件 + 改名逻辑不改。补测试：文件前三个字节是 BOM、第一行是表头、行数不变、含引号 / 换行的单元格仍正确；在导出按钮的说明或悬停提示里说一句「用 Excel 打开中文不会乱码」。


## 不要做
- 不改额度判断、`REFUSAL_PREFIX`、`is_refusal`、`spend_guard::probe`、提示词、`PROMPT_VERSION`、迁移、`jobs.rs`、`ai.rs`；不新增命令、不新增依赖；前端只许改上面点名的 `automation-view.ts(.test.ts)`（去掉两项探测的结果名）。
- 不放宽、不删现有断言（把期望字符串换成新文案是对的；断言 `REFUSAL_PREFIX` 的不许动）。

## 验收
1. `tools/cards/run.sh rust-fmt`、`rust-check`、`rust-test`、`web-build`、`web-test` 全部通过，警告数不增；上面各条有对应测试（新句子逐字断言）。
2. `git diff --stat` 只出现「允许改动」里的文件。
3. 报告里写：条号 → 文件:行；「运行检查」改完后每一项检查的名字和文案；没把握的地方。

## 停止条件
- 需要改额度判断逻辑、提示词、迁移——停手并写进报告。
- 卡 34 / 35 的改动还没合并进来、导致 `spend_guard.rs` / `lib.rs` 冲突——停手并写进报告。
