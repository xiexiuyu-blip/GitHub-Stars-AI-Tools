---
编号: 33-copy-settings
允许改动:
  - apps/desktop/src/pages/settings.tsx
  - apps/desktop/src/pages/v2/toolbox.tsx
  - apps/desktop/src/App.tsx
  - apps/desktop/src/components/app-layout.tsx
  - apps/desktop/src/components/command-palette.tsx
  - apps/desktop/src/components/welcome-flow.tsx
  - apps/desktop/src/components/app-update-panel.tsx
  - apps/desktop/src/providers/app-update-provider.tsx
  - apps/desktop/src/features/sidebar/**
  - apps/desktop/src/hooks/use-settings.ts
  - apps/desktop/src/hooks/use-stars-workspace.ts
  - apps/desktop/src/hooks/use-ai-automation.ts
  - apps/desktop/src/lib/automation-view.ts
  - apps/desktop/src/lib/automation-view.test.ts
  - apps/desktop/src/lib/ai-config.ts
  - apps/desktop/src/lib/ai-config.test.ts
  - apps/desktop/src/lib/api.ts
  - apps/desktop/src/lib/api.test.ts
  - apps/desktop/src-tauri/src/v2_ops.rs
  - apps/desktop/src-tauri/src/lib.rs
---

# 卡 33 · 文案：设置 / 工具箱 / 欢迎引导 / 顶栏 / 命令面板 / 报错

## 为什么有这张卡
同卡 31：`docs/rebuild/文案审读.md`。这张卡处理**设置、工具箱（自动整理与备份）、欢迎引导、顶栏和 ⚡ 🔔 头像菜单、⌘K 命令面板、各处报错**：开发术语没翻译、说的和做的对不上、旧功能入口还在、花钱只写 token。

## Fox 能看到的效果
- 顶栏不再永远写「尚未同步」：显示「上次同步：昨天 18:17」；⚡ 菜单只有一项「给没有说明的收藏写中文说明」；🔔 里没有「仪表盘」；头像点了去「设置」；⚡ 里的搜索是「搜工具」、直接进资料库。
- 设置页的页签叫「GitHub / AI 设置 / 通用 / GitHub 备份 / 关于」，「向量」页签不再出现（代码留着）；工具箱叫「自动整理与备份」。
- 「一次性全部整理」的确认框用大白话讲清：要整理多少个、不受每天额度限制、最多 48 小时、随时能停、什么时候自动停；每天额度同样先说「每天最多整理约 N 个」；横幅里说「约整理了 N 个」；**不写人民币**（见审读文件「总控的决定」第 2 点）。
- 导出资料库导出**全部**，提示里写数量（「已导出 497 个工具到「下载」文件夹」）。
- 报错说人话：「…没读出来。你的资料都还在。点「重试」…」。

## 要做的条号（见 `docs/rebuild/文案审读.md`）
**3**（只做顶栏那一条「搜索仓库、标签或内容」→「搜工具、说明或关键词 ⌘K」）、**6、7、8、9、11、12、14、15、22、23、24、25、26、27、28、30、31、35、37、39、40**（40 里 Rust 那条「同步完成：…」消息可以改，`lib.rs` 里相关测试同步改；**不要动卡 29 新加的 README 跟进那些消息**）、**42、43、44、48**。
行为修复（都点名在审读文件里）：
- **第 8 条**：`v2_ops.rs` 导出 Markdown / CSV 去掉 50 / 200 的上限（导出全部有效仓库；注意一次写文件的方式别把内存撑爆，逐行写）；返回值带上导出数量，界面提示写数量；补 Rust 测试（造 300 个仓库导出，条数对得上）。
- **第 9 条**：顶栏的同步状态读 `syncedAt`（契约里已有）；没有才写「还没同步过」。
- **第 14 条**：拿掉三个旧入口（见上）；旧页面文件不动。
- **第 15 条**：藏起「向量」页签（只是不渲染，代码和命令都留着）。
- **第 37 条**：`api.ts` 里 `not found` 被翻成「这个功能还没接上」的问题：只在确定是「命令不存在」（invoke 的 `command ... not found` 这类，不是 GitHub 的 404 等业务报错）时才这么说；补 `api.test.ts` 测试。
- **第 39 条**：「约整理了 N 个」改读真实个数（`AutomationStatus` 里已有 `succeeded24h` 这类字段；先看 `automation-view.ts` 的现状，用现成字段，不要新增后端字段；没有现成字段就停手写进报告）。

## 通用规则（三张文案卡都一样）
- **文案的依据是 `docs/rebuild/文案审读.md`**：每一条写了「位置、现在的文字、问题、建议改成」。本卡列出的条号逐条做；建议改成的文字照用，不要自己再润色成别的说法（觉得不妥写进报告，别改）。
- **统一用词表**（同一份文件里）一律照用：工具 / 收藏（GitHub 的 Star）/ 资料库 / 收集箱 / 中文说明 / 整理 / 场景 / 交接单 / 同步 / 项目说明（README）/ 怎么试 / 类型 / 令牌（GitHub）与密钥（AI 服务）/ 钥匙串 / 你（不自称「我」）。改完后，你改到的文件里不许再出现表里「要替换掉的旧叫法」（旧页面 `pages/dashboard|repositories|rankings|tag-network|ai-search|profile.tsx` 不在范围内，别碰）。
- **只改文字，以及本卡点名要修的行为**。不改布局、配色、状态逻辑、命令名、JSON 键名（后端的 camelCase 键、`PackRequest` 字段、`task.mode` 等契约一个字都不许动）。
- **测试跟着文案走**：断言旧文案的测试改成新文案；不许为了过测试删断言或放宽断言（把期望字符串换成新文案是对的）。新增的行为（本卡明确要修的）要补测试。
- 前端文案里的括号一律用全角「（）」，引号用「」，数字大于一万写「X 万」，日期写「10 月 3 日 18:17」；应用对 Fox 说话用「你」。
- 不新增依赖；不联网；不读正式库；不碰 `PROMPT_VERSION`、提示词、`spend_guard.rs`、`ai.rs`、`jobs.rs`、迁移。
- 验收通用项：`tools/cards/run.sh rust-test`（有 Rust 改动时）、`web-test`、`web-build`、`rust-fmt`、`rust-check` 全过，警告数不增；`git diff --stat` 只出现允许改动里的文件；报告里列出「条号 → 改了哪些文件:行」，没做或做不到的条号写明原因。

## 验收（本卡补充）
1. 前端新增至少 16 个测试：同步状态文字（今天 / 昨天 / 更早 / 从没同步过）、确认框三句话的生成函数、横幅文字、导出提示、`describeInvokeError` 的几种报错（命令不存在 / 业务 404 / 网络）、`automation-view` 各状态按钮文案（统一后的）、AI 未配置提示、备份时间行、类型标签。
2. Rust 新增至少 4 个测试：导出全部（300 个）、导出数量回传、空库、含引号 / 换行的 CSV 转义不被破坏。
3. 设置页里不再出现「标签网络」「增量 AI 解析」「向量」「GSAT」「系统凭据管理器」「Personal Access Token」这些旧叫法（`grep` 给出命令输出）。

## 停止条件
- 要新增后端字段 / 命令、要动 `pages/v2/solve.tsx` / `components/solve/**` / `components/library/**` 等别的卡的文件——停手并写进报告。
