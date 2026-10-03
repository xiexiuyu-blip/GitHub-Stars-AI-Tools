# 交回报告 · 卡 13-ui-capture

## 一句话结论
收集页已按契约重写：纯函数 25 个测试、类型检查和前端打包都通过。没有启动 App，界面点击没有亲手验证。

## 验收逐条
| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| 1. `web-build`、`web-test` 通过；`capture-view.test.ts` 至少 14 个用例，覆盖每个纯函数和列出的边界 | 是 | `tools/cards/run.sh web-build` 退出码 0（`tsc --noEmit` 后 vite 2157 个模块，657ms 打完）。`tools/cards/run.sh web-test`：9 个文件、101 个测试通过，其中 `capture-view.test.ts` 25 个。边界在该文件：`describeCommit` 91–128 行，`buildSourceContext` 200 字与非法来源 132–158 行，`previewSummary` 162–170 行，`rowLabel` 182–202 行，`noteError` 500/501 且汉字 1500 字节仍通过 275–285 行，`shouldPreview` 7/8 字 288–297 行，`isTooLong` 中文 3 字节边界（65535 假、65536 假、65537 真、21846 个「测」真）300–314 行，`archiveAction` 两个方向 252–260 行。 |
| 2. 粘贴、预览、汇总、状态、主按钮、加入、防重复、收集箱、备注、归档、打开、空状态各自的文件:行 | 是 | 见下面「界面行为位置」。 |
| 3. 页面没有写死的十六进制颜色；只有一个主按钮 | 是 | 对两个 tsx 文件搜 `#[0-9a-fA-F]{3,8}` 没有匹配。唯一 `variant="primary"` 在 `apps/desktop/src/components/capture/preview-card.tsx:73`。验收那条命令把目录当文件时，macOS grep 在 stderr 打印 `Is a directory`、退出码 2，stdout 没有颜色匹配。 |
| 4. `git diff --stat` 里只出现「允许改动」里的路径 | 是 | `git diff --stat` 只有 `apps/desktop/src/pages/v2/capture.tsx`（321 增、58 删）。新文件还没进索引，`git status --short` 是 `?? apps/desktop/src/components/capture/`、`?? apps/desktop/src/lib/capture-view.ts`、`?? apps/desktop/src/lib/capture-view.test.ts`，都在允许改动里。 |
| 5. 写清哪些逻辑因后端命令还没合并而无法实际验证，以及兜底 | 是 | 见「没做到的」和下面兜底说明。本工作区 `lib.rs` 里已有 `preview_capture` / `list_inbox` / `set_inbox_status` / `set_inbox_note` 的命令入口，但按红线没有启动 App，没有用真库或假后端点过。 |

