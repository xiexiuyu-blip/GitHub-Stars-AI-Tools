---
编号: 77-release-acceptance
允许改动:
  - tools/cards/release.sh
  - tools/cards/merge-card.sh
  - tools/cards/run.sh
  - tools/cards/release_test.py
  - tools/dev-ui/acceptance.mjs
  - tools/dev-ui/acceptance-fixture.js
  - tools/dev-ui/README.md
  - apps/desktop/src/App.tsx
  - apps/desktop/src/pages/v2/solve.tsx
  - apps/desktop/src/components/solve/hit-drawer.tsx
  - apps/desktop/src/pages/settings.test.tsx
  - docs/rebuild/**
  - AGENTS.md
---

# 卡 77：发布门禁与集成验收（总控）

2026-10-28，依 Fox 本批授权执行。总控是上述文件最终写入者；三张开发卡收回后才补跨卡接线，其他 agent 只读审查。

- 发布与合并脚本：构建或测试退出非零立即停止；合法成功输出不依赖 grep 恰好匹配。Rust 通过既有 run.sh 排除钥匙串测试。
- 发布预算：无法读取、文件缺失/目录/断链、字段缺失/非法均失败关闭；非零大整数不能绕过。保留既有需要明确授权的 RESET/KEEP 参数，不在本批运行它们。
- 原版构建与隔离页面对照；修复后完整前端/后台、格式与类型检查、发布故障演练、20 项浏览器操作。
- App 新问题消费回调；找方案未整理工具详情到 AI 设置的完整接线。
- 本地集成与文档记录，不推送、不打 tag、不开 PR；语义检索下一批。
- 未运行原生 App、未接触真实开发/正式库、凭据或真实 AI。发布及原生实机验收等待 Fox 放行后追加结果。

证据见 AI 工具中台 `70_执行记录/Fox Stars Lab_批次A验收结果_20261028.md` 及 `_logs/batch-a-20261028/`。
