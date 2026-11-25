# Fox Stars Lab 开发与交付交接指南（2.1.0-fox.1）

> **本文档为团队协作单入口总览**，适用于接手开发的同事与 AI 协作者。历史阶段的设计草稿、任务卡和切库实录仍保留在 `docs/rebuild/` 下作为溯源记录，遇口径差异时以本文与现行源码为准。

---

## 1. 当前基线与核心升级摘要

- **当前版本**：`2.1.0-fox.1`
- **主开发分支**：`fox-rebuild`（日常开发与正式构建基线）；`fox-integration` 与之对齐
- **M1–M5 阶段升级内容**：
  1. **页面与路由精简（M1）**：删除已淘汰的旧版独立页面与「按类型看」重复视图，收口为 **今日 / 资料库 / 找方案 / 发现 / 收集 / 设置** 6 个核心入口。
  2. **单层设置与动态费用估算（M2）**：将原 `settings.tsx` 与 `toolbox.tsx` 合并拆分为单层 6 页签模块化设置（`apps/desktop/src/pages/settings/`：`账号与同步` / `AI 设置` / `自动整理` / `备份与导出` / `外部连接` / `外观与关于`）；后端 `card_handler.rs` 增加基于最近 50 次成功任务的动态 `avgTokensPerCard` 估算。
  3. **发现工作台全面升级（M3）**：
     - 新增 SQLite 迁移 `039_discover_feeds`（`discover_feed_items` / `discover_feed_runs`）与 3 个 Tauri 命令（`list_discover_feed`、`refresh_discover_feed`、`search_discover_github`）；
     - 支持 **热门趋势**（GitHub Trending 今日/本周含 Search API 兜底、7 天/30 天新锐榜、HN Show GitHub 项目热榜）、**自由搜索**（关键词 + 时间窗口 + 排序 + 语言）与 **按场景补缺**（保留近 90 天新锐名额）；
     - 对 `api.github.com` 安全复用已连接 GitHub 令牌（401 时无令牌降级重试）。
  4. **通用化文案与交互打磨（M4）**：界面展示的个人称呼去个人化（通用称呼与 `AI 助手`、动态读取 GitHub 用户名），详情页首屏瘦身、明确「确认（不再自动覆盖）」语义、新增「加入当前方案」、列表支持 `↑ / ↓` 与 `J / K` 导航、资料库切页保留筛选条件与滚动位置。
  5. **交付收尾（M5）**：版本升至 `2.1.0-fox.1`，正式版通过 D2 流程原地升级并完成双远端同步。

---

## 2. 开发环境、运行与测试入口

- **依赖环境**：Node.js 24+、pnpm 11、Rust 工具链（`cargo` / `rustfmt`）。
- **构建与测试唯一入口**：统一通过 `tools/cards/run.sh` 执行（自动锁定本地 `.corepack` 且禁网防装包冲突）：
  ```bash
  tools/cards/run.sh web-build      # 构建 packages + 前端 tsc & vite build
  tools/cards/run.sh web-test       # Vitest 全部前端单测（含文案守卫 old-terms.test.ts）
  tools/cards/run.sh rust-fmt       # Rust 代码格式检查（修复用 rust-fmt-fix）
  tools/cards/run.sh rust-test      # Rust 全量单元测试（除 1 条依赖系统钥匙串的用例）
  tools/cards/run.sh script-test    # 工具脚本 Python unittest
  tools/cards/run.sh ui-test        # 基于隔离夹具的 Playwright 端到端旅程验收
  ```
- **日常开发启动**（使用 `.dev` 隔离数据目录，不会碰正式版数据）：
  ```bash
  pnpm dev:desktop
  ```

---

## 3. 代码结构导览

