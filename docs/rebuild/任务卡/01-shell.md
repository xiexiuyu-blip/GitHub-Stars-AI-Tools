---
编号: 01-shell
允许改动:
  - apps/desktop/src/App.tsx
  - apps/desktop/src/styles.css
  - apps/desktop/src/styles/**
  - apps/desktop/src/components/app-layout.tsx
  - apps/desktop/src/components/app-shell.tsx
  - apps/desktop/src/components/sub-tabs.tsx
  - apps/desktop/src/components/ui/**
  - apps/desktop/src/lib/nav.ts
  - apps/desktop/src/lib/nav.test.ts
  - apps/desktop/src/providers/settings-provider.tsx
  - apps/desktop/src/types-settings.ts
  - apps/desktop/src/lib/theme-vars.ts
  - apps/desktop/src/lib/theme-vars.test.ts
---

# 卡 01 · 新外壳：换肤 + 窄导航条 + 5 项导航

## Fox 能看到的效果
打开开发版：窗口顶部是一条细的白色顶栏，左边一个蓝色小图标加「Fox Stars Lab」，中间是搜索框，右边是同步状态和头像；左边是一条窄的白色导航条，只有 5 个入口——**今日、资料库、找方案、收集、发现**——图标在上、文字在下，底部是**设置**；选中项浅蓝底蓝字。页面底色浅灰，卡片白底细边框小圆角，不再有毛玻璃和模糊。原来的「地图」「参考包」「工具箱」没有消失：地图在资料库页面顶部的二级切换里，参考包在找方案页面顶部的二级切换里，工具箱在设置页面顶部的二级切换里。

## 背景与先读什么
- **视觉依据**：`docs/rebuild/原型/template.html`（单文件原型）。看三处：顶部 `:root{...}` 色板（浅色和深色两套）；`.top`、`.rail`、`.ri`、`.card`、`.btn`、`.seg`、`.tag` 这些样式（约第 43–120 行）；窄屏时 `.rail` 移到底部的规则（约第 293 行）。原型里的图标是 Lucide，**应用里继续用现有的 `components/ui/icon.tsx`（Material Symbols），不要换图标库**。
- **现状**：`apps/desktop/src/components/app-layout.tsx`（目前是 260px 宽的毛玻璃侧栏，`NAV_ITEMS` 有 8 项）；`apps/desktop/src/App.tsx` 的 `renderPage()`（约第 299–359 行）按 `currentPage` 渲染各页；`apps/desktop/src/types.ts:38` 的 `AppPage` 类型；`apps/desktop/src/styles.css` 的 `@theme` 令牌；`apps/desktop/src/styles/design-system.css` 的 `.glass-*` 类。
- 其余背景见 `docs/rebuild/接手说明.md` §5 和 `docs/rebuild/原型/README.md`。

## 要做的事

### 1. 换色板（只改令牌的取值，令牌名不变，旧页面自动换肤）
在 `styles.css` 的 `@theme` 和 `.dark` 里，把 Material 风格令牌的取值换成原型的色板（令牌名一个都不要改，别处在用）：
- 页面底色（`--color-background`、`--color-surface`）→ `#F2F3F5`；卡片和弹层（`--color-card`、`--color-popover`、`--color-surface-container-lowest`）→ `#FFFFFF`；悬停底 → `#F5F6F7`；浅灰块 → `#EFF0F1`。
- 边框（`--color-border`、`--color-card-border`、`--color-outline-variant`）→ `#E4E5E7`；强一点的边框（`--color-outline`、输入框边框）→ `#D0D3D6`。
- 正文（`--color-foreground`、`--color-on-surface`、`--color-card-foreground`）→ `#1F2329`；次要文字（`--color-on-surface-variant`、`--color-muted-foreground`）→ `#646A73`。
- 品牌蓝（`--color-primary`、`--color-ring`）→ `#2E6BF0`，悬停 `#2558D8`；浅蓝底（`--color-accent`、`--color-primary-fixed`）→ `#E1EAFF`，其上的文字 `#245BDB`。
- 提醒红（`--color-error`、`--color-destructive`）→ `#D83931`，浅红底 `#FDE2E2`，其上文字 `#C02A26`；成功色 `#34C724`。
- 背景和卡片令牌里现在带透明度（`oklch(... / 0.85)`），改成不透明实色。
- `.dark` 用原型深色色板（页面底 `#161617`、卡片 `#1F1F21`、边框 `#2E2E31`、正文 `#EBEBEC`、品牌蓝 `#4C88FF`、提醒红 `#F76964`）。深色只要求可读，不要求逐像素一致。
- 圆角：卡片 8px，按钮和输入框 6px，小标签 4px（调整 `--radius` 系列令牌）。

### 2. 去毛玻璃、去网络字体
- `design-system.css` 里的 `.glass-sidebar`、`.glass-topbar`、`.glass-panel`、`.glass-card`：去掉 `backdrop-filter` 和半透明，改成实色白底或浅灰底加 1px 细边框；`.glass-card:hover` 去掉位移和大阴影，只留浅灰悬停底。**类名保持不变**（页面里在用）。
- 去掉对 Google Fonts 文字字体的引用（`@import url('https://fonts.googleapis.com/...')`），字体改系统字体栈：`"PingFang SC","Noto Sans SC",-apple-system,system-ui,sans-serif`；数字字体用 `"Inter","PingFang SC",system-ui,sans-serif` 即可（Inter 没装也能回落）。**`index.html` 里的 Material Symbols 图标字体引用不要动**（它不在允许改动里，另有卡处理离线图标）。
- 字号体系：12 / 14 / 16 / 20 / 28。

### 3. 外壳布局（在现有 `app-layout.tsx` 上改造，不要整体重写）
- 顶栏高 46px：左边蓝色圆角小图标 + 「Fox Stars Lab」；中间搜索框样式的按钮（点击打开现有命令面板，快捷键提示 ⌘K）；右边同步状态点和文字、「同步」按钮、用户头像（点击进入现有的 `profile`）。
- 左侧导航条宽 72px，白底，右侧 1px 边框；每个入口是图标在上、文字在下（文字 12px），选中项浅蓝底、蓝字，悬停浅灰底；底部是「设置」。
- **功能入口一个都不能少**：同步、抓取 README、增量 AI 解析、生成标签网络、检查更新、全局搜索、命令面板（⌘K）、通知、任务进度条、失败重试、用户入口。原来在侧栏里的，放进顶栏右侧的「快速操作」菜单（现在就有这个菜单，保留）；任务进度条保留在内容区顶部。
- 窄屏（窗口宽度小于 900px）：导航条移到窗口底部，变成一排横向的入口（参考原型第 293 行附近的做法）。

### 4. 导航数据与二级切换
- 新增 `apps/desktop/src/lib/nav.ts`（纯数据，不引 React）：
  - `NAV_ITEMS`：恰好 5 项，顺序固定：`today` 今日、`library` 资料库、`solve` 找方案、`capture` 收集、`discover` 发现；`SETTINGS_ITEM`：`settings` 设置。每项含 `key`、`label`、`icon`（Material Symbols 图标名）。
  - `SUB_TABS`：`library` → 「资料库」(`library`)、「地图」(`map`)；`solve` → 「找方案」(`solve`)、「参考包」(`packs`)；`settings` → 「设置」(`settings`)、「工具箱」(`toolbox`)。
  - `navKeyFor(page: AppPage)`：返回该页面应高亮哪个导航项：`map→library`、`packs→solve`、`toolbox→settings`；`today/library/solve/capture/discover/settings` 返回自己；其余旧页面（`dashboard`、`repositories`、`rankings`、`tag-network`、`ai-search`、`profile`）返回 `null`（不高亮任何项）。
- 新增 `apps/desktop/src/components/sub-tabs.tsx`：分段控件样式（参考原型 `.seg`：灰底圆角容器，选中项白底加轻阴影），props 为 `{ tabs, active, onSelect }`。
- 在 `App.tsx` 里，当当前页属于某个二级切换组时，在页面内容**上方**渲染 `SubTabs`（外层包装，**不要改 `pages/**` 里任何文件**）。
- 新增 `apps/desktop/src/lib/nav.test.ts`（vitest）：至少覆盖——`NAV_ITEMS` 恰 5 项且顺序、标签正确；`navKeyFor` 对 12 个页面返回正确值；`SUB_TABS` 里每个页面的 `navKeyFor` 都等于它所在的组；`NAV_ITEMS` 里没有 `map`、`packs`、`toolbox`。

## 不要做
- 不改 `apps/desktop/src/pages/**`、`features/**`、`src-tauri/**`、`packages/**`、`index.html`。旧页面里残留的 `backdrop-blur`、写死的颜色，**只统计不改**：在报告里给出各文件的出现次数。
- 不换图标库、不新增依赖、不改任何后端命令和数据逻辑。
- 不删除任何页面或路由；`AppPage` 类型里的页面一个都不能少。

## 验收
1. `tools/cards/run.sh web-build` 通过；`tools/cards/run.sh web-test` 通过，且 `nav.test.ts` 至少 4 个用例。
2. `NAV_ITEMS` 恰好 5 项、顺序为 今日/资料库/找方案/收集/发现，设置在底部；导航条渲染出来的入口里没有「地图」「参考包」「工具箱」（给出 `app-layout.tsx` 对应行号）。
3. 二级切换：给出 `App.tsx` 里渲染 `SubTabs` 的位置；说明资料库↔地图、找方案↔参考包、设置↔工具箱切换时，左侧导航高亮分别保持在 资料库、找方案、设置。
4. 色板：给出 `styles.css` 里主色 `#2E6BF0`、页面底 `#F2F3F5`、卡片 `#FFFFFF`、边框 `#E4E5E7`、正文 `#1F2329`、提醒红 `#D83931` 的所在行；`design-system.css` 与 `styles.css` 里的 `.glass-*` 和 `@theme` 区域不再有 `backdrop-filter`（给出 grep 命令和输出）；整个 `src` 里不再有 `fonts.googleapis.com/css2?family=Manrope|Inter|JetBrains` 的引用（给出 grep 输出）。
5. 尺寸：顶栏 46px、导航条 72px 在代码里的位置；窄屏（<900px）导航条移到底部的实现位置。
6. 功能入口清单：逐项写出「同步、抓取 README、增量 AI 解析、生成标签网络、检查更新、全局搜索、命令面板、通知、任务进度条、失败重试、用户入口」在新布局中的位置（文件:行）。
7. 报告里有「卡外残留」一节：`pages/**`、`features/**` 里 `backdrop-blur` 和写死颜色的出现次数（分文件），供下一张卡处理。
8. `git diff --stat` 里只出现「允许改动」里的路径。

## 停止条件
- 需要改卡外的文件才能完成；需要新依赖；发现某个入口在新布局里放不下——停手，在报告里写清楚，不要自己拍板删功能。