## 界面行为位置
- 页头「收集」和副标题：`apps/desktop/src/pages/v2/capture.tsx:285-288`。单列纵向：同文件 284 行，没有断点。
- 粘贴卡：大文本框最小高 150px、占位「粘贴一段聊天记录或帖子」在 `apps/desktop/src/components/capture/paste-card.tsx:15-22`。灰字「来源」和微信群 / X / 论坛 / 其他下拉在 24–40 行。备注最宽 280px、占位「备注，比如：某某群 10/03」在 41–48 行。页面挂上这张卡：`capture.tsx:289-297`。
- 预览防抖 300 毫秒：`capture.tsx:28` 和 `capture.tsx:102`。不够 8 个字不请求并清空：`capture.tsx:73-78`，规则 `apps/desktop/src/lib/capture-view.ts:144-146`。超过 64KiB 不请求：`capture.tsx:66-71`，规则 `capture-view.ts:152-159`。新请求序号加一，旧请求晚到丢掉：`capture.tsx:65`、`89`、`96`。太长文案「这段文字太长，请分几次粘贴」：`preview-card.tsx:23-28`，常量 `capture-view.ts:18`。
- 汇总行：文案 `capture-view.ts:71-78`，渲染 `preview-card.tsx:70-71`。
- 四种预览标签：规则 `capture-view.ts:97-103`（新蓝、已在 Stars 灰、已在收集箱灰、将被拦下红并带原因）。渲染 `preview-card.tsx:90-102`。图标名 `capture-view.ts:105-109`，画在 `preview-card.tsx:95-96`。新蓝底、拦下红底：`preview-card.tsx:107-110`。
- 唯一主按钮「加入收集箱（N）」：`preview-card.tsx:72-79`。没有新链接时去掉括号：`capture-view.ts:85-88`，是否可点 `capture.tsx:133-134` 和 `capture-view.ts:81-83`。
- 加入成功：toast 用 `describeCommit`（`capture-view.ts:53-61`），清空文本和预览，筛选切回待处理并刷新第一页：`capture.tsx:144-155`。失败 toast `describeInvokeError`，不清空文本：`capture.tsx:156-157`。来源上下文：`capture.tsx:140`，规则 `capture-view.ts:63-68`。
- 重复点击：`submittingRef` 在 `capture.tsx:56`、`137`、`141`、`159`。按钮 `loading` 在 `preview-card.tsx:74`，kit 的 Button 在 loading 时禁用。提交中粘贴区不可改：`capture.tsx:293`。
- 收集箱筛选：默认待处理 `capture.tsx:41`。`CardHeader` 加带个数的 `Segmented`：`inbox-card.tsx:46-56`，选项文案 `capture-view.ts:112-118`。切筛选重新取第一页：`capture.tsx:109-131` 和 `164-168`。每页 100 条：`capture.tsx:29`、`117`。有更多时「加载更多」：`inbox-card.tsx:59-63`，请求 `capture.tsx:171-191`（按 id 去重 `capture-view.ts:194-207`）。
- 备注内联：悬停或键盘聚焦出「备注」`inbox-card.tsx:190-191`。点开变成输入框，回车保存、Esc 取消：`inbox-card.tsx:133-166`，进入编辑 `capture.tsx:194-198`。超过 500 字红字且不提交：`capture.tsx:200-216`，规则 `capture-view.ts:139-142`，红字 `inbox-card.tsx:168-170`。保存调用 `setInboxNote`：`capture.tsx:220-233`。
- 归档与撤销：按钮文案 `capture-view.ts:128-131`，画在 `inbox-card.tsx:192`。成功后从当前列表移走：`capture.tsx:249-250`（计数 `capture-view.ts:209-228`）。待处理 / 已整理 / 已归档时 toast「已归档」加「撤销」：`capture.tsx:252-257`，撤销写回原状态并重新拉列表 `capture.tsx:274-280`。从归档点「恢复」toast「已恢复」同样可撤销：`capture.tsx:260-265`。
- 打开链接：`inbox-card.tsx:193-201` 的 `<a target="_blank" rel="noopener noreferrer">`。只放行 http/https：`capture-view.ts:190-192`。系统浏览器沿用未改的 `apps/desktop/src/App.tsx:114-117`：点外链先 `open_external_url`，失败再 `window.open`。
- 空状态：预览「还没有链接 / 把群聊或帖子粘贴到上面，链接会在这里列出来」`preview-card.tsx:53-61`。收集箱待处理「收集箱是空的 / 上面粘贴后点「加入收集箱」，新链接会进来」`capture-view.ts:180-186`，渲染 `inbox-card.tsx:84-86`。其他筛选「这个分类下没有内容」`capture-view.ts:187`。
- 收集箱状态标签：待处理灰且转圈、已整理蓝、失败红、已归档灰：`capture-view.ts:121-126`，画在 `inbox-card.tsx:177-188`。
- 调用失败兜底：预览失败停在预览卡并给「重试」，文案走 `describeInvokeError`（命令不存在会变成「这个功能还没接上。」）：`capture.tsx:95-100`，`preview-card.tsx:32-38`。收集箱读取失败同样写在卡片里：`capture.tsx:122-126`，`inbox-card.tsx:68-74`。写入失败只 toast，不把页面打崩。

## 改了哪些文件
- `apps/desktop/src/lib/capture-view.ts`：收集页的纯函数（文案、截断、预览门槛、收集箱筛选和列表拼接）。
- `apps/desktop/src/lib/capture-view.test.ts`：25 个用例。
- `apps/desktop/src/pages/v2/capture.tsx`：整页重写，管防抖、过期结果、加入、收集箱刷新、备注和归档。
- `apps/desktop/src/components/capture/paste-card.tsx`：粘贴框、来源、备注。
- `apps/desktop/src/components/capture/preview-card.tsx`：汇总、主按钮、四态行、空状态和太长提示。
- `apps/desktop/src/components/capture/inbox-card.tsx`：筛选、列表、备注编辑、归档、打开。
- `apps/desktop/src/components/capture/index.ts`：上面三个组件的出口。

