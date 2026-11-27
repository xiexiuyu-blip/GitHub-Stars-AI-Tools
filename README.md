<div align="center">
  <img src="apps/desktop/public/icon.png" alt="Fox Stars Lab 应用图标" width="128" />
  <h1>Fox Stars Lab</h1>
  <p><strong>本地优先的 GitHub Stars 开源项目情报库与 AI 工具选型工作台</strong></p>
  <p>收进来 · 看得懂 · 找得到 · 选得准 · 交出去给 AI 用</p>
  <p>
    <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/license-PolyForm%20Noncommercial%201.0.0-111827?style=for-the-badge" /></a>
    <a href="https://tauri.app/"><img alt="Tauri" src="https://img.shields.io/badge/Tauri-2-24C8DB?style=for-the-badge&logo=tauri&logoColor=white" /></a>
    <a href="https://react.dev/"><img alt="React" src="https://img.shields.io/badge/React-19-087EA4?style=for-the-badge&logo=react&logoColor=white" /></a>
    <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/Rust-backend-B7410E?style=for-the-badge&logo=rust&logoColor=white" /></a>
  </p>
</div>

> **当前版本**：`2.1.1-fox.1` ｜ **默认主分支**：`main`（日常开发与发版基线）

Fox Stars Lab 基于 Tauri 2 + React 19 + Rust + SQLite 构建，围绕真实研发与效率选型场景演进，把“随手点的 GitHub Stars 和群聊/网页链接”沉淀为可检索、可对比、可直接生成 AI 助手交接单的本机情报库。

---

## 一、核心工作台（2.1 现行结构）

应用主界面收敛为 **5 大核心工作台 + 单层 6 页签设置**，无多余嵌套入口：

| 入口 | 核心职责 |
|---|---|
| **今日** | 展示需要优先关注与“值得先看”的收藏、整理进度与待办入口 |
| **资料库** | 按业务场景、我的状态（没看 / 想试 / 试过 / 在用 / 不要了）、中文说明状态、形态、维护状态多维筛选与排序；支持 `↑/↓` 与 `J/K` 导航、切页状态保持、查看/确认中文说明及一键「加入当前方案」 |
| **找方案** | 任务驱动检索：描述要解决的事（如“把群聊记录整理成日报”），结合本地匹配、维护状态提醒与可选 AI 精选，一键生成可直接复制给 Claude / Codex 等 AI 助手的**任务交接单**（Markdown / AGENTS.md / JSON） |
| **发现** | **热门趋势**（GitHub 今日飙升 / 本周热门 / 7 天新锐 / 30 天黑马 / HN Show 热门）、**自由搜索**（关键词 + 时间窗口 + 排序 + 语言）与 **按场景补缺**（经典高星 + 近 90 天新锐），支持复用已连接 GitHub 令牌 |
| **收集** | 整段群聊/帖子粘贴抽取 GitHub / 网页链接、抓取标题摘要、提取文中提及的 GitHub 项目，并支持通过本机 HTTP 接口接收外部投递 |
| **设置** | 单层 6 页签：**账号与同步** / **AI 设置** / **自动整理** / **备份与导出** / **外部连接** / **外观与关于** |

---

## 二、设计特性与边界

- **本地优先与凭据隔离**：收藏仓库、README 缓存、中文说明、笔记与收集箱全量保存在本机 SQLite（迁移 `001–039`）；GitHub 令牌与 AI 密钥写入系统钥匙串，不落明文配置。
- **AI 预算硬闸门**：`dailyBudgetTokens` 默认为 `0`（不自动花钱）。除用户在界面显式确认的「一次性全部整理」与「重新整理这一张」外，所有 AI 调用受滚动 24 小时额度闸门限制；无额度时资料库检索、收集、找方案基础匹配与交接单导出照常可用。
- **安全与只读兼容**：不可信网页/README 解析采用线性有限状态逻辑与安全切分；开发版（`com.foxwork.fox-stars-lab.dev`）与正式版（`com.foxwork.fox-stars-lab`）数据目录与钥匙串完全隔离。

---

## 三、项目全景文档地图

