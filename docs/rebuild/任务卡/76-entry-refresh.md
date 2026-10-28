---
编号: 76-entry-refresh
允许改动:
  - apps/desktop/src/App.tsx
  - apps/desktop/src/pages/settings.tsx
  - apps/desktop/src/components/command-palette.tsx
  - apps/desktop/src/components/command-palette.test.tsx
  - apps/desktop/src/hooks/use-repo-readme.ts
  - apps/desktop/src/hooks/use-repo-readme.test.ts
  - apps/desktop/src/hooks/use-stars-workspace.ts
  - apps/desktop/src/hooks/use-stars-workspace.test.ts
  - apps/desktop/src/providers/task-center.tsx
  - apps/desktop/src/providers/task-progress-provider.tsx
  - apps/desktop/src/lib/api.ts
  - apps/desktop/src/lib/automation-view.ts
  - apps/desktop/src/lib/automation-view.test.ts
  - apps/desktop/src/components/library/automation-banner.tsx
  - apps/desktop/src/components/library/item-detail.tsx
  - apps/desktop/src/pages/v2/today.tsx
  - apps/desktop/src/pages/v2/library.tsx
  - apps/desktop/src/components/library/today-page.test.tsx
  - apps/desktop/src/components/library/item-detail.test.tsx
  - apps/desktop/src/pages/settings.test.tsx
---

# 卡 76-entry-refresh · 设置入口、搜索键盘与README刷新

> 2026-10-28：Fox 已明确批准本批“现有开发版验收、关键问题修复、49c”，由当前 Codex 总控和多 agent 执行。语义检索下一批；不启动正式/开发App、不读真实数据、不碰凭据、不调用真实AI。总控沿用项目已授权的本地Git集成流程。

## 范围与验收

1. 未配置AI的“去设置AI”必须直达设置AI配置页，其他暂停/额度等入口仍去对应自动整理；最小参数接线，不重构设置。SolvePage的onOpenToolbox仅原样保留（另卡负责solve.tsx）。
2. 命令面板普通词Enter进行资料库搜索，方向键选择、Enter打开、Esc关闭，焦点管理与恢复；不触发真实同步/AI测试。避免旧搜索请求污染新查询结果。
3. README成功null或旧结果缓存必须在同步/README更新后失效，并让仍打开的详情刷新。找到正确现有事件来源，针对性失效；失败可重试。不得靠每次渲染反复拉取。
4. 自动整理费用文案将“每天”明确滚动24小时，关闭只表示后台自动整理关闭，不承诺全App免费；修最小误导，不改预算实际值/门禁。
5. 基于替身数据测试上述行为，不加依赖。App.tsx最后由总控补75的新问题消费回调，其他人不同时写此文件。发现实际任务进度provider路径不在允许列表时先告知总控扩范围。

## 交回

只在分配的隔离工作区修改；按 tools/cards/run.sh 做适当验证并写 .card-report.md。不要Git写操作、不要在主工作区改文件。报告区分已测/未测/限制。

