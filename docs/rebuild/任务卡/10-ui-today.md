---
编号: 10-ui-today
允许改动:
  - apps/desktop/src/components/kit/**
  - apps/desktop/src/components/library/**
  - apps/desktop/src/lib/api.ts
  - apps/desktop/src/lib/api.test.ts
  - apps/desktop/src/lib/item-format.ts
  - apps/desktop/src/lib/item-format.test.ts
  - apps/desktop/src/hooks/use-list-keys.ts
  - apps/desktop/src/hooks/use-list-keys.test.ts
  - apps/desktop/src/pages/v2/today.tsx
  - apps/desktop/src/styles/kit.css
  - apps/desktop/src/main.tsx
  - apps/desktop/src/App.tsx
---

# 卡 10 · 界面基础件 + 「今日」页

## Fox 能看到的效果
打开开发版，点「今日」：顶部是「早上好，Fox」和一句人话（这周收藏了几个、几个在等 AI 整理、先看哪几个）；下面四张统计卡片（本周新收藏 / AI 正在整理 / 想试 / 在用），**点哪张下面的列表就切成哪一类**；左边是项目列表（最上面一组「今天值得先看」，下面是「本周新收藏」），右边是选中项目的详情：先是 AI 的中文结论，再是它能做什么、适合解决什么、怎么试；「我的状态」五档（没看 / 想试 / 试过 / 在用 / 不要了）一点就改，改完弹出「已标为想试」并带「撤销」；右下角**唯一的蓝色主按钮「复制给 Codex」**，点一下就把一段能直接粘给 Codex 的话复制好。键盘 J/K 上下选、T 想试、X 不要了、C 复制给 Codex。**样式照原型**，同样的行、详情、状态条以后「资料库」页要原样复用，所以这张卡同时把这些共用的积木做好。

## 背景与先读什么
- **视觉与行为依据：原型**——`docs/rebuild/原型/template.html`。看：①样式表（第 43–308 行）里的 `.st`/`.stats`（统计卡片）、`.split`、`.lcard`/`.dcard`、`.lrows`/`.lgh`/`.lrow`/`.lc`/`.l1`/`.l2`/`.pendline`/`.lr`/`.hov`/`.lnote`（列表）、`.dbody`/`.dhead`/`.dt`/`.dm`/`.mystate`/`.aibox`/`.ailab`/`.conf`/`.bul`/`.sec`/`.sec-h`/`.tags`/`.code`/`details.more`/`.dfoot`（详情）、`.btn`/`.btn.pri`/`.btn.text`/`.ib`/`.tag`/`.seg`/`.chip`/`.empty`/`.spinner`/`.av`；②`pageToday`（第 544 行起）、`row`、`detail`、`toast`、`codexPrompt`（第 470–543 行）。**颜色、间距、圆角、字号用现有令牌**（`styles.css` 的 `@theme`，已经和原型的色板一致：品牌蓝 `bg-primary`、页面底 `bg-background`、卡片 `bg-card`、边框 `border-border`、正文 `text-foreground`、次要文字 `text-on-surface-variant` 等），尺寸照原型的像素值写（Tailwind 的任意值写法即可，比如 `h-8`、`px-[14px]`）。
- **后端契约（只读，不许改）**：`apps/desktop/src/lib/contracts.ts`。本页用到的命令：`get_today_view`、`list_library_view`、`get_library_detail`、`set_library_reading`、`copy_item_for_codex`、`confirm_library_card`、`regenerate_library_card`、`list_scenarios`。**这些命令的后端由别的卡实现，现在可能还没有合并**：本卡里界面照契约写，调用失败要有友好的提示（见下），不能崩。开发时没法用真数据看效果，总控会用假后端验收；你只需要保证类型检查、测试、构建通过，并且逻辑照契约写对。
- **图标**：用现有的 `components/ui/icon.tsx`（Material Symbols，字体已随应用打包），**不要引入新图标库，不要用 lucide**。原型里的 Lucide 图标对应的 Material 图标名自己挑（例如 sparkles→`auto_awesome`、copy→`content_copy`、x→`close`、bookmark-plus→`bookmark_add`、external-link→`open_in_new`、arrow-left→`arrow_back`、chevron-right→`chevron_right`、inbox→`inbox`、check→`check`、circle-check→`check_circle`）。
- 现状：`apps/desktop/src/pages/v2/today.tsx`（旧的骨架页，约 60 行，会被整个替换）；`App.tsx` 里怎么挂载 `TodayPage`（它传了 `onOpenLibrary`/`onOpenCapture`/`onOpenSolve` 三个回调，新页保留这个 props 签名，不用的就忽略）；`components/ui/icon.tsx`；`lib/reading-status.ts`（旧的 8 个状态值，新页只用契约里的 5 个状态组，**不要改这个文件**）。
- 测试环境：vitest 的 node 环境，**没有 jsdom 也没有 testing-library，也不许加**。所以**逻辑必须写成不依赖 React 的纯函数并测试**；组件只负责把数据画出来。

## 要做的事

### 1. 界面基础件 `components/kit/`（每个一个文件，加一个 `index.ts` 统一导出）
`PageHeader`（标题 20px 加副标题 14px 次要色加右侧操作槽）、`Card`/`CardHeader`（白底、1px 边框、8px 圆角；`CardHeader` 高 48px，左标题 16px、计数、`flex-1` 占位、右侧操作槽）、`Button`（`variant`: `primary` 蓝底白字 / `default` 白底细边框 / `text` 文字链接式；`size`: `md` 32px 高 / `sm` 28px 高；`loading`、`disabled`；**一个页面里主按钮只能有一个**，在组件注释里写明）、`Tag`（`tone`: `gray` / `blue` / `red`）、`StatCard`（`label`、`value`、`active`、`onClick`，激活态蓝色边框）、`Segmented`（分段控件，`options`、`value`、`onChange`；原型的 `.seg`；`components/sub-tabs.tsx` 保持不动）、`EmptyState`（图标、一句粗体、一句说明、可选按钮）、`RepoAvatar`（28px 或 40px 圆角方块，首字母，浅灰底）、`Spinner`、`Kbd`、`ToastProvider` + `useToast()`（见下）。样式写在组件里（Tailwind），共用的少量 CSS（比如 spinner 动画）放新文件 `styles/kit.css`，并在 `main.tsx` 里引入。
- **Toast**：`useToast().show({ title, sub?, actions?: { label, onClick }[], durationMs? })`，屏幕底部居中，可叠多条；左边一个对勾圆图标，中间标题（和可选的次要说明），右边是文字按钮（点了就执行并关掉这条）；默认 4.6 秒后自动消失；`role="status"`。`ToastProvider` 要包在应用最外层（改 `main.tsx` 或 `App.tsx`，只改这一处）。**纯函数部分**（比如「加入一条、按 id 移除、超过 4 条挤掉最旧的」）写成 `components/kit/toast-state.ts` 并测试。

### 2. 共用的列表与详情 `components/library/`
- `ItemRow`：**照原型 `row`**。左 28px 头像、中间两行（第一行仓库名 14px 加「owner · ★星数 · 语言」12px 灰字；第二行是 `oneLiner`，没有理解卡时显示转圈加「AI 正在整理 · {description}」）、可选的第三行 `why`（12px 次要色）、右边状态标签（`want`/`in_use` 蓝色标签，`in_use` 带对勾；`tried`/`dropped` 灰色；`unseen` 不显示）；选中行浅蓝底加左侧 3px 蓝条；**悬停时**右侧浮出两个小按钮「想试（T）」「不要了（X）」（盖住状态标签）。props 用契约里的 `LibraryItem`。
- `ItemDetail`：**照原型 `detail`**，props 是 `LibraryDetail | null`。`null` 显示空状态「选一个工具」。结构从上到下：头部（40px 头像、仓库名 20px、一行 12px 灰字的元信息：owner、星数、语言、最近更新「N 天前」、收藏于「月/日」，加形态/成熟度/「中文友好」标签）→ 「我的状态」五档分段控件（`Segmented`，值是 `ReadingGroup`，中文标签没看/想试/试过/在用/不要了）→ **AI 说明盒**（浅蓝底；标题「AI 说明」带 `auto_awesome` 图标，右侧灰字「依据 README」；正文是 `whatItIs` 的前 3 句；下面一行「说得对吗？[对] [不对，重新整理]」，`cardReview==='confirmed'` 时改成「✓ 你确认过了」；没有理解卡时改成转圈加「AI 正在整理这个仓库」和 GitHub 原文描述）→ 「它能做」（`capabilities` 最多 4 条，超过 46 个字符截断加「…」）→ 「适合解决」（`problemsSolved`）→ 「怎么试」（`howToTry` 用 `cleanHowToTry` 处理后非空就放进带复制小按钮的代码块，空就显示「README 里没有现成的安装步骤。点「打开 GitHub」看 Releases 或文档；也可以直接复制给 Codex，让它去读 README。」）→ 折叠的「更多信息」（GitHub 原文描述、话题标签 `topics`、适合的场景——场景名从 `list_scenarios` 的结果里按 `scenarioIds` 查）→ 底栏：左边文字按钮「打开 GitHub」（用现有的打开外链方式，见 `invoke('open_external_url')` 在旧代码里的用法；没有就用 `<a target="_blank">`），右边**主按钮「复制给 Codex」**。窄屏（宽度小于 900px）时有「返回」按钮（由页面传入的 `onBack` 控制是否显示）。
- `ReadingSegmented`：「我的状态」那一排，供详情和以后别的地方用。

### 3. 纯函数 `lib/item-format.ts`（带测试 `item-format.test.ts`，至少 12 个用例）
`formatStars`（小于 1000 原样；大于等于 1000 写成一位小数加 k 并去掉 `.0`：1200→`1.2k`、12000→`12k`）、`relativeDays(iso, now)`（今天 / 昨天 / N 天前 / N 个月前 / N 年前，规则照原型 `ago`；空值或坏值返回「—」）、`monthDay(iso)`（`M/D`）、`firstSentences(text, n)`（按「。」切，取前 n 句，补回句号；照原型 `sentences`）、`cleanHowToTry(text)`（空、含「摘录中未」「未提供」「没有提供」「未展示」时返回空串，否则原样）、`readingGroupLabel(group)`（五档中文）、`greetingFor(hour)`（5–10 点「早上好」、11–13「中午好」、14–17「下午好」、其余「晚上好」）、`codexFallbackText`（见下，万一后端命令不可用时的兜底文案，可选）。边界要测（0 点、整点、负数、坏日期）。

### 4. 键盘 `hooks/use-list-keys.ts`（带测试，至少 6 个用例）
纯函数 `nextSelection(ids, currentId, key)`：`j` 向下、`k` 向上，到头不绕圈，当前为空选第一个，列表为空返回 `null`；纯函数 `shouldIgnoreKey(target)`：焦点在 `input`/`textarea`/`select`/`contenteditable` 或有修饰键（⌘ Ctrl Alt）时忽略。hook 本身只负责监听 `keydown` 并回调 `onMove(id)`、`onStatus('want'|'dropped')`（T、X）、`onCopy()`（C）。

### 5. 接口封装 `lib/api.ts`（带测试 `api.test.ts`，至少 8 个用例）
给**所有**新界面页都要用的命令各写一个带类型的函数（类型一律从 `@/lib/contracts` 导入，函数名用驼峰）：`listScenarios`、`listLibraryView(filters)`、`getLibraryDetail(id)`、`setLibraryReading(ids, group)`、`getTodayView()`、`solveV2(query)`、`buildReferencePack(request, format, generatedOn?)`（命令名 `build_reference_pack_v2`，参数 `{ request, format, generatedOn }`；`PackRequest`/`PackOutput` 的类型在 `api.ts` 里按卡 03 的约定自己声明：`PackRequest { title, problem, architectureNotes?, items: {repoId, role: 'main'|'alternative'|'inspiration', why?}[], localCapabilities: {name, oneLiner, path}[] }`、`PackOutput { format, content, itemCount, missing: string[], warnings: string[] }`）、`copyItemForCodex(id)`（命令 `copy_item_for_codex`，参数 `{ repoId }`）、`previewCapture(text)`、`listInbox(status?, limit?, offset?)`、`setInboxStatus(id, status)`、`setInboxNote(id, note)`、`confirmLibraryCard(id)`、`regenerateLibraryCard(id)`，以及已有命令 `commitCaptureBatch(text, sourceContext)`（命令 `commit_capture_batch`，参数 `{ text, sourceContext }`，返回值类型照 `pages/v2/capture.tsx` 里的 `Commit`）。**命令名和参数名要和契约、后端卡一字不差**：参数是对象的（`filters`、`request`）就按对象传。为了可测，每个函数通过一个可替换的 `invoke` 引用调用（比如 `setInvokeForTests(fn)`），测试里替换它来断言「调用了哪个命令、传了什么参数」。再导出一个 `describeInvokeError(reason)`，把后端的字符串错误或 `Error` 变成给 Fox 看的一句话；如果错误里含「not found」「Command … not found」「没有这个命令」之类，就说「这个功能还没接上」。

### 6. 「今日」页 `pages/v2/today.tsx`（整个重写）
- 布局（桌面宽度）：页面外框 `flex-col gap-3`，内边距 16/20；顶部问候区；下面一排 4 张 `StatCard`；再下面是 `grid` 两列（左 340–460px 列表卡，右自适应详情卡），两列都撑满剩余高度并各自内部滚动；窄屏（小于 900px）只显示列表，点一项进入详情并出现「返回」。
- 问候：`greetingFor(当前小时)` + 「，」+ 名字（`get_github_auth_state` 的 `user.name`，没有就用 `Fox`）。下面一句：「这周你收藏了 {weekNew} 个新工具」+（`cardsPending>0` 时「，其中 {cardsPending} 个正在等 AI 整理」）+「。」+（有推荐时「先从下面 {picks 数} 个你可能用得上的看起。」，没有时「AI 整理好之后，这里会按你常做的事挑出值得先看的。」）。
- 统计卡片：本周新收藏（`counts.weekNew`）、AI 正在整理（`counts.cardsPending`）、想试和在用（用 `listLibraryView({ cardState: 'all', limit: 1 })` 返回的 `counts.reading.want` 和 `counts.reading.in_use`）。点哪张哪张高亮，下面列表对应：「本周新收藏」= `getTodayView()` 的「今天值得先看」（`picks`，**但已经被 Fox 标过状态的不显示**）加「本周新收藏」（`weekNew` 去掉已在 picks 里的）两组；「AI 正在整理」= `listLibraryView({ cardState: 'pending', sort: 'starred', limit: 80 })`；「想试」= `{ reading: 'want', cardState: 'all' }`；「在用」= `{ reading: 'in_use', cardState: 'all' }`。组标题照原型（「今天值得先看」带数量和灰字「从你还没看过的里，按你常做的事挑的」；其余组标题是对应卡片名加数量）。「今天值得先看」的每一行下面多一行 `why`：「适合：」+ 该项第一条 `problemsSolved`，没有就用第一条 `capabilities`，再没有就用 `oneLiner`（取详情里的字段：因为 `LibraryItem` 里没有这些，选中后才有详情，**列表里的 `why` 就只用 `oneLiner`**）。
- 选中项：默认选列表第一项；选中后调用 `getLibraryDetail(id)` 填右边详情（加载中显示简单骨架，失败显示 `describeInvokeError` 的话）。
- 改状态：行上悬停按钮、详情里的分段控件、键盘 T/X 都走同一个函数：先乐观更新当前列表和详情，调用 `setLibraryReading([id], group)`，成功后弹 `useToast`：「已标为「想试」」加项目名加「撤销」按钮（撤销就把状态改回原来的组，不再弹新 toast）；失败则回滚并弹错误 toast。改完要刷新统计卡片的数字。`dropped`（不要了）的项目从「想试/本周新收藏」之类列表里消失（因为按状态组筛选了）。
- 复制给 Codex：`copyItemForCodex(id)` 拿到文案后 `navigator.clipboard.writeText`，成功弹 toast「已复制，去 Codex 里粘贴」加项目名；剪贴板失败弹「浏览器不允许自动复制」加「请手动选中后按 ⌘C」（照原型 `copy`）。
- 「说得对吗？」：「对」调用 `confirmLibraryCard(id)` 并把详情里的 `cardReview` 改成 `confirmed`；「不对，重新整理」调用 `regenerateLibraryCard(id)`，成功弹 toast「已排队，整理好会更新」。任何一步失败都用 `describeInvokeError` 弹 toast，**不能让页面崩**。
- 空状态：照原型（「想试」为空时「还没有标成「想试」的工具 / 在右边点「想试」，它就会出现在这里」；其他为空「这里没有内容」）；数据加载中显示简单占位，不要白屏；加载失败显示一行错误和「重试」按钮。
- 页面里**只有一个主按钮**：详情底栏的「复制给 Codex」。

## 不要做
- 不改 `lib/contracts.ts`、不改后端（`src-tauri`）、不改 `pages/` 下别的页面、不改 `components/ui/**` 和已有的 `components/*.tsx`（`app-layout.tsx`、`sub-tabs.tsx` 等）；不新增依赖；不引入 lucide；不写死十六进制颜色（用令牌）。
- 不要启动 App，不要读写任何数据目录。

## 验收
1. `tools/cards/run.sh web-build` 通过；`tools/cards/run.sh web-test` 通过，新增测试合计**至少 26 个**（`item-format` ≥ 12、`use-list-keys` ≥ 6、`api` ≥ 8，另有 `toast-state` 的测试）。
2. `api.test.ts` 里逐个断言命令名和参数形状，至少覆盖：`listLibraryView` 传 `{ filters }`、`setLibraryReading` 传 `{ ids, group }`、`copyItemForCodex` 传 `{ repoId }`、`buildReferencePack` 传 `{ request, format, generatedOn }`、`commitCaptureBatch` 传 `{ text, sourceContext }`，以及 `describeInvokeError` 对「命令不存在」的识别。
3. 逐项列出 `ItemRow`、`ItemDetail`、`Toast`、统计卡片、问候区、四个筛选、改状态加撤销、键盘 J/K/T/X/C、复制给 Codex、「说得对吗」各自在哪个文件的哪一行（文件:行）。
4. 说明 `ToastProvider` 在哪一行挂到应用最外层，以及页面里主按钮（蓝色）只有一个（给出 grep 证据：`variant="primary"` 在 `today.tsx` 和它用到的组件里的出现位置）。
5. 页面没有任何写死的十六进制颜色（给出 `grep -nE '#[0-9a-fA-F]{3,8}' apps/desktop/src/components/kit apps/desktop/src/components/library apps/desktop/src/pages/v2/today.tsx` 的输出，应为空）。
6. `git diff --stat` 里只出现「允许改动」里的路径；`App.tsx` 的改动只有挂载 `ToastProvider` 这一处（如果改了 `main.tsx` 就不动 `App.tsx`）。
7. 报告里写清：哪些逻辑因为后端命令还没合并而无法实际验证，以及你为这些情形做了什么兜底。

## 停止条件
- 要改契约或后端、要新依赖、原型某处和本卡描述冲突、需要改卡外文件——停手并写进报告。
