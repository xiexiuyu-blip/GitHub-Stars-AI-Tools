## 改动摘要
<!-- 用 1–3 句话说明本次 PR 解决的问题或新增的能力 -->

## 关联 Issue / 路线图
<!-- 例如：Closes #12，或对应 docs/ROADMAP.md 中的哪一项 -->

## 改动类型
- [ ] 功能新增（Feature）
- [ ] 缺陷修复（Bugfix）
- [ ] 结构重构 / 性能优化（Refactor / Perf）
- [ ] 文档 / 规范完善（Docs）
- [ ] 测试 / 构建脚本（Test / Chore）

## 高危边界与规范自查（若不涉及可保持不勾选并简注）
- [ ] 未改动既有 SQLite 迁移 `001–039`；如新增迁移，已基于最新 `main` 顺序递增并登记于 `migrations.rs`
- [ ] 未新增绕过 `spend_guard` / `jobs` 每日额度闸门的 AI 调用；付费能力具备降级方案
- [ ] 未包含个人本机路径、主机名、内网地址、Token 或 AI API Key
- [ ] 前端界面文案符合 `apps/desktop/DESIGN.md` 与 `docs/rebuild/文案审读.md`，且通过 `old-terms.test.ts`
- [ ] 未移动 `docs/rebuild/spec/`、`docs/quality/embedding-golden.json` 等编译期内嵌路径

## 本地验证清单
- [ ] `tools/cards/run.sh web-build`
- [ ] `tools/cards/run.sh web-test`
- [ ] `tools/cards/run.sh rust-fmt`
- [ ] `tools/cards/run.sh rust-test`
- [ ] `tools/cards/run.sh script-test`
- [ ] `tools/cards/run.sh ui-test`（涉及核心 UI 旅程时勾选）

## 验证结果 / 截图说明
<!-- 可贴上关键测试结果摘要、边界用例说明或界面截图 -->
