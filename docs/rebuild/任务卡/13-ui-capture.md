---
编号: 13-ui-capture
允许改动:
  - apps/desktop/src/pages/v2/capture.tsx
  - apps/desktop/src/components/capture/**
  - apps/desktop/src/lib/capture-view.ts
  - apps/desktop/src/lib/capture-view.test.ts
---

# 卡 13 · 「收集」页：粘贴群聊，先看后收，收集箱

## Fox 能看到的效果
点「收集」：一个大文本框，把群聊整段粘贴进去；下面选个来源（微信群 / X / 论坛 / 其他）、写个备注（比如「某某群 10/03」）；**粘贴后马上**在下方列出识别到的每条链接，每条标好会怎么处理——**新（蓝）/ 已在你的 Stars / 已在收集箱 / 将被拦下（红，写明原因，比如内网地址）**；顶上一行汇总「识别到 7 个链接：新 4 · 已在 Stars 2 · 已在收集箱 0 · 将被拦下 1」，唯一的蓝色主按钮「加入收集箱（4）」。点了以后弹出「新收 4 条，2 条已在你的 Stars 里，1 条是本机或内网地址，已拦下」，文本框清空，下面的**收集箱**多出 4 条。收集箱可以按「待处理 / 已整理 / 失败 / 已归档」筛选，每条能写备注、归档、在浏览器打开。这是 Fox 要的三个新功能之一「微信联动」的界面（微信工作台以后用本地接口往这里送链接）。

## 背景与先读什么
- **视觉与行为依据：原型** `docs/rebuild/原型/template.html` 的 `pageCapture`（第 643 行起）、`capPreviewHTML`、`capStatus`，以及样式表里的 `.caprow`、`.fic`、`.capsum`、`.nm`、`.u`、`.ta`、`.sel`、`.inp`。
- **卡 10 做好的积木（已在基线里，先读它们的 props 再用）**：`apps/desktop/src/components/kit/**`（`Card`/`CardHeader`、`Button`、`Tag`、`Segmented`、`EmptyState`、`Spinner`、`useToast`）、`apps/desktop/src/lib/api.ts`（`previewCapture`、`commitCaptureBatch`、`listInbox`、`setInboxStatus`、`setInboxNote`、`describeInvokeError`）。
- **后端契约（只读，不许改）**：`apps/desktop/src/lib/contracts.ts`：`CapturePreviewRow`、`CapturePreview`、`InboxItem`、`InboxPage`、`SetInboxStatusRequest`。`commit_capture_batch` 是已有命令，返回值类型是现有页面里的 `Commit`（`created`、`skipped`、`duplicateBatch`、`alreadyStarred`、`alreadyInInbox`、`blocked`、`linksFound`），文案规则看现有 `pages/v2/capture.tsx` 里的 `describeCommit`（**保留这套文案**，抽成纯函数）。**`preview_capture`、`list_inbox`、`set_inbox_status`、`set_inbox_note` 的后端由别的卡实现，可能还没合并**；调用失败要有友好提示，不能崩。你没法用真数据看效果，总控会用假后端验收；你要保证类型检查、测试、构建通过，逻辑照契约写对。
- 现状：`apps/desktop/src/pages/v2/capture.tsx`（66 行的旧页，整个替换；`App.tsx` 里挂载方式不变，**不需要改 `App.tsx`**）。
- 打开链接：现有代码里怎么在系统浏览器打开外链（搜 `open_external_url`），沿用；找不到就用 `<a target="_blank" rel="noopener">`。
- 测试环境：vitest 的 node 环境，没有 jsdom，也不许加依赖。**逻辑必须写成不依赖 React 的纯函数并测试**。

## 要做的事

### 1. 纯函数 `lib/capture-view.ts`（带测试，至少 14 个用例）
- `describeCommit(commit)`：沿用现有文案（没有链接→「没有找到链接。」；整段重复→「这段已经收过，没有重复写入。」；否则「新收 N 条，M 条已在你的 Stars 里，K 条已在收集箱，J 条是本机或内网地址，已拦下。」，0 的部分省略）。
- `buildSourceContext(source, note)`：`来源`加（有备注时）` · 备注`，去掉首尾空白，**最长 200 个字符**（超出截断，不报错）；来源只许是「微信群 / X / 论坛 / 其他」，别的值当「其他」。
- `previewSummary(preview)`：返回汇总文案「识别到 N 个链接：新 a · 已在 Stars b · 已在收集箱 c · 将被拦下 d」（0 个链接返回 `null`）；`canCommit(preview)`：新链接数大于 0。
- `rowLabel(row)`：GitHub 仓库显示 `owner/name`，其他显示去掉协议和结尾斜杠的规范化链接；`rowStatusTag(row)`：`{ tone: 'blue'|'gray'|'red', text }`（`new`→蓝「新」、`starred`→灰「已在你的 Stars」、`inbox`→灰「已在收集箱」、`blocked`→红「将被拦下」），`blocked` 时再返回原因文字；`kindIcon(kind)`：按 `kind` 返回 Material 图标名（仓库→`inventory_2`、X→`forum`、网页→`language`，其他→`link`）。
- `inboxFilterOptions(counts)`：四个选项（待处理/已整理/失败/已归档）和个数；`inboxStatusTag(status)`：`pending`→灰「待处理」（带转圈标记 `spinning: true`）、`ready`→蓝「已整理」、`failed`→红「失败」、`archived`→灰「已归档」；`archiveAction(status)`：`archived` 时按钮文案「恢复」目标状态 `pending`，否则「归档」目标 `archived`。
- `noteError(note)`：备注超过 500 个字符返回「备注最多 500 个字」，否则 `null`。
- `shouldPreview(text)`：去掉空白后至少 8 个字符才去预览（避免每敲一个字就请求）；`isTooLong(text)`：超过 64KB（按 UTF-8 字节数算）为真。

### 2. 页面 `pages/v2/capture.tsx`（整个重写）
- **布局**：`PageHeader`（「收集」，副标题「把群聊、帖子整段粘贴进来，我来找出里面的链接，并告诉你哪些是新的。」）→ 粘贴卡（大文本框，最小高 150px，占位「粘贴一段聊天记录或帖子」；下面一行：灰字「来源」+ 下拉（微信群/X/论坛/其他）+ 备注输入框（最宽 280px，占位「备注，比如：某某群 10/03」））→ 预览卡 → 收集箱卡。页面在窄屏下同样单列纵向，所以不需要断点处理。
- **预览**：文本变化后 300 毫秒防抖调用 `previewCapture(text)`（`shouldPreview` 为假就不请求并清空预览；`isTooLong` 为真直接显示「这段文字太长，请分几次粘贴」，不请求；**新请求覆盖旧请求，旧请求晚到的结果丢掉**）。预览卡顶部是汇总行（`previewSummary`）+ 右侧**页面唯一的主按钮「加入收集箱（N）」**（`canCommit` 为假时禁用，文案去掉括号里的数字）；下面逐条 `rowLabel` + 图标 + `rowStatusTag`（`blocked` 的显示原因）。没有链接：`EmptyState`「还没有链接 / 把群聊或帖子粘贴到上面，链接会在这里列出来」。
- **加入**：点按钮调用 `commitCaptureBatch(text, buildSourceContext(来源, 备注))`，成功后用 toast 显示 `describeCommit` 的文案，清空文本框和预览，刷新收集箱（回到第一页，并让筛选切到「待处理」）；失败用 `describeInvokeError` 弹 toast，**不清空文本框**。提交中按钮显示转圈并禁用，防止重复点击。
- **收集箱卡**：`CardHeader`（「收集箱」、该筛选下的个数、右侧 `Segmented` 四个筛选，选项文案带个数，默认「待处理」）；数据来自 `listInbox(status, 100, 0)`（切筛选时重新取；有更多时底部「加载更多」）；每一条：左边图标方块、中间两行（标题 = `title` 或规范化链接；第二行灰字「{来源上下文或平台} · {相对时间}」，有 `userNote` 时再显示一行备注）、右边 `inboxStatusTag`；悬停或键盘聚焦时显示操作：「备注」（点开变成内联输入框，回车或点「保存」调用 `setInboxNote`，`noteError` 非空时红字提示不提交，Esc 取消）、`archiveAction`（调用 `setInboxStatus`，成功后从当前列表移除该条并弹 toast「已归档」+「撤销」）、「打开」（在系统浏览器打开 `originalUrl`）。空状态：「收集箱是空的 / 上面粘贴后点「加入收集箱」，新链接会进来」（当前筛选不是「待处理」时说「这个分类下没有内容」）。
- 页面里**只有一个主按钮**：「加入收集箱」。

## 不要做
- 不改 `lib/contracts.ts`、不改后端、不改 `App.tsx`、不改别的页面、不改 `components/kit/**` 和卡 10 已有的文件（只允许在 `components/capture/` 里**新增**文件）；不新增依赖；不写死十六进制颜色（用令牌）；不引入 lucide。
- 不要启动 App，不要读写任何数据目录。

## 验收
1. `tools/cards/run.sh web-build`、`tools/cards/run.sh web-test` 通过；`capture-view.test.ts` **至少 14 个用例**，覆盖上面每个纯函数，含边界：`describeCommit` 的各分支；`buildSourceContext` 的 200 字截断和非法来源；`previewSummary` 的各数字；`rowLabel` 的仓库与网页；`noteError` 恰好 500 通过、501 报错；`shouldPreview` 的 7/8 个字符；`isTooLong` 对中文（3 字节）的字节计数边界；`archiveAction` 两个方向。
2. 逐项列出：粘贴卡与来源/备注、预览防抖与过期结果丢弃、汇总行、四种状态标签、主按钮、加入后的清空与刷新、重复点击防护、收集箱筛选与加载更多、备注内联编辑、归档与撤销、打开链接、各空状态各自在哪个文件哪一行（文件:行）。
3. 页面没有写死的十六进制颜色（给出 `grep -nE '#[0-9a-fA-F]{3,8}' apps/desktop/src/pages/v2/capture.tsx apps/desktop/src/components/capture` 的输出，应为空）；页面只有一个主按钮（给出 `variant="primary"` 的位置）。
4. `git diff --stat` 里只出现「允许改动」里的路径。
5. 报告里写清：哪些逻辑因为后端命令还没合并而无法实际验证，以及你做了什么兜底。

## 停止条件
- 要改契约或后端、要新依赖、卡 10 的积木不够用且必须改它们、需要改卡外文件——停手并写进报告。
