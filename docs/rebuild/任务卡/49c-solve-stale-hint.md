---
编号: 49c-solve-stale-hint
允许改动:
  - apps/desktop/src-tauri/src/solve.rs
  - apps/desktop/src-tauri/src/packs.rs
  - apps/desktop/src/lib/contracts.ts
  - apps/desktop/src/lib/solve-view.ts
  - apps/desktop/src/lib/solve-view.test.ts
  - apps/desktop/src/components/solve/solve-hit-row.tsx
  - apps/desktop/src/components/solve/solve-hit-row.test.tsx
  - apps/desktop/src/lib/maintenance.ts
  - apps/desktop/src/lib/maintenance.test.ts
---

# 卡 49c-solve-stale-hint · 找方案与交接单维护状态提示

> 2026-10-28：Fox 已明确批准本批“现有开发版验收、关键问题修复、49c”，由当前 Codex 总控和多 agent 执行。语义检索下一批；不启动正式/开发App、不读真实数据、不碰凭据、不调用真实AI。总控沿用项目已授权的本地Git集成流程。

## 范围与验收

1. SolveHit 加 pushedAt 与 archived（保留可缺省/未知兼容），全部命中及后备候选保持字段；读取已有repositories列，不加迁移、不改同步。
2. 找方案行复用维护规则：已归档优先、放缓/疑似停更按已有边界、未知不误报。提醒不改变排序、选择、筛选，不把旧工具自动排除。
3. 交接单三种格式保留维护提醒：沿用现有cautions/risks等兼容字段，优先不新增JSON schema键；从DB事实生成，不相信前端传入。归档优先、不制造更新/安全结论。
4. 覆盖活跃/放缓/停更/已归档/未知、近期仍归档、边界时间、moreHits、无理解卡、JSON/Markdown/AGENTS输出；其他排名与选择不退步。
5. 不修改 solve.tsx/App.tsx，无新增依赖、不联网、不真实AI。

## 交回

只在分配的隔离工作区修改；按 tools/cards/run.sh 做适当验证并写 .card-report.md。不要Git写操作、不要在主工作区改文件。报告区分已测/未测/限制。

