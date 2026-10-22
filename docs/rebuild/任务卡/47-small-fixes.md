---
编号: 47-small-fixes
允许改动:
  - apps/desktop/src/components/kit/stat-card.tsx
  - apps/desktop/src/components/kit/stat-card.test.tsx
  - apps/desktop/src/styles/kit.css
  - apps/desktop/src/lib/automation-view.ts
  - apps/desktop/src/lib/automation-view.test.ts
  - apps/desktop/src/**/*.test.ts
  - apps/desktop/src/**/*.test.tsx
  - apps/desktop/src-tauri/src/jobs.rs
---

# 卡 47 · 两个小修：深色模式下「今日」选中的统计卡看不见；全量整理的估算从每张 7500 改成 9000

> 执行者：Grok（见分配单）｜不加迁移、不加依赖。**两件事互不相干，各做各的。** 测试文件只许「因为常量变了而换数字」，不许删断言、不许放宽。

## Fox 能看到的效果
1. 切到深色模式，打开「今日」：第一张统计卡（默认选中）里的文字、数字**看得清**，选中的样子是「深色底 + 蓝色描边」，不再是一块白底配浅色字。
2. 「自动整理」页点「现在整理」前，预计花费按每张约 9000 token 估（原来 7500）。比如 388 个预计约 350 万；后台的「用量超过预计 × 4 自动停」的刹车线同步变宽，不会比估算先停。

## 事情 A · 深色模式统计卡

### 背景
`apps/desktop/src/components/kit/stat-card.tsx` 选中态的类名里写死了一个**很浅的底色**：`bg-[hsl(var(--h)_100%_98.5%)]`（亮度 98.5%，接近纯白）。深色模式下文字用 `text-foreground`（`#EBEBEC`，浅灰）和 `text-on-surface-variant`（`#A6A7AB`），落在近白底上，几乎看不见。另外同一张卡里的图标底 `.hue-tile`（`apps/desktop/src/styles/kit.css` 约 50 行）也是写死的浅底 `hsl(var(--h) 85% 94%)` + 深色图标，深色页面里是一块很亮的方块。
深色令牌在 `apps/desktop/src/styles.css` 的 `.dark { … }`（约 144 行起），Tailwind 的深色变体写法是 `@custom-variant dark (&:is(.dark *))`，所以类名里可以直接用 `dark:`。
`StatCard` 的颜色由 `--h`（色相，蓝 222、琥珀 36、紫 268、绿 148）决定，别的地方也在用这个写法（`kit.css` 的 `.scn-chip`、`.hue-tile`）。

### 要做的事
1. `stat-card.tsx`：选中态在深色模式下换成**深色底**，用同一个 `--h` 做色相，亮度压到 15%–20%、饱和度 35%–50% 之间（例如 `dark:bg-[hsl(var(--h)_40%_17%)]`，具体数值你定，但必须满足下面的对比度要求）。浅色模式的外观**一个像素都不许变**（原来的类名原样保留）。
2. `kit.css`：给 `.hue-tile` 加深色版本——`.dark .hue-tile { background: hsl(var(--h) 40% 24%); color: hsl(var(--h) 85% 78%); }`（数值可微调，原则：底比卡片底稍亮一点、图标是该色相的亮色，两者对比度 ≥ 3:1）。只加深色规则，不改现有浅色规则。
3. 对比度要求（在报告里写出你的计算）：深色选中态的卡面底色与 `#EBEBEC`（主文字）的对比度 ≥ 7:1，与 `#A6A7AB`（次要文字）的对比度 ≥ 4.5:1，四种色相（222 / 36 / 268 / 148）**都要算**。对比度用 WCAG 公式（相对亮度）自己算，可以写一个临时的 `node -e` 一行脚本，但**别把脚本留在仓库里**。

### 测试
新建 `apps/desktop/src/components/kit/stat-card.test.tsx`（照 `pages/v2/toolbox.test.tsx` 的写法，用 `?raw` 读源文件）：
1. 源码里选中态的类名同时包含原来的浅色底和一个 `dark:bg-[` 开头的深色底（**只有浅色底、没有深色底就失败**）；
2. `kit.css` 里有 `.dark .hue-tile` 规则；
3. 浅色规则 `.hue-tile { background: hsl(var(--h) 85% 94%);` 原样还在（浅色外观没被改）。
再用 `renderToStaticMarkup`（React 自带，不加依赖）渲染 `<StatCard label="x" value={1} icon="inbox" active />`，断言输出的 `class` 里有 `dark:bg-[`。

## 事情 B · 全量整理估算 7500 → 9000

