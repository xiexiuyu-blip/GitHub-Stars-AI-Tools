---
编号: 75-solve-continuity
允许改动:
  - apps/desktop/src/pages/v2/solve.tsx
  - apps/desktop/src/lib/solve-draft.ts
  - apps/desktop/src/lib/solve-draft.test.ts
  - apps/desktop/src/lib/solve-pack-state.ts
  - apps/desktop/src/lib/solve-pack-state.test.ts
  - apps/desktop/src/components/solve/solve-page.test.tsx
---

# 卡 75-solve-continuity · 交接单当前版本与返回状态、AI精选最小费用说明

> 2026-10-28：Fox 已明确批准本批“现有开发版验收、关键问题修复、49c”，由当前 Codex 总控和多 agent 执行。语义检索下一批；不启动正式/开发App、不读真实数据、不碰凭据、不调用真实AI。总控沿用项目已授权的本地Git集成流程。

## 范围与验收

1. 交接单输出必须对应当前问题、选中工具及顺序、格式、模式、本机能力；旧预览可以保留但明确上一版，未就绪/失败不能复制旧内容，旧请求不得覆盖新结果。
2. 返回找方案保留会话内勾选；带入问题必须区分新任务与返回。推荐新增 onInitialQuestionConsumed 回调由总控接 App.tsx（不要改 App.tsx）。同一个问题被明确再次带入仍可视为新请求。可优先修消费一次；重启持久化不在本批。
3. AI精选开关就近说明会使用对应AI额度，等待中关闭只关闭后续精选、已发请求可能仍计费；不得改变后端门禁，不发真实请求。
4. 覆盖换问题/相同工具、换格式、改选、生成失败、请求乱序、离开返回、已有草稿与新问题。使用真实行为测试优先，不仅源码字符串断言。
5. 不加依赖，不改 contracts/solve-view/App.tsx（其他负责人拥有）。需要额外路径先告知总控。

## 交回

只在分配的隔离工作区修改；按 tools/cards/run.sh 做适当验证并写 .card-report.md。不要Git写操作、不要在主工作区改文件。报告区分已测/未测/限制。

