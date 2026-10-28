# 交回报告 · 卡 76-entry-refresh

## 一句话结论

卡内四项修复已完成，前端 44 个文件 / 921 项测试全部通过，类型检查和生产构建通过；未运行真实 App，等待总控只读审查和隔离页面验收。

## 验收逐条

| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| 未配置AI的“去设置AI”必须直达设置AI配置页，其他暂停/额度等入口仍去对应自动整理；最小参数接线，不重构设置。SolvePage的onOpenToolbox仅原样保留（另卡负责solve.tsx）。 | 是（代码与替身测试）；真实 App 待验收 | `App.tsx:202` 明确设置 AI 页签后进入 settings，`App.tsx:322/335/378` 接到今日、资料库与设置页；`settings.tsx:187` 默认 GitHub、外部可指定 AI；`automation-view.ts:415/426/497` 仅未配置 AI 带 `actionTarget`；横幅和 PendingNote 行为测试实际调用回调，验证 AI 与自动整理分别执行。 |
| 命令面板普通词Enter进行资料库搜索，方向键选择、Enter打开、Esc关闭，焦点管理与恢复；不触发真实同步/AI测试。避免旧搜索请求污染新查询结果。 | 是（组件处理器替身测试）；真实 DOM 待验收 | `command-palette.tsx:36` 保存/恢复焦点；`:46` 请求失效保护；`:110` 起 dialog、Tab 边界、Escape；输入框处理普通搜索/方向键/Enter，并忽略中文输入法确认。8 项组件行为测试覆盖普通词无候选、方向键、问句、命令、焦点、迟到成功/失败、关闭后响应。 |
| README成功null或旧结果缓存必须在同步/README更新后失效，并让仍打开的详情刷新。找到正确现有事件来源，针对性失效；失败可重试。不得靠每次渲染反复拉取。 | 是（hook 与事件订阅替身测试）；真实同步待验收 | `use-repo-readme.ts:27` 缓存失效同时通知订阅者；`:36` 仅同步/README任务收尾刷新；`:53` 防止旧响应覆盖新正文；`:19` 防止旧失败删除新缓存。`use-stars-workspace.ts:144` 接到现有 task-progress 监听。7 项测试覆盖缓存null/旧正文、并发共享、重复渲染、关闭重开、失败重试、迟到响应、目标过滤；另 1 项执行真实工作区订阅回调并检查取消监听。 |
| 自动整理费用文案将“每天”明确滚动24小时，关闭只表示后台自动整理关闭，不承诺全App免费；修最小误导，不改预算实际值/门禁。 | 是 | `automation-view.ts:99` 档位数值保持 0/100000/200000/600000，文案改为每24小时；`:107` 明确最近24小时滚动累计、不在零点重置、逐步恢复；关闭提示说明手动整理、AI问答等仍可能收费。既有预算/确认断言保留，增加窗口和关闭含义回归。 |
| 基于替身数据测试上述行为，不加依赖。App.tsx最后由总控补75的新问题消费回调，其他人不同时写此文件。发现实际任务进度provider路径不在允许列表时先告知总控扩范围。 | 是 | 初次发现 provider 文件不存在，已通知总控；总控扩卡至 `hooks/use-stars-workspace.ts` 与同名测试后才改动。无新依赖。未接卡75回调，未修改 solve.tsx / contracts / solve-view / solve-hit-row。 |

## 改了哪些文件

以下均位于本卡工作区 `apps/desktop/src/`：

