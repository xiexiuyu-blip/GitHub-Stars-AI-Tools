# 交回报告 · 卡 10-ui-today

## 一句话结论
做完了：界面基础件、列表/详情和「今日」页已按契约接上，`web-test` 与 `web-build` 通过；没有启动 App，尚未合并的后端命令只做了失败兜底。

## 验收逐条
| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| 1. `tools/cards/run.sh web-build` 通过；`tools/cards/run.sh web-test` 通过，新增测试合计**至少 26 个**（`item-format` ≥ 12、`use-list-keys` ≥ 6、`api` ≥ 8，另有 `toast-state` 的测试）。 | 是 | `web-test`：8 files, 75 passed。新增 `it`：`item-format.test.ts` 22、`use-list-keys.test.ts` 8、`api.test.ts` 21、`toast-state.test.ts` 4，合计 55。`web-build`：`tsc --noEmit && vite build` 成功（2152 modules，产出 `today-*.js`）。环境有 `NO_COLOR`/`FORCE_COLOR` 警告，退出码仍是 0。 |
| 2. `api.test.ts` 里逐个断言命令名和参数形状，至少覆盖：`listLibraryView` 传 `{ filters }`、`setLibraryReading` 传 `{ ids, group }`、`copyItemForCodex` 传 `{ repoId }`、`buildReferencePack` 传 `{ request, format, generatedOn }`、`commitCaptureBatch` 传 `{ text, sourceContext }`，以及 `describeInvokeError` 对「命令不存在」的识别。 | 是 | `api.test.ts:53` `{ filters }`；`:66` `{ ids, group }`；`:105` `{ repoId }`；`:87` `{ request, format, generatedOn }`，`:96` 没写日期时 `generatedOn` 为 `null`；`:159` `{ text, sourceContext }`；`:176`–`:181` 把 `Command not found` / `not found` / `没有这个命令` / `命令不存在` 收成「这个功能还没接上。」。同文件还覆盖了本卡封装的其余命令。 |
| 3. 逐项列出 `ItemRow`、`ItemDetail`、`Toast`、统计卡片、问候区、四个筛选、改状态加撤销、键盘 J/K/T/X/C、复制给 Codex、「说得对吗」各自在哪个文件的哪一行（文件:行）。 | 是 | `ItemRow`：`components/library/item-row.tsx:8`，今日列表 `pages/v2/today.tsx:437`、`:453`。<br>`ItemDetail`：`components/library/item-detail.tsx:9`，挂载 `today.tsx:390`。<br>`Toast`：`components/kit/toast.tsx:24`（`ToastProvider`），`:70` 底部居中，`:74` `role="status"`；挤掉旧条在 `toast-state.ts:21`。<br>统计卡片：`components/kit/stat-card.tsx:3`，四张在 `today.tsx:372`–`:375`。<br>问候区：`today.tsx:370`（`greetingFor` + 名字），副标题 `:352`–`:358`（`buildTodayLead`）。<br>四个筛选：查询 `today.tsx:45`–`:48`，卡片 `:371`–`:375`，切换 `:342`。<br>改状态加撤销：`today.tsx:232` `changeReading`，成功 toast 与「撤销」`:241`–`:244`，撤销 `:265`；行上悬停 `:443`–`:444`、`:458`–`:459`；详情分段 `:397`。<br>键盘 J/K/T/X/C：`hooks/use-list-keys.ts:66`–`:82`，接入 `today.tsx:330`–`:339`，页脚提示 `:383`–`:386`。<br>复制给 Codex：`today.tsx:294`，按钮 `item-detail.tsx:202`–`:204`。<br>「说得对吗」：`item-detail.tsx:96`–`:98`；「对」`today.tsx:312`，「不对，重新整理」`:321`，接入 `:398`–`:399`。 |
| 4. 说明 `ToastProvider` 在哪一行挂到应用最外层，以及页面里主按钮（蓝色）只有一个（给出 grep 证据：`variant="primary"` 在 `today.tsx` 和它用到的组件里的出现位置）。 | 是 | 挂在 `main.tsx:12`–`:14`，位于 `AppErrorBoundary` 里面、`App` 外面。`App.tsx` 未改。`variant="primary"`：`today.tsx` 0 处；真正的按钮只有 `item-detail.tsx:202`（「复制给 Codex」）。`button.tsx:7` 是组件注释，任务卡要求在注释里写明一页只能有一个主按钮。重试、返回、确认都是 `variant="text"`。 |
| 5. 页面没有任何写死的十六进制颜色（给出 `grep -nE '#[0-9a-fA-F]{3,8}' apps/desktop/src/components/kit apps/desktop/src/components/library apps/desktop/src/pages/v2/today.tsx` 的输出，应为空）。 | 是 | 这条原文在 macOS 的 grep 上对目录报 `Is a directory`（不加 `-r` 不进目录），退出码 2，没有扫到文件内容。同一范围改用 `grep -nRE '#[0-9a-fA-F]{3,8}' …`：无匹配，退出码 1。`styles/kit.css` 同样无匹配。颜色用的是已有令牌（`bg-primary`、`text-on-surface-variant`、`bg-accent` 等）；spinner 用 `var(--color-input)` / `var(--color-primary)`。 |
| 6. `git diff --stat` 里只出现「允许改动」里的路径；`App.tsx` 的改动只有挂载 `ToastProvider` 这一处（如果改了 `main.tsx` 就不动 `App.tsx`）。 | 是 | `git diff --stat`：`apps/desktop/src/main.tsx`（6 行）、`apps/desktop/src/pages/v2/today.tsx`（535 行）。`App.tsx`、`lib/contracts.ts` 无差异。新文件尚未跟踪，`git status --short` 里全是允许改动路径：`components/kit/`、`components/library/`、`lib/api.ts`、`lib/api.test.ts`、`lib/item-format.ts`、`lib/item-format.test.ts`、`hooks/use-list-keys.ts`、`hooks/use-list-keys.test.ts`、`styles/kit.css`。 |
| 7. 报告里写清：哪些逻辑因为后端命令还没合并而无法实际验证，以及你为这些情形做了什么兜底。 | 是 | 见下方「没做到的」。本工作区 `src-tauri/src/lib.rs` 里能对上的只有 `commit_capture_batch`（约 1058）、`build_reference_pack_v2`（约 1128）、`copy_item_for_codex`（约 1140）、`get_github_auth_state`（约 1408）。`get_today_view`、`list_library_view`、`get_library_detail`、`set_library_reading`、`confirm_library_card`、`regenerate_library_card`、`list_scenarios`、`solve_v2`、`preview_capture`、`list_inbox`、`set_inbox_status`、`set_inbox_note` 没有对应函数。界面按契约调用；失败进 `describeInvokeError`，不把异常抛出渲染。 |