无论是新加入的团队同事、开源贡献者还是 AI 协作者，建议从以下 **现行唯一真源文档** 快速建立全局认知：

| 文档 | 内容定位 |
|---|---|
| [文档总目录](docs/README.md) | 区分**现行真源 / 运维规范 / 版本日志 / 历史归档**的索引总览 |
| [产品全景说明](docs/PRODUCT.md) | 产品演进脉络、核心用户旅程、六大模块详述、预算门禁与边界定义 |
| [代码与技术架构](docs/ARCHITECTURE.md) | Monorepo 目录职责、前后端 IPC 与数据流、SQLite 迁移、`include_str!` 约束与本地 API 契约 |
| [后续开发路线图](docs/ROADMAP.md) | 下一阶段演进顺序（检索核心与 MCP 只读接入 → 应用内收藏问答 RAG → 体验精修与跨平台发布） |
| [开发者上手与交接](docs/HANDOFF.md) | Day-1 环境搭建、dev 隔离启动、统一测试命令矩阵与正式版 D2 原地升级流程 |
| [2.1.0-fox.1 发布日志](docs/releases/v2.1.0-fox.1.md) | 2.0 重建至 2.1.0-fox.1 完整变更汇总与验证口径 |
| [视觉与文案规范](apps/desktop/DESIGN.md) | 色彩/排版/组件设计令牌与设置页文案写法规范 |
| [贡献指南](CONTRIBUTING.md) | 分支与 PR 流程、SQLite 迁移/额度闸门审阅规范与提测要求 |
| [安全策略](SECURITY.md) | 本地安全边界说明与私密漏洞报告渠道 |

---

## 四、快速开始

### 桌面端使用（最终用户）

- 系统要求：macOS 10.15+（当前主构建为 Apple Silicon）、Windows 10+ 或 Linux（安装包用户无需 Node.js / Rust）。
- 上手流程：启动应用 → 设置中连接 GitHub Personal Access Token → 同步 Stars 与缓存 README → 按需在「AI 设置」配置服务商并设定每日额度。
- **macOS 临时签名放行提示**：本地构建或未公证的测试安装包首次打开时，系统可能提示无法验证开发者。可在终端执行 `xattr -dr com.apple.quarantine "/Applications/Fox Stars Lab.app"` 后打开；若替换本地重编包，macOS 钥匙串首次读取凭据会提示一次授权，选择「始终允许」即可。

### 开发者本地运行与验证

环境要求：**Node.js >= 24**、**pnpm >= 11**、**Rust 工具链**（`cargo` / `rustfmt`）。

```bash
# 1. 启动隔离开发版（加载 dev 身份，不触碰正式库）
COREPACK_HOME="$PWD/.corepack" pnpm dev:desktop

# 2. 统一构建与回归验证入口（自动隔离网络与环境）
tools/cards/run.sh web-build     # 共享包 + 前端类型检查与 Vite 构建
tools/cards/run.sh web-test      # 前端 Vitest 单元测试（含文案守卫）
tools/cards/run.sh rust-fmt      # Rust 代码格式检查
tools/cards/run.sh rust-test     # Rust 全量单元测试（沙盒外排除 1 条钥匙串用例）
tools/cards/run.sh script-test   # 发布门禁脚本测试
tools/cards/run.sh ui-test       # Playwright 隔离端到端验收（见 tools/dev-ui/README.md）
```

> 请勿直接运行 `pnpm tauri dev`：该命令使用正式版 Bundle ID，会在调试期命中安全防护并退出。详细开发规范见 [CLAUDE.md](CLAUDE.md)、[AGENTS.md](AGENTS.md) 与 [docs/HANDOFF.md](docs/HANDOFF.md)。

---

## 五、许可证

本项目采用 [PolyForm Noncommercial License 1.0.0](LICENSE)。源码可用于个人学习、研究、非营利组织和非商业用途；涉及公司商业使用、商业再分发或商业产品集成前，须先核实并取得对应授权，同时保留原始许可证声明。

感谢 [Tauri](https://tauri.app/)、[React](https://react.dev/)、[Rust](https://www.rust-lang.org/)、[SQLite](https://www.sqlite.org/) 与相关开源社区项目。