### 背景
全量整理（Fox 在「自动整理」页确认后的那一轮）有两处「每张多少 token」的估算，约定**必须一致**：
- Rust：`apps/desktop/src-tauri/src/jobs.rs` 第 29 行 `const FULL_SCAN_TOKENS_PER_CARD: i64 = 7_500;`，用来算刹车线 `full_scan_ceiling`（`max(300 万, 总数 × 每张 × 4)`，约 27–29 行的注释和约 1132–1135 行的实现；测试在约 4295–4297 行，`assert_eq!(full_scan_ceiling(388), 388 * 7_500 * 4)`）；
- 前端：`apps/desktop/src/lib/automation-view.ts` 第 95–96 行 `export const FULL_SCAN_TOKENS_PER_REPO = 7500;`，用来给 Fox 看「预计花费」，约 148–150 行和 350 行附近有用到它的估算函数 `estimateFullScanTokens` 和面板文案。
切库后全量整理 500 张的实测是**每张约 9 千**（含思考），7500 偏低，面板上预计的数字会比实际少约两成。

### 要做的事
1. 两处常量都改成 9000（Rust 写 `9_000`），注释里的依据改成：「真实记录：切库后全量整理约 500 张，实测每张约 9 千（含思考）；Rust 的 `FULL_SCAN_TOKENS_PER_CARD` 与前端的 `FULL_SCAN_TOKENS_PER_REPO` 必须一致，改一处要同时改另一处。」不要写别的数字当依据。
2. **每日额度的折算不动**（前端 `TOKENS_PER_REPO = 6000` 只用来估「每天约整理多少个」，和全量整理无关，不要碰）。
3. 受影响的测试**只换数字、不改逻辑、不删断言**：
   - `automation-view.test.ts` 约 430–433 行那一组（常量等于 9000；1 个 = 9000；388 个 = 3,492,000）和 `estimateFullScanTokens(1.2)` 等；
   - 约 429 行附近「388 个约 290 万」→ 重新算（388 × 9000 = 3,492,000，按现有的取整规则得到多少就写多少）；
   - 凡是用 12 个算出「预计约 9 万」的地方（`fullScanPanel` 的 `body` 和 `estimate`）→ 12 × 9000 = 108,000，按现有取整规则改；3 个的那处同理；
   - `jobs.rs` 约 4297 行改成 `388 * 9_000 * 4`，以及因此变化的其它断言；
   - 任何别的、因为这个常量而失败的测试（所以允许改 `apps/desktop/src/**/*.test.ts(x)`），**逐条点名**写进报告：文件:行、旧值 → 新值、为什么是这个新值（写出算式）。
4. 新增一条 Rust 测试（`jobs.rs` 的测试模块里）：`include_str!("../../src/lib/automation-view.ts")` 读出前端文件，从里面找到 `export const FULL_SCAN_TOKENS_PER_REPO = ` 后面的数字（手工切字符串，不加 `regex` 依赖），断言它等于 `FULL_SCAN_TOKENS_PER_CARD`。这样以后只改一边会立刻失败。路径是相对 `jobs.rs` 的：`apps/desktop/src-tauri/src/jobs.rs` → `apps/desktop/src/lib/automation-view.ts`。

## 不要做
- 不改 `spend_guard.rs`、`card_handler.rs`、`card_pipeline.rs`，不改任何不在「允许改动」里的文件，不动提示词和 `PROMPT_VERSION`。
- 不改「每日额度」档位和 `TOKENS_PER_REPO`；不改 `full_scan_ceiling` 的公式（只有常量变）。
- 不改浅色模式的任何外观；不写新的十六进制色值——深色规则里用 `hsl(var(--h) …)`，和现有写法一致。
- 不加依赖，不运行 `pnpm install`。

## 验收
1. `git diff` 里 `stat-card.tsx` 只**增加**了深色类名（原来的浅色类名一个字符都没少）；`kit.css` 只**新增**了 `.dark .hue-tile`。
2. 报告里有四种色相下的对比度计算表（底色、与主文字对比度、与次要文字对比度），数值满足上面的门槛。
3. `grep -rn "7500\|7_500" apps/desktop/src apps/desktop/src-tauri/src` 在非历史注释的地方不再出现（出现的话逐个说明为什么保留）。
4. `tools/cards/run.sh web-build`、`web-test`、`rust-fmt`、`rust-check`、`rust-test` 全部通过；报告里写前端测试项数、Rust 通过 / 失败 / 忽略数，和改动前的基线对比（先在改动前跑一次基线）。
5. 改坏检查（各做一遍，写「改了什么 → 哪条测试失败」）：① 把 `stat-card.tsx` 的 `dark:bg-[…]` 删掉；② 把 `FULL_SCAN_TOKENS_PER_REPO` 改回 7500；③ 把 `FULL_SCAN_TOKENS_PER_CARD` 改回 7_500；④ 把 `.dark .hue-tile` 规则删掉。每条改完要还原。

## 停止条件
- 对比度算出来达不到门槛又找不到合适的数值 → 停手，把算过的候选写进报告。
- 因为常量变化而失败的测试里，有需要改**逻辑**（不只是数字）才能通过的 → 停手，写进报告，不要硬改。
- 需要改「允许改动」以外的文件 → 停手写进报告。