## 改了哪些文件
- `apps/desktop/src/main.tsx`：引入 `styles/kit.css`，在错误边界内用 `ToastProvider` 包住 `App`。
- `apps/desktop/src/pages/v2/today.tsx`：整页重写。保留 `onOpenLibrary` / `onOpenCapture` / `onOpenSolve`，函数体内 `void props`。
- `apps/desktop/src/styles/kit.css`：spinner 动画，颜色走 CSS 变量。
- `apps/desktop/src/lib/api.ts`、`api.test.ts`：可替换 `invoke` 的命令封装，以及 `describeInvokeError`。
- `apps/desktop/src/lib/item-format.ts`、`item-format.test.ts`：星数、相对日期、问候、今日导语、Codex 兜底文案、本周分组。
- `apps/desktop/src/hooks/use-list-keys.ts`、`use-list-keys.test.ts`：`j/k/t/x/c` 与忽略规则。
- `apps/desktop/src/components/kit/`：`PageHeader`、`Card`/`CardHeader`、`Button`、`Tag`、`StatCard`、`Segmented`、`EmptyState`、`RepoAvatar`、`Spinner`、`Kbd`、`Toast`、`toast-state` 及测试、`index.ts`。
- `apps/desktop/src/components/library/`：`ItemRow`、`ItemDetail`、`ReadingSegmented`、`index.ts`。

## 验证
- `tools/cards/run.sh web-test` → 8 files, 75 passed（约 303ms）。
- `tools/cards/run.sh web-build` → `tsc --noEmit` 通过，`vite build` 成功（约 659ms）。
- `grep -nRE '#[0-9a-fA-F]{3,8}'` 覆盖 kit、library、`today.tsx`、`kit.css` → 无匹配。
- `git diff --stat` 与 `git status --short` → 只有允许改动里的路径；`App.tsx` 无改动。
- 没有启动 App，没有在浏览器里点选、改状态或看窄屏。任务卡禁止 `pnpm dev:desktop` / `tauri dev` / `cargo run`。

