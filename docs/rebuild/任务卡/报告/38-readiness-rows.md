# 交回报告 · 卡 38-readiness-rows

## 一句话结论
已完成：设置页运行检查只显示和统计六项，移除两项已停止检查的类型与标题，并新增源码守卫测试。

## 验收逐条
| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| `tools/cards/run.sh web-build`、`web-test` 通过（`web-test` 在沙盒里跑不了时，说明原因，总控在沙盒外跑）。 | 是 | `tools/cards/run.sh web-build` → exit 0，TypeScript 检查通过、Vite 构建完成；`tools/cards/run.sh web-test` → 33 个测试文件通过，770 项通过。 |
| `git diff --stat` 只出现「允许改动」里的文件。 | 是 | `git diff --stat` 显示的 4 个已跟踪文件均在允许清单；`git status --short` 另显示新增 `apps/desktop/src/pages/settings.test.tsx`，也在允许清单。 |
| 报告里写：条号 → 文件:行；第 6 条 `grep` 的输出里范围外的残留清单；没把握的地方。 | 是 | 本报告“范围外残留”和“给审核者的提示”列出扫描范围外条目及行号；逐项证据见下。 |

## 改了哪些文件
- `apps/desktop/src/pages/settings.tsx`：第 783–788 行只渲染六项；第 817–825 行汇总只计六项。
- `apps/desktop/src/types.ts`：第 312–319 行的 `RuntimeReadinessCheckResult` 只保留六个字段。
- `apps/desktop/src/lib/automation-view.ts`：第 720–731 行标题映射只保留六项，并移除兼容键说明。
- `apps/desktop/src/lib/automation-view.test.ts`：第 1066–1076 行断言标题映射严格为六项，保留旧叫法断言。
- `apps/desktop/src/pages/settings.test.tsx`：第 1–21 行通过 `?raw` 守护设置页不再读取停止检查字段，并确认六项标题引用及两个设置项仍保留。

## 验证
- `tools/cards/run.sh web-build` → 通过，TypeScript 检查通过，Vite 构建完成（exit 0）。
- `tools/cards/run.sh web-test` → 通过，33 个测试文件、770 项通过（exit 0）。
- `git status --short` → 4 个修改文件 + 1 个新增文件，均属于允许改动路径。
- `git diff --stat` → 已跟踪变更只涉及允许清单内的 4 个文件；新增测试文件由 `git status` 确认在允许清单内。
- `rg -n "tagNetwork|recommendation" apps/desktop/src` → 设置页不再读取运行检查字段；范围外匹配如下。

## 没做到的 / 偏离任务卡的地方
- 无。

## 需要总控或 Fox 决定的事
- 无。

## 给审核者的提示
- 第 6 条扫描的范围外残留（按 `rg -n "tagNetwork|recommendation" apps/desktop/src` 输出）：
  - `apps/desktop/src/pages/settings.tsx:78,80`：设置项 `tagNetworkBatchSize`、`tagNetworkPromptVersion`，本卡要求保留。
  - `apps/desktop/src/pages/tag-network.tsx:266,273,309,321,327,329,331`：独立标签网络页面文案/变量，超出允许改动范围。
  - `apps/desktop/src/pages/repositories.tsx:165,297,324,691,707,805`：资料库推荐选择与动作逻辑，超出允许改动范围。
  - `apps/desktop/src/pages/discover.tsx:5,6,57`、`apps/desktop/src/features/discovery/recommendation-detail-panel.tsx:45,88`、`apps/desktop/src/hooks/use-stars-workspace.ts:381,1437,1460,1465,1475,1486,1496`、`apps/desktop/src/App.tsx:678`：发现页 GitHub 推荐候选功能及命令，超出允许改动范围。
- 源码守卫针对运行检查字段引用做精确断言，同时单独确认设置项字段仍在，避免把任务卡明确要求保留的设置误判为残留。

---

## 总控审核（2026-10-05，Claude）

- **执行者**：Codex（`gpt-6-luna`，high），12:04:50 派出、12:06 交回（约 2 分钟，10 条命令）。
- **范围**：`check-scope.sh` 5 个文件全在允许范围内；没有 `.tmp-card/`（派发脚本新加的专用 `TMPDIR` 生效，前端构建和测试在沙盒里直接跑通）。
- **日志审计**：10 条命令里没有碰正式 / 开发数据目录、钥匙串、密钥，没有 git 写命令，没有联网。
- **diff**：只做了任务卡写的事——设置页删两行 `RuntimeCheckRow` 并让小结只算六项、`types.ts` 去掉两个键、`RUNTIME_CHECK_TITLES` 去掉两个键、测试改成「只有六项」，外加一个 `?raw` 源码守卫测试（不误伤 `tagNetworkBatchSize` / `tagNetworkPromptVersion` 两个设置项）。后端仍回传 `tagNetwork` / `recommendation` 两个键（固定「没检查」），前端不再读，多出来的键无害；Rust 里那两个字段和 `readiness_retired` 留到以后顺手清（backlog）。
- **报告里列出的范围外残留**（`tag-network.tsx`、`repositories.tsx`、`discover.tsx` 等里的 `tagNetwork` / `recommendation`）是别的功能（标签网络页、发现页的 GitHub 推荐），与「运行检查」无关，不处理。
