---
编号: 32-copy-library
允许改动:
  - apps/desktop/src/pages/v2/today.tsx
  - apps/desktop/src/pages/v2/library.tsx
  - apps/desktop/src/pages/v2/capture.tsx
  - apps/desktop/src/pages/v2/discover.tsx
  - apps/desktop/src/pages/v2/map.tsx
  - apps/desktop/src/components/library/**
  - apps/desktop/src/components/capture/**
  - apps/desktop/src/components/discover/**
  - apps/desktop/src/lib/item-format.ts
  - apps/desktop/src/lib/item-format.test.ts
  - apps/desktop/src/lib/library-filters.ts
  - apps/desktop/src/lib/library-filters.test.ts
  - apps/desktop/src/lib/capture-view.ts
  - apps/desktop/src/lib/capture-view.test.ts
  - apps/desktop/src/lib/discover-view.ts
  - apps/desktop/src/lib/discover-view.test.ts
  - apps/desktop/src/lib/community-view.ts
  - apps/desktop/src/lib/community-view.test.ts
  - apps/desktop/src/lib/codex-prompts.ts
  - apps/desktop/src/lib/codex-prompts.test.ts
  - apps/desktop/src/hooks/use-copy.ts
  - apps/desktop/src/hooks/use-regenerate.ts
  - apps/desktop/src-tauri/src/library_view.rs
  - apps/desktop/src-tauri/src/discover.rs
---

# 卡 32 · 文案：今日 / 资料库 / 收集 / 发现 / 地图

## 为什么有这张卡
同卡 31：`docs/rebuild/文案审读.md`。这张卡处理**五个主页面**里的文字：「待整理 / 已整理」三种口径、仓库 / 工具 / 项目混用、「收集」和「收藏」分不清、理由和标签没解释、地图页的按钮会直接改库。

## Fox 能看到的效果
- 今日 / 资料库 / 找方案行里「待整理」说成「待 AI 整理」，并解释是「还没有中文说明的收藏」；收集页没有永远是 0 的「已整理」页签。
- 「收集」只指放进收集箱，「收藏」只指在 GitHub 点 Star；发现页按钮「先放进收集箱」，成功后告诉他还要去 GitHub 点 Star 再同步。
- 每个工具详情里：类型 / 成熟度标签带分组名（「类型：应用」「成熟度：可用」），「已停更」不再显示英文 stale；「依据项目说明」；「复制「帮我装好并试用」给 Codex」讲清粘出去会发生什么。
- 「不要了」悬停时说明后果；「你常做「小红书」」改成准确的说法；各种空状态告诉他下一步。
- 地图页不再有会直接合并分类的按钮，页签叫「按类型看」，说明写清「目前只能看，不能改」。

## 要做的条号（见 `docs/rebuild/文案审读.md`）
**1**（收集页那一部分 + 今日 / 资料库 / 找方案行标签；找方案行标签在 `components/solve/solve-hit-row.tsx`，**那个文件归卡 31**，这里只改 `lib/library-filters.ts`、`pages/v2/today.tsx`、`lib/capture-view.ts` 等你文件里的）、**3**（除顶栏「搜索仓库…」那条，那条归卡 33；其余：整理新收藏…、495 个工具、你已经收藏了、有 1 个工具已不在资料库里、收集箱里的链接…）、**10**（拿掉合并按钮、改说明、页签改名「按类型看」；成功 / 失败提示分开样式；打开页面时**不要**再触发后台重新分类——如果这是现有行为，去掉它并在报告里说明；做不到就停手写报告）、**17、18**（前端那一半 + `library_view.rs` 里「你常做」的原句）、**19、20、21**（`discover.rs` 里相关原句一起改）、**32**（`use-regenerate.ts` 的提示和 `item-detail.tsx` 的「说得对吗」）、**33**（`detail-panels.tsx` 的说明 + `codex-prompts.ts` 里「复制一段话」的提示；**复制出去的那段给 Codex 的话本身不改**，只改界面上的说明）、**34**（类型名、小标签分组名、「可上生产」→「可正式使用」、补 `stale` → 「久未更新」；「中文友好」本卡不改）、**36**（资料库顶部一句「场景是自动猜的，可能不准」）、**38**（项目说明（README））、**45、46**（只改你文件里的：键位提示、复制失败提示、括号、数字和日期写法、「项目最近更新」）、**47**、**49**。
**另有三处展示小修**（都不需要后端新字段）：
1. 详情里「AI 说明」旁的「依据 README」：如果说明文字以「只有 GitHub 简介，没有 README：」开头，改显示「只依据 GitHub 简介，没读到项目说明，别当真」；否则显示「依据项目说明」。
2. 「注意」（用之前注意）整块为空时，显示一行灰字「项目说明里没提限制，不代表没有」，不要整块消失；「灵感」文字里出现「Fox」时显示成「你」。
3. 被硬截断的句子（以半个括号或「、」结尾）不处理——这个已经在后端提示词里解决，前端不用做。

## 通用规则（三张文案卡都一样）
- **文案的依据是 `docs/rebuild/文案审读.md`**：每一条写了「位置、现在的文字、问题、建议改成」。本卡列出的条号逐条做；建议改成的文字照用，不要自己再润色成别的说法（觉得不妥写进报告，别改）。
- **统一用词表**（同一份文件里）一律照用：工具 / 收藏（GitHub 的 Star）/ 资料库 / 收集箱 / 中文说明 / 整理 / 场景 / 交接单 / 同步 / 项目说明（README）/ 怎么试 / 类型 / 令牌（GitHub）与密钥（AI 服务）/ 钥匙串 / 你（不自称「我」）。改完后，你改到的文件里不许再出现表里「要替换掉的旧叫法」（旧页面 `pages/dashboard|repositories|rankings|tag-network|ai-search|profile.tsx` 不在范围内，别碰）。
- **只改文字，以及本卡点名要修的行为**。不改布局、配色、状态逻辑、命令名、JSON 键名（后端的 camelCase 键、`PackRequest` 字段、`task.mode` 等契约一个字都不许动）。
- **测试跟着文案走**：断言旧文案的测试改成新文案；不许为了过测试删断言或放宽断言（把期望字符串换成新文案是对的）。新增的行为（本卡明确要修的）要补测试。
- 前端文案里的括号一律用全角「（）」，引号用「」，数字大于一万写「X 万」，日期写「10 月 3 日 18:17」；应用对 Fox 说话用「你」。
- 不新增依赖；不联网；不读正式库；不碰 `PROMPT_VERSION`、提示词、`spend_guard.rs`、`ai.rs`、`jobs.rs`、迁移。
- 验收通用项：`tools/cards/run.sh rust-test`（有 Rust 改动时）、`web-test`、`web-build`、`rust-fmt`、`rust-check` 全过，警告数不增；`git diff --stat` 只出现允许改动里的文件；报告里列出「条号 → 改了哪些文件:行」，没做或做不到的条号写明原因。

## 验收（本卡补充）
1. 前端新增至少 14 个测试，覆盖：成熟度 / 类型 / 小标签的新文案（含 `stale`）、依据项目说明两种显示、「注意」为空的灰字、「灵感」里 Fox→你、收集页页签（没有「已整理」）、`capture-view` 各状态文字、`discover-view` 限流 / 空状态 / 「—」→「还没找过」、`community-view` 状态词、`codex-prompts` 界面说明、日期 / 数字的格式化函数。
2. `library_view.rs` / `discover.rs` 里改了原句的，对应 Rust 测试更新；新增至少 3 个。
3. 地图页：合并按钮已经没有；页面里不再引用「合并」的命令调用（命令本身留着，别删）。

## 停止条件
- 要改后端命令的签名 / 返回字段、要动 `components/solve/**`、`pages/v2/solve.tsx`、`pages/settings.tsx`、`pages/v2/toolbox.tsx`、`components/app-layout.tsx`（这些归别的卡）——停手并写进报告。