## 没做到的 / 偏离任务卡的地方
- 界面没有用真窗口验收。加载、筛选、键盘、窄屏（小于 900px）、toast、复制，都只靠类型、构建和纯函数测试。Vitest 是 node 环境，没有 jsdom，也没有加 testing-library。
- 上面列出的、本仓库还没有的命令，调用结果无法用真后端核对。兜底是：
  - 今日主数据失败：副标题「今天的清单没有读出来。」，列表一行错误加文字按钮「重试」（`today.tsx:414`、`:502`）。四张数字停在「…」。
  - 某个筛选列表失败：同样一行错误加「重试」（`:418`）。列表里已经有条目时，错误条留在上面（`:430`）。
  - 详情失败：展示 `describeInvokeError` 的那句话加「重试」，重试只刷新这一条详情。
  - `list_scenarios` 失败：场景名留空，不把整页打成失败（`today.tsx:109`）。
  - `get_github_auth_state` 失败，或 `user.name` 去空白后是空的：名字用「Fox」（`:54`、`:114`）。不用 login。
  - 改状态失败：撤回这次乐观更新并 toast（`:252`）。写入成功但刷新数字失败：保留已改状态，再 toast，副文案「状态已经改了，数字可能还没刷新」（`:249`）。撤销成功不再弹成功 toast；撤销后刷新失败的副文案是「已撤销，数字可能还没刷新」（`:274`）；撤销写入失败会 toast，并把状态放回刚写上的那一档（`:276`）。
  - 复制给 Codex：只有错误被收成「这个功能还没接上」并且当前详情已经加载时，才改用 `codexFallbackText`（`today.tsx:301`）。别的错误只 toast，不拼兜底文案。剪贴板失败 toast「浏览器不允许自动复制」/「请手动选中后按 ⌘C」（`:289`）。本仓库已有 `copy_item_for_codex`，缺命令这条兜底只有后端真的报 not found 时才会走到。
  - 「对 / 不对」失败只 toast，不改 `cardReview`。确认成功把 `cardReview` 写成 `confirmed`；重新整理成功 toast「已排队，整理好会更新」，两个按钮留着。
- 「怎么试」只在已有理解卡时显示（`item-detail.tsx:132`）。还没有卡时，AI 说明盒已经写了整理完成后才会出现怎么试，避免在 README 还没读完时就说没有安装步骤。
- 点到和当前相同的状态时直接返回，不发请求，也不把它切回「没看」（`today.tsx:235`）。
- 提示条在屏幕底部居中（任务卡正文）。原型 template 把提示放在靠上的位置，这里跟任务卡。窄屏断点用 900px（任务卡正文）。
- 空列表的说明句跟原型 `template.html:556`：除了标题（想试用「还没有标成「想试」的工具」，其余用「这里没有内容」），说明都是「在右边点「想试」，它就会出现在这里」。任务卡括号里只点了非想试的标题。
- 打开 GitHub 用 `<a href target="_blank">`（`item-detail.tsx:190`），沿用 `App.tsx` 里已有的外链点击。今日页没有再调 `open_external_url`。
- 输入法组字（`isComposing`）时不处理快捷键。方向键不处理，只处理 J/K/T/X/C。
- `CardHeader` 已做出来，今日列表没有用它。原型这一页的列表没有卡头，留给后面的资料库。
- 任务卡原文那条不带 `-r` 的 grep，在这台 macOS 上扫不到目录里的文件。递归结果是空的，见验收 5。

## 需要总控或 Fox 决定的事
- 无

## 给审核者的提示
- 新文件都还没进 git。`git diff --stat` 只能看到 `main.tsx` 和 `today.tsx`，范围核对请用 `git status --short` 或 `tools/cards/check-scope.sh`。
- `grep variant="primary"` 会多命中 `button.tsx:7` 的注释。页面上蓝色实底按钮只有详情底栏那一个。
- 统计卡上的「本周新收藏」用的是服务端 `counts.weekNew`；下面分组标题的数字是当前画出来的条数。服务端计数和列表上限不一致时，两个数字可以不同。
- 「今天值得先看」只保留 `readingGroup === 'unseen'` 的推荐；本周其余条目去掉已出现在推荐里的，并去掉「不要了」。行上的「适合：」只用 `oneLiner`。
- 这一次会话里改过的状态记在页面的覆盖表里。后一次改状态如果先回来，不会被早一次的失败回滚盖掉。
- `TodayPage` 的三个打开回调仍留在类型里，避免改 `App.tsx` 的调用处。
