# Fox Stars Lab 开发者上手与交付指南（2.1.0-fox.1）

> 本文档为新加入同事与协作者的 **Day-1 实操入口**。产品功能全貌见 [PRODUCT.md](PRODUCT.md)，技术架构见 [ARCHITECTURE.md](ARCHITECTURE.md)，后续规划见 [ROADMAP.md](ROADMAP.md)。

---

## 1. 环境准备

- **必备工具链**：
  - Node.js `>= 24.0.0`
  - pnpm `>= 11.0.0`（仓库根目录内置 `.corepack` 支持零网络稳定调用）
  - Rust 稳定版工具链（`cargo`、`rustc`、`rustfmt`）
  - SQLite 3
  - macOS 构建还需 Xcode Command Line Tools
- **仓库分支约定**：
  - `main`：默认主分支，当前与 `2.1.0-fox.1` 现役正式版完全对齐；
  - `fox-rebuild` / `fox-integration`：2.x 演进与集成辅助分支，当前与 `main` 保持同步；
  - 历史版本基线标签：`upstream-base`（分叉前上游基线）、`v2.1.0-fox.1`（当前正式版发版基线）。

---

## 2. 日常开发启动（安全隔离）

开发调试必须通过以下命令启动 **dev 身份**（`com.foxwork.fox-stars-lab.dev`），其数据库与钥匙串和正式版完全分开，不会影响正式数据：

```bash
# 完整桌面开发模式（前端热更新 + Rust 后端 dev 身份）
COREPACK_HOME="$PWD/.corepack" pnpm dev:desktop
```

> **禁止运行 `pnpm tauri dev`**：该命令默认读取正式版 `tauri.conf.json`，若解析到正式目录或以 debug 身份运行会被启动守卫（`dev_guard.rs`）拒绝退出。

新同事本地开发直接使用自己的 GitHub Token 与测试数据启动即可，用户的 SQLite 库与凭据不包含在 Git 仓库内。

---

## 3. 构建与全量测试矩阵

提交代码或发起合并前，统一使用根目录测试脚本 `tools/cards/run.sh` 执行验证（自动配置离线 `.corepack` 与环境变量）：

```bash
tools/cards/run.sh web-build      # 构建 packages/* 并检查/打包桌面端前端
tools/cards/run.sh web-test       # 运行全部 Vitest 前端单元测试（含文案守卫 old-terms.test.ts）
tools/cards/run.sh rust-fmt       # Rust 代码格式校验（自动修复可用 rust-fmt-fix）
tools/cards/run.sh rust-check     # Rust 后端增量编译检查
tools/cards/run.sh rust-test      # Rust 后端全量单测（排除 1 条依赖系统钥匙串的用例）
tools/cards/run.sh script-test    # 工具链预算与发布门禁脚本测试（release_test.py）
```

如需运行基于 Playwright 与 `mock-host` 替身的隔离端到端浏览器验收（20 条核心用户旅程），需本机安装 Chrome 及 Playwright，并参考 [tools/dev-ui/README.md](../tools/dev-ui/README.md) 执行 `tools/cards/run.sh ui-test`。

---

## 4. 代码修改与提交必查红线

1. **不动正式数据**：开发与测试代码严禁读写正式数据目录（macOS 下为 `~/Library/Application Support/com.foxwork.fox-stars-lab/`）。
2. **不绕过预算闸门**：任何新增 AI 对话调用必须走后台队列（`jobs.rs`）或经过 `spend_guard.rs`，不得新增绕过 `dailyBudgetTokens` 闸门的自动花钱路径。
3. **迁移只追加**：SQLite 迁移编号严禁改动历史 `001–039`，新变更在 `packages/storage/migrations/fox/` 追加 `040_*.sql` 并在 `apps/desktop/src-tauri/src/db/migrations.rs` 登记。
4. **保持内嵌路径与公共列**：禁止移动 `docs/rebuild/spec/`、`docs/quality/embedding-golden.json` 路径；禁止重命名或删除 `repositories`、`repo_ai_documents`、`repo_tags`、`annotations` 与 `v_public_*` 既有字段。
5. **遵守界面文案约定**：新增或修改设置项与界面提示前，先核对 [apps/desktop/DESIGN.md](../apps/desktop/DESIGN.md)「文案与设置页写法」与 [docs/rebuild/文案审读.md](rebuild/文案审读.md)「统一用词表」，保证通过 `old-terms.test.ts`。

---

## 5. 本地打包与正式版 D2 原地升级

- **打包本地 `.app` / `.dmg`**：
  ```bash
  COREPACK_HOME="$PWD/.corepack" pnpm package:desktop
  # 或生成带 Finder 图标布局的 DMG
  COREPACK_HOME="$PWD/.corepack" pnpm package:desktop:dmg
  ```
- **在装有真实正式数据的机器上升级正式版**：必须严格按照 [docs/rebuild/切库清单.md](rebuild/切库清单.md) 中的 **D2 原地升级流程** 执行（退出正式进程 → 全量目录与旧 App 备份及 SHA-256 核对 → 临时库迁移演练 → 构建校验 → 替换 `.app` → 启动后只读检查迁移号、完整性与业务表行数；切勿覆盖既有 `fox-stars-lab.queue.json` 预算设置）。
- **跨平台与许可证提示**：如向使用 Windows 或 Intel Mac 的同事分发安装包，需在对应目标平台编译验证；商业用途或对外商业分发前，请先核实 [LICENSE](../LICENSE)（PolyForm Noncommercial 1.0.0）授权范围。