- `App.tsx`：新增 AI 设置入口和页签参数，离开设置后恢复普通入口默认值。
- `pages/settings.tsx`：接收初始页签，保留默认 GitHub 与内部页签操作。
- `pages/settings.test.tsx`：验证默认/AI 页签和 App 接线。
- `components/command-palette.tsx`：普通搜索、键盘选择、Esc/Tab、焦点恢复、旧请求防护。
- `components/command-palette.test.tsx`：8 项替身行为测试。
- `hooks/use-repo-readme.ts`：共享缓存、失效通知、按任务收尾刷新、失败/旧请求保护。
- `hooks/use-repo-readme.test.ts`：7 项缓存/订阅/请求竞争测试。
- `hooks/use-stars-workspace.ts`：向已有 task-progress 监听增加 README 失效处理。
- `hooks/use-stars-workspace.test.ts`：1 项真实监听回调接入替身测试。
- `lib/automation-view.ts`：AI 提示目的地、滚动24小时与关闭含义。
- `lib/automation-view.test.ts`：更新改动文案的精确期望，新增3项含义/去向回归，不放宽预算断言。
- `components/library/automation-banner.tsx`：根据明确目标选择回调。
- `components/library/item-detail.tsx`：未整理提示直达相应设置；导出 PendingNote 供行为测试。
- `components/library/item-detail.test.tsx`：2 项真实按钮回调目的地测试。
- `pages/v2/today.tsx`：传递 AI 设置入口。
- `pages/v2/library.tsx`：传递 AI 设置入口。

`.card-report.md` 为本报告。任务卡允许范围由总控更新，开发者未改任务卡。生产构建生成的产物仅为现有构建流程输出。

## 验证

工作目录：`/Users/fox/WorkSpace/_infra/fox-stars-lab-wt/76-entry-refresh`。

- `tools/cards/run.sh web-test command-palette use-repo-readme automation-view` → 首次实际运行的是全 src 套件，43 文件、913 项中 910 通过 / 3 失败；失败均是费用文案变化后未同步的精确字符串期望，已据新含义更新，没有删掉断言。
- `tools/cards/run.sh web-test` → 第二轮 43 文件、920 项中 919 通过 / 1 失败；剩余一处“最近24小时已用”的精确期望遗漏，已修正。
- `tools/cards/run.sh web-test` → 最终 44 文件、921 项全部通过，退出码 0，耗时 2.27 秒。
- `tools/cards/run.sh web-build` → 两轮都通过；最终 TypeScript 项目检查、前端类型检查、Vite 构建退出码 0，2191 modules，Vite 用时 524ms。
- 没有改 Rust，因此未运行 Rust 编译/测试。
- 最后通过后只修改两处说明注释，并写本报告，没有再改行为。

## 没做到的 / 偏离任务卡的地方

- 未启动真实 App、浏览器或开发服务；没有真实库、真实 AI、真实同步和系统焦点验收。上述成功是代码与替身证据，不是 Fox 亲手验收。
- 现有测试依赖没有 DOM 运行环境。本卡未新增依赖，键盘/焦点测试直接执行组件的状态、effect、事件处理器，焦点对象为替身；真实浏览器的默认事件行为、视觉焦点和实际滚动仍需总控页面验收。
- 现有完成事件没有仓库 id，所以按四种相关任务的收尾事件清空 README 缓存，只对打开中的详情重新读取。不对每条运行进度失效，也不轮询/每次渲染读取；这是本卡保守选择。
- 一次同步会先发 `sync-stars` 成功，后续抓取 README 后才发 `readme-follow` 收尾，所以两阶段各失效一次，避免早期读到的空值再次粘住。
- 未执行 Git 命令、未改预算值/后端闸门、未做卡55重构、未接卡75的 `onInitialQuestionConsumed`。

## 需要总控或 Fox 决定的事

无新权限或依赖决定。总控可开始只读审查；卡75消费回调由总控后续接线。

## 给审核者的提示

- 页面替身事件必须使用真实契约：`{taskId:'fetch-readmes', taskType:'readme', status:'succeeded', stage:'done', ...}`，部分成功为 `partial`。`taskType:'fetch-readmes',status:'completed'` 与当前后端不符。
- 后端依据：`apps/desktop/src-tauri/src/lib.rs:153` 字段使用 camelCase；`:200` 成功构造器写 status `succeeded`；`:4411` 同步早期成功；`:4194` README 跟进收尾；`:4546` 单仓库完成；`:4734` 批量完成/部分成功。均只读源码确认。
- `readme-follow` 评估失败时可能已有 README 写入，故其 failed 收尾也失效；旧请求 rejection 只删除属于自己的 Promise，不能清新缓存。
- 未配置 AI 的横幅与无卡详情有明确 `actionTarget`，没有可用 AI 回调时不展示错路按钮。暂停/额度/开启入口保持原自动整理回调。
