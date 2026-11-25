# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) and human/AI contributors when working in this repository.

**开始编码前，请先阅读 [docs/README.md](docs/README.md)、[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)、[docs/HANDOFF.md](docs/HANDOFF.md) 与 [AGENTS.md](AGENTS.md)。**

---

## 项目概述

Fox Stars Lab（当前版本 `2.1.0-fox.1`）是一个基于 Tauri 2 + React 19 + Rust + SQLite 的本地优先 GitHub Stars 开源项目情报库与 AI 工具选型工作台，采用 pnpm monorepo 架构。完整产品设计与边界见 [docs/PRODUCT.md](docs/PRODUCT.md)。

---

## 环境要求

- Node.js `>= 24`
- pnpm `>= 11`
- Rust 稳定版（`cargo`、`rustc`、`rustfmt`）
- SQLite 3
- macOS 需要 Xcode Command Line Tools

所有 pnpm 命令须使用仓库内置离线配置前缀：`COREPACK_HOME="$PWD/.corepack"`（或直接调用 `tools/cards/run.sh`）。

---

## 常用命令

### 开发与启动

```bash
# 启动开发版桌面应用（使用 com.foxwork.fox-stars-lab.dev 隔离身份，不碰正式库）
COREPACK_HOME="$PWD/.corepack" pnpm dev:desktop

# 仅启动前端 Vite 开发服务器（无桌面后端能力）
COREPACK_HOME="$PWD/.corepack" pnpm dev
```

> 不要用 `pnpm tauri dev`：它加载正式版 Bundle ID，debug 构建会被 `dev_guard.rs` 直接拒绝退出。

### 构建与自动化验收（推荐入口）

```bash
tools/cards/run.sh web-build     # 构建 packages/* 与桌面前端（tsc + vite build）
tools/cards/run.sh web-test      # 前端 Vitest 单元测试（含文案守卫 old-terms.test.ts）
tools/cards/run.sh rust-fmt      # 检查 Rust 代码格式（修复可用 rust-fmt-fix）
tools/cards/run.sh rust-check    # Rust 后端编译检查
tools/cards/run.sh rust-test     # Rust 后端全量单测（排除 1 条依赖系统钥匙串的用例）
tools/cards/run.sh script-test   # 发布门禁脚本测试（tools/cards/release_test.py）
tools/cards/run.sh ui-test       # Playwright 隔离端到端验收（用法见 tools/dev-ui/README.md）
```

### 打包

```bash
COREPACK_HOME="$PWD/.corepack" pnpm package:desktop
COREPACK_HOME="$PWD/.corepack" pnpm package:desktop:dmg
```

---

## 架构概览

```text
apps/desktop/
  src/
    pages/v2/          # 五大业务工作台：today / library / solve / discover / capture
    pages/settings/    # 单层 6 页签设置模块：github / ai / organize / backup / connect / general
    components/        # UI 基础件（kit / ui）与业务展示组件
    lib/               # 契约（contracts.ts / api.ts）、纯视图逻辑与 old-terms 文案守卫
    hooks/             # 复用 Hooks（键盘导航、自动整理轮询、单张重写、README 缓存失效）
    features/          # 上游兼容遗留组件（新界面统一落在 pages/v2 与 pages/settings）
  src-tauri/src/       # Rust 后端：auth / db/migrations / card_pipeline / jobs / spend_guard / solve / discover* / local_api
packages/{ai,domain,github,search,storage,worker}/
```

更完整的模块索引、IPC/本地 HTTP 接口契约与 `include_str!` 内嵌文件路径约束见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。

---

## 核心工程约定

### 1. 数据隔离与凭据安全

- 开发只使用 dev 身份：`com.foxwork.fox-stars-lab.dev`（产品名 `Fox Stars Lab Dev`，Keychain 服务名 `fox-stars-lab-dev`）。
- 正式数据目录 `~/Library/Application Support/com.foxwork.fox-stars-lab/` 与系统钥匙串仅由正式版 App 与用户本人管理，开发或测试代码严禁读取或修改；正式版升级严格遵循 [docs/rebuild/切库清单.md](docs/rebuild/切库清单.md) 的 D2 原地升级流程。
- GitHub Token 与 AI API Key 仅存系统钥匙串，不写入 `localStorage`、代码、日志或提交记录。

### 2. Schema 与迁移 discipline

- `packages/storage/migrations/fox/001–018` 为历史冻结基线，**严禁修改**；当前已按顺序追加至 **`039_discover_feeds.sql`**，统一登记在 `apps/desktop/src-tauri/src/db/migrations.rs` 的 `MIGRATIONS` 数组中。
- 新增表或字段只能在 `packages/storage/migrations/fox/` 顺序追加（下一个为 `040_*.sql`），并在 `MIGRATIONS` 中登记、补测试；启动迁移自动备份并校验 `PRAGMA integrity_check`。
- 保持 `repositories`、`repo_ai_documents`、`repo_tags`、`annotations` 现有列与 `v_public_*` 视图向后兼容（不改名、不删除）。

### 3. 界面文案与设计守卫

- 修改 UI 文案或新增设置项前，先阅读 [apps/desktop/DESIGN.md](apps/desktop/DESIGN.md)「文案与设置页写法」与 [docs/rebuild/文案审读.md](docs/rebuild/文案审读.md)「统一用词表」，确保 `old-terms.test.ts` 通过。

---

## 当前演进入口

- 当前版本：`2.1.0-fox.1`（默认主分支 `main`，`fox-rebuild` 与 `fox-integration` 保持对齐）
- 产品全景：[docs/PRODUCT.md](docs/PRODUCT.md)
- 后续开发计划：[docs/ROADMAP.md](docs/ROADMAP.md)
- 开发者交接清单：[docs/HANDOFF.md](docs/HANDOFF.md)