- **前端应用**（`apps/desktop/src/`）：
  - `pages/v2/{today,library,solve,discover,capture}.tsx`：五大业务工作台
  - `pages/settings/{index,shared,github-section,ai-section,organize-section,backup-section,connect-section,general-about-section}.tsx`：单层 6 页签设置模块
  - `lib/{contracts,api,item-format,discover-view,library-filters,library-session,solve-view,solve-draft,automation-view}.ts`：契约类型、命令封装与纯视图逻辑
  - `lib/codex-prompts.ts`：复制给外部 AI 助手的任务交接提示词模板
  - `lib/old-terms.test.ts`：新界面禁用旧词静态扫描守卫
- **Rust 后端**（`apps/desktop/src-tauri/src/`）：
  - `lib.rs`：Tauri 命令注册与桌面生命周期
  - `auth.rs`：macOS Keychain / 凭据读取与 GitHub 令牌校验（含 `optional_github_token()`）
  - `db/migrations.rs` & `packages/storage/migrations/fox/001..039_*.sql`：SQLite 顺序迁移与完整性校验
  - `discover.rs` & `discover_feeds.rs`：按场景补缺、Trending HTML 线性解析、新锐榜、HN Show 与自由搜索
  - `card_handler.rs` / `card_pipeline.rs` / `jobs.rs` / `spend_guard.rs`：AI 队列、理解卡流水线与每日 token 硬额度闸门
  - `solve.rs` / `solve_rerank.rs` / `packs.rs`：意图匹配、AI 精选与交接单构建
- **内嵌规格文件**（`docs/rebuild/spec/`，由 `include_str!` 编译进二进制，**禁止移动路径**）：
  - `scenarios_seed.json`、`taxonomy_seed.json`、`ai_card.schema.json`、`discover_queries.json`

---

## 4. 数据隔离、安全边界与许可证声明

1. **正式库 vs 开发库隔离**：
   - 正式版标识 `com.foxwork.fox-stars-lab`，数据位于 `~/Library/Application Support/com.foxwork.fox-stars-lab/`。
   - 开发版标识 `com.foxwork.fox-stars-lab.dev`，钥匙串与本地数据完全独立。开发调试只读写 `.dev` 目录；正式版升级必须严格遵循 [切库清单](docs/rebuild/切库清单.md) 中的 **D2 原地升级流程**（先退出 → 完整备份 + SHA-256 校验 → 副本迁移演练 → 构建核验 → 替换 App → 只读核对）。
2. **令牌、密钥与额度安全**：
   - 用户的 SQLite 库、GitHub Token 与 AI Key 均保存在本地应用支持目录和系统钥匙串，**不在源码仓库内**。新接手同事使用自己的账号与测试数据启动即可。
   - 不要绕过 `spend_guard` 或在升级/初始化时修改 `dailyBudgetTokens`（正式环境默认为 `0`，由用户在设置里主动调整）。
3. **开源许可证合规提示**：
   - 本项目基于上游代码演进，仓库声明采用 **PolyForm Noncommercial 1.0.0**。涉及公司商业使用、对外再分发或产品化集成前，须先核实授权情况并保留对应许可证文本。

---

## 5. 历史文档索引（`docs/rebuild/`）

| 文档 | 用途与定位 |
|---|---|
| [docs/rebuild/切库清单.md](docs/rebuild/切库清单.md) | **现行有效**：正式版 D2 原地升级固定流程与每次升级实录 |
| [docs/rebuild/文案审读.md](docs/rebuild/文案审读.md) | 统一用词表与多轮文案审读问题台账 |
| [docs/rebuild/命令契约.md](docs/rebuild/命令契约.md) | V2 前后端命令契约参考 |
| [docs/rebuild/后续开发计划.md](docs/rebuild/后续开发计划.md) | 远期演进方向参考（检索增强 / AI 助手接入 / 问答） |
| [docs/rebuild/接手说明.md](docs/rebuild/接手说明.md) | 早期重建与批次 A 交接记录（历史归档参考） |
| [docs/rebuild/产品重设计与开发说明书.md](docs/rebuild/产品重设计与开发说明书.md) | 2.0 初始产品设计说明书（部分界面已在 2.1 中继续合并精简） |
| [docs/rebuild/任务卡/README.md](docs/rebuild/任务卡/README.md) | 历史开发任务卡目录 |
