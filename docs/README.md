# Fox Stars Lab 文档总目录

> 本文档帮你快速分辨仓库里哪些文档是**现行唯一真源**，哪些属于**规范/运维工具**，哪些属于**历史归档记录**。若历史记录与代码或现行文档存在口径差异，**以现行真源与现行源码为准**。

---

## 一、现行唯一真源（推荐按顺序阅读）

| 文档 | 适合谁看 | 核心内容 |
|---|---|---|
| [../README.md](../README.md) | 所有人 | 项目首页定位、6 大工作台速览、快速启动与许可证说明 |
| [PRODUCT.md](PRODUCT.md) | 产品 / 研发 / 协作者 | `2.1.0-fox.1` 完整产品设计、用户旅程、功能细节、AI 额度闸门与边界 |
| [ARCHITECTURE.md](ARCHITECTURE.md) | 研发工程师 / AI 开发者 | Monorepo 结构、React ↔ Tauri IPC ↔ Rust ↔ SQLite 架构、命令契约与外部接口 |
| [ROADMAP.md](ROADMAP.md) | 产品 / 研发负责人 | 当前已落地里程碑与 P1 / P2 / P3 后续迭代路线（唯一有效计划） |
| [HANDOFF.md](HANDOFF.md) | 新接手同事 | Day-1 上手清单、开发运行、验证命令、发版与正式版原地升级入口 |
| [../CONTRIBUTING.md](../CONTRIBUTING.md) | 团队同事 / 开源贡献者 | 分支协作规则、PR 核对项、迁移冲突解决与贡献授权说明 |
| [../SECURITY.md](../SECURITY.md) | 所有协作者 / 安全研究者 | 核心安全边界、私密漏洞报告流程与凭据脱敏要求 |
| [../apps/desktop/DESIGN.md](../apps/desktop/DESIGN.md) | 前端 / 设计 | 语义色板、排版、组件规格与设置页文案写法规范 |

---

## 二、现行工程规范与测试资产

| 路径 | 用途与重要约束 |
|---|---|
| [rebuild/spec/](rebuild/spec/) | 编译期内嵌规格文件（`ai_card.schema.json`、`discover_queries.json`、`scenarios_seed.json`、`taxonomy_seed.json`）。由 Rust `include_str!` 直接编译，**禁止移动路径** |
| [quality/embedding-golden.json](quality/embedding-golden.json) | `embedding.rs` 单测内嵌的检索黄金评测集，**禁止移动路径** |
| [quality/local-vector-search-acceptance.md](quality/local-vector-search-acceptance.md) | 本地向量检索实测验收标准（供 P1 上线混合语义检索时参考） |
| [licenses/embedding-runtime.md](licenses/embedding-runtime.md) | 本地 Embedding 运行时依赖的许可证与来源说明 |
| [rebuild/切库清单.md](rebuild/切库清单.md) | 正式版 **D2 原地升级 SOP**（备份 → 迁移演练 → 构建 → 替换 → 只读核验 → 回滚）及历史升级实录 |
| [rebuild/文案审读.md](rebuild/文案审读.md) | 全应用统一中文用词表与多轮界面文案审读台账 |
| [rebuild/微信联动接入.md](rebuild/微信联动接入.md) | 外部工具通过本地 API `POST /v1/capture` 向收集箱投递链接的接口说明 |

---

## 三、版本发布日志

- **[releases/v2.1.0-fox.1.md](releases/v2.1.0-fox.1.md)**：`2.0` 重建至 `2.1.0-fox.1` 的完整演进与验证记录
- 早期 1.x 发布日志保留在 [releases/](releases/)（`v0.1.0.md` 至 `v1.5.3.md`）

---

## 四、历史归档（仅作溯源，不再作为现行口径）

- **[rebuild/README.md](rebuild/README.md)**：`docs/rebuild/` 目录索引，区分现行规格文件与 2.0 初期设计稿、任务卡（`任务卡/`）、原型与 MCP 草案。
- **[archive/README.md](archive/README.md)**：`docs/archive/v1/` 目录索引，集中保存上游 v0.1–v1.5.3 阶段的分析、规划、阶段报告与旧版截图。