## 验证
- `tools/cards/run.sh web-build` → 退出码 0。`tsc --noEmit` 通过，vite 2157 个模块，约 657ms 打完。
- `tools/cards/run.sh web-test` → 退出码 0。9 个文件、101 个测试通过；`src/lib/capture-view.test.ts` 25 个通过。
- `grep -nE '#[0-9a-fA-F]{3,8}' apps/desktop/src/pages/v2/capture.tsx apps/desktop/src/components/capture/*.tsx apps/desktop/src/components/capture/*.ts` → 无匹配，退出码 1。
- 验收原文那条把目录传给 grep：stderr `grep: apps/desktop/src/components/capture: Is a directory`，退出码 2，stdout 无匹配。
- `grep -n 'variant="primary"'` 在页面和 `components/capture` 里只有 `preview-card.tsx:73`。
- `git diff --stat` → 只有 `apps/desktop/src/pages/v2/capture.tsx`。`git status --short` 里的未跟踪文件都在允许改动内。
- 没有跑 Rust。这张卡没有改 Rust。
- 没有启动 App，没有在浏览器里点过。

## 没做到的 / 偏离任务卡的地方
- 没有在界面上走通粘贴、预览、加入、筛选、备注、归档、打开。红线不许启动 App，测试环境也没有 jsdom，不能加依赖做组件测试。总控需要用假后端点一遍。
- `preview_capture`、`list_inbox`、`set_inbox_status`、`set_inbox_note` 没有用真数据打过。失败时预览和收集箱列表把 `describeInvokeError` 写在卡片里（含「这个功能还没接上。」），并给「重试」；加入、备注、归档、撤销失败只 toast。加入失败不清空文本框。
- 从「失败」归档后没有「撤销」。后端 `set_inbox_status` 拒绝把状态写回 `failed`。这种条目只 toast「已归档」。
- 从归档恢复时 toast 是「已恢复」，仍带「撤销」（撤销会写回 `archived`）。任务卡只写了归档方向的「已归档」。
- 加入成功后清空文本和预览，来源和备注留着，方便同一群再贴一段。任务卡没有要求清来源。
- 筛选按钮文案用「待处理 4」这种「名称加空格加数字」。任务卡只要求带个数，没有规定括号。
- 备注按输入原文的 Unicode 码位计数，不先去掉首尾空白。后端 `set_inbox_note` 是先 trim 再数 500 字。前端更严：501 个字符即使首尾是空格也会拦住，不提交。
- 来源上下文最长 200 字，按任务卡截断。后端 `commit_capture` 还会再截到 2000。前端先截到 200。
- `kind` 为 `github`（GitHub 页面，不是仓库）时图标用 `link`。仓库才是 `inventory_2`，网页才是 `language`。
- 窄屏时四个筛选可能挤出卡片头。`CardHeader` 固定高 48px 且不换行，本卡不许改 kit。页面本身是单列，可以整页滚动。

## 需要总控或 Fox 决定的事
- 无。没有加依赖，没有改契约、后端、`App.tsx` 或卡 10 的积木。

## 给审核者的提示
- 打开链接不要在收集页里再找 `invoke('open_external_url')`。页面放的是普通外链，现有 `App.tsx:114-117` 会接住。假后端验收时确认这个监听还在。
- 预览结果必须对得上当前文本才允许加入。防抖期间按钮不出现（预览卡显示「正在识别链接…」），避免用旧预览提交新文本。
- 收集箱「加载更多」用偏移。归档会在本地把这一条拿掉并改两侧计数；撤销则重新拉第一页。加载更多期间如果又有新条目插到最前，可能漏一条，去重只防重复 id。
- `git diff --stat` 看不到新文件，审核范围时请把 `git status` 里的未跟踪文件算进去。它们都在允许改动里。
- 汉字「测」按 3 字节测过 64KiB 边界：65536 字节不算太长（和 `inbox_view` 的 `text.len() > 64 * 1024` 一致），65537 才算。
