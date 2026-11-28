# Fox Stars Lab 本地开发与 CI 接入指南

项目已配置 GitHub Actions，并开启了 `main` 分支保护。日常协作流程是：**本地提交 → 推送开发分支 → 在 GitHub 开启指向 `main` 的草稿 PR → 查看 Checks → PR 检查全绿并确认后在 GitHub 合并 → 拉回本地并同步公司内网 `seeway` 远端**。同一个 PR 开好后，后续每次推送该分支都会自动触发新一轮检查。详见 [项目 CI 配置](../.github/workflows/ci.yml)。

---

## 1. 从本地开发接入 CI

1. 在仓库根目录工作。开始新任务时，从最新的 `main` 创建开发分支：

   ```bash
   git switch main
   git pull --ff-only origin main
   git switch -c feat/my-change
   ```

2. 完成修改并本地验证通过后，查看改动、提交并推送到 `origin`（如需团队内网预览也可同步推 `seeway`）：

   ```bash
   git status --short
   git diff
   git add apps/desktop/src/pages/v2/capture.tsx
   git commit -m "feat(capture): 更新收集页"
   git push -u origin feat/my-change
   ```

3. 前往 GitHub 仓库进入 **Pull requests → New pull request**（或使用 `gh pr create --draft`），把 **base** 设为 `main`，**compare** 设为你的开发分支，创建 **Draft pull request**。在 PR 的 **Checks** 页签查看自动化检查结果。
4. 若需要继续修改，只需在同一分支追加提交并 `git push`，即会自动更新 PR 并重跑 CI。
5. **合并与双远端对齐**：因 `main` 受分支保护规则约束，禁止直接向 `main` 强推或本地合并后直推，需在 PR 检查全部通过并在 GitHub 完成 PR 合并后，回到本机同步双远端：

   ```bash
   git switch main
   git pull --ff-only origin main
   git push seeway main
   ```

> 如需在本地接续或核对他人已推送的开发分支，可先执行 `git fetch origin`，再用 `git switch --track origin/<branch>` 建立本地跟踪分支。

---

## 2. 了解触发条件和检查结果

| 你的操作 | 当前工作流如何运行 |
|---|---|
| 推送 `feat/**` / `fix/**` 开发分支（即使还没开 PR） | 自动运行全套 4 项 Checks |
| 创建指向 `main` 的 PR（含 Draft），或向该 PR 推送新提交 | 自动运行全套 4 项 Checks（若同时属于 `feat/**` / `fix/**`，会各自产生一次 `push` 与 `pull_request` 检查） |
| PR 合入 `main` | 自动运行全套校验，守卫主线质量 |
| 其他分支想在开 PR 前单独验证 | 有仓库写权限时，在 **Actions → CI → Run workflow** 选择对应分支手动触发 |

在 PR 的 **Checks** 或仓库 **Actions** 中会看到以下 4 项作业（定义见 [ci.yml](../.github/workflows/ci.yml)）：

| 检查名称 | 检查内容 |
|---|---|
| `Web Build, Unit Tests & Hygiene` | 共享包与前端 TypeScript/Vite 构建、Vitest 单测（含文案守卫）、脚本测试及文档脱敏检查 |
| `Rust Format Check` | Rust 源码 `cargo fmt --check` 格式校验 |
| `Rust Check & Unit Tests` | Linux 环境 Cargo 编译检查与全量后端单测（跳过 1 条依赖 macOS 钥匙串的用例） |
| `UI Browser Acceptance` | Chromium 接入隔离测试夹具跑端到端 UI 验收（含窄窗口、深色模式、收集/发现/资料库回归等） |

检查失败时，先点开第一个失败步骤查看日志。如果 `UI Browser Acceptance` 失败，可在该次运行的 **Artifacts** 中下载 `ui-acceptance-failure`（包含 `results.json` 与失败现场截图）。修复并推回同一分支后，PR 上会自动更新新一轮结果。

> **平台说明**：GitHub CI 在 `ubuntu-24.04` 上完成构建与全套回归。仓库内遗留的上游发布工作流目前未启用 Fox Stars Lab 正式签名；macOS 正式桌面版的签名打包与原地升级仍以 [切库清单 D2 流程](rebuild/切库清单.md) 为准。

---

## 3. 推送前在本地跑全套检查

准备好 Node.js `>= 24`、pnpm `>= 11` 与 Rust 稳定版工具链后，在仓库根目录**统一使用 `tools/cards/run.sh`**（内置离线 `COREPACK_HOME="$PWD/.corepack"` 与防误连网配置，详见 [HANDOFF.md](HANDOFF.md)）：

```bash
# 如需首次在本机/CI 安装依赖，使用仓库内置 Corepack 离线配置：
COREPACK_HOME="$PWD/.corepack" COREPACK_ENABLE_NETWORK=0 pnpm install --frozen-lockfile

tools/cards/run.sh web-build
tools/cards/run.sh web-test
tools/cards/run.sh rust-fmt
tools/cards/run.sh rust-check
tools/cards/run.sh rust-test
tools/cards/run.sh script-test
```

本机运行 Playwright 隔离浏览器验收前，需先完成 `tools/cards/run.sh web-build`，再参照 [浏览器验收说明](../tools/dev-ui/README.md) 指定本地 Chrome / Playwright 路径并执行 `tools/cards/run.sh ui-test`。

---

## 4. 让修过的 Bug 持续受 CI 保护

1. 复现问题时，顺手记下所在页面、操作顺序、实际结果与预期结果。
2. 用 [acceptance-fixture.js](../tools/dev-ui/acceptance-fixture.js) 中的确定性测试替身或临时 SQLite 构造先失败、再通过的回归测试：
   - **界面/状态竞态/交互流程**：补进 [acceptance.mjs](../tools/dev-ui/acceptance.mjs) 或前端 Vitest 单测；
   - **数据结构/迁移/去重/接口契约**：补进对应 Rust 模块单测或 `db/migrations.rs` 升级测试。
3. 将测试随修复一并推送；后续每个 PR 都会在 CI 中自动执行。
4. 需要 Codex 协助排查失败 CI 时，可在对话中附上 PR 或 Actions 运行链接及预期表现，例如：

> 请查看这个 PR 的 CI 失败日志（附链接），在本地复现并补一条回归测试，修复验证后推送到原开发分支，我确认后再合并。

---

## 5. 自动触发规则（已开启 `feat/**` 与 `fix/**` 自动检查）

当前 [ci.yml](../.github/workflows/ci.yml) 已在 `on.push.branches` 中开启 `main`、`feat/**` 与 `fix/**`：

```yaml
on:
  pull_request:
    branches: [main]
  push:
    branches:
      - main
      - 'feat/**'
      - 'fix/**'
  workflow_dispatch:
```

- **尚未建 PR 的 `feat/**` / `fix/**` 分支**：只要推送就会自动触发 CI，适合 Codex 云端分支先行自检。
- **已经建 PR 的分支**：一次推送会同时触发 `push` 和 `pull_request` 两次检查，两者相互独立（`concurrency` 按 `github.ref` 隔离，不会互相取消），合并 PR 时以 `pull_request` 状态检查为准。
- 如后续团队想减少重复运行、改为仅开 PR 后跑一次，可删去 `'feat/**'` 与 `'fix/**'` 仅保留 `main` 即可。

同时，[AGENTS.md](../AGENTS.md) 第三节「PR 自动代码审查必查规则」会在 Codex 自动审查 PR 或 `@codex review` 时生效（包括新 Tauri 命令权限同步、SSRF/凭据边界、AI 额度闸门、跨平台兼容与文案守卫）。
