# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

这是一个基于 Tauri 2 的本地优先 GitHub Stars 管理桌面应用，采用 pnpm monorepo 架构。核心能力包括：GitHub Stars 同步到本地 SQLite、README 抓取与缓存、AI 中文摘要生成、自然语言搜索、标签与笔记管理、Gist 注解同步。

## 环境要求

- Node.js >= 24
- pnpm >= 11
- Rust (用于 Tauri 后端)
- SQLite 3
- macOS 需要 Xcode Command Line Tools

所有命令必须设置 `COREPACK_HOME="$PWD/.corepack"` 前缀。

## 常用命令

### 开发与启动

```bash
# 启动开发版桌面应用。必须用这条，会加载 dev 身份，不会打开正式库。
COREPACK_HOME="$PWD/.corepack" pnpm dev:desktop

# 仅启动前端开发服务器（无桌面能力）
COREPACK_HOME="$PWD/.corepack" pnpm dev
```

不要用 `pnpm tauri dev`。那条命令仍是正式 bundle id，debug 构建会被启动检查直接退出。

### 构建与检查

```bash
# 完整构建流程
COREPACK_HOME="$PWD/.corepack" pnpm build:packages  # 构建共享包
COREPACK_HOME="$PWD/.corepack" pnpm build           # 构建桌面应用

# Rust 后端检查
cargo fmt --manifest-path "$PWD/apps/desktop/src-tauri/Cargo.toml" --check
cargo check --manifest-path "$PWD/apps/desktop/src-tauri/Cargo.toml"

# Rust 格式化修复
cargo fmt --manifest-path "$PWD/apps/desktop/src-tauri/Cargo.toml"
```

### 打包

```bash
# 生成安装包
COREPACK_HOME="$PWD/.corepack" pnpm tauri build

# 查看打包产物
open apps/desktop/src-tauri/target/release/bundle/
```

### 提交前检查顺序

```bash
COREPACK_HOME="$PWD/.corepack" pnpm build:packages
COREPACK_HOME="$PWD/.corepack" pnpm build
cargo fmt --manifest-path "$PWD/apps/desktop/src-tauri/Cargo.toml" --check
cargo check --manifest-path "$PWD/apps/desktop/src-tauri/Cargo.toml"
```

## 架构概览

### Monorepo 结构

```
apps/desktop/              # Tauri 桌面应用（React + Vite 前端 + Rust 后端）
  src/                     # React 前端代码
    features/              # 按功能模块拆分：repositories、knowledge、sidebar
  src-tauri/               # Rust 后端
    src/
      lib.rs               # Tauri 命令注册
      auth.rs              # GitHub Token 本地存储（macOS Keychain）
      github.rs            # GitHub API 调用
      storage.rs           # SQLite 操作封装

packages/domain/           # 领域类型定义（TypeScript）
packages/storage/          # SQLite Schema 与初始化脚本
  migrations/              # SQL Schema 文件（如 001_initial_schema.sql）
packages/github/           # GitHub API 接入边界
packages/ai/               # AI Provider 抽象层
packages/search/           # 搜索 DSL 与查询结构
packages/worker/           # 同步、README 缓存与 AI 摘要任务编排
```

### 数据流与职责边界

1. **前端 (React)**: 用户交互、状态管理、调用 Tauri 命令
2. **Tauri 命令 (Rust)**: 安全沙箱层，处理 GitHub Token、SQLite 读写、系统调用
3. **共享包 (TypeScript)**: 跨前后端的类型定义与业务逻辑
4. **SQLite**: 本地事实数据存储（仓库元信息、README、AI 摘要、标签、用户注解）

### 关键领域模型 (packages/domain)

- `RepositoryFacts`: GitHub 仓库元信息（owner、name、stars、topics、syncStatus）
- `RepositoryAnnotation`: 用户注解层（tags、notes、readStatus）
- `ReadmeDocument`: README 原始 Markdown 与缓存元信息
- `AiRepositoryDocument`: AI 生成的中文摘要、关键词、建议标签
- `RepoQuery`: 统一查询结构，支持 keyword、natural_language、hybrid 三种模式

### Tauri 命令边界

Rust 后端通过 Tauri 命令暴露给前端，主要模块：

- **auth.rs**: `save_github_token`、`load_github_token`（macOS Keychain 读写）
- **github.rs**: 调用 GitHub API（同步 Stars、抓取 README）
- **storage.rs**: SQLite CRUD 操作（仓库查询、标签管理、注解读写）

前端通过 `@tauri-apps/api` 的 `invoke` 调用这些命令。

### Schema 与迁移（S3 / S4）

Fox 的正式库已经有 001–018。这些文件在 `packages/storage/migrations/fox/`，一个字都不能改，也不能删库重建。

禁止：

- 删除、移动、覆盖 `~/Library/Application Support/com.foxwork.fox-stars-lab/` 里的任何文件
- `DROP` Fox 已有的表
- 对正式目录执行 `clear_local_database` 或旧库删除
- 用 `INSERT OR IGNORE` 写 `schema_migrations`
- 把 upstream 的 002/003 直接套到 Fox 库上。Fox 的 002/003 已经是别的迁移

规则：

- 新迁移从 019 开始，只追加。草案在 `docs/rebuild/migrations/proposed/`
- upstream 以后的迁移改编成新的 Fox 编号，名字加 `upstream_` 前缀
- runner 必须核对版本号和名称都匹配。名称不符就拒绝启动
- 每次迁移前自动备份，并做 `PRAGMA integrity_check`
- 每个迁移单独一个事务
- 开发只用 dev 身份：`com.foxwork.fox-stars-lab.dev`，产品名 `Fox Stars Lab Dev`，Keychain 服务名 `fox-stars-lab-dev`
- debug 构建或 dev 身份如果解析到正式数据目录，必须立即退出

P1 会把现在的 upstream `migrate()` 换成这套版本化 runner。在那之前，不要对正式库启动本仓库。

### 前端架构约定

- **功能模块**: `apps/desktop/src/features/` 按业务领域拆分（repositories、knowledge、sidebar）
- **UI 组件库**: Radix UI + Tailwind CSS + shadcn/ui
- **状态管理**: React Hooks（无全局状态库）
- **样式**: Tailwind 4 + CSS Variables，使用 `@tailwindcss/vite` 插件

### GitHub Token 安全策略

- Token 存储: macOS Keychain（通过 Rust `security-framework` crate）
- Token 仅用于本地 API 调用，不上传到任何远程服务
- 禁止在代码、日志、截图、issue 中暴露 Token

### Gist 注解同步机制

仅同步用户注解层数据（tags、repo_tags、notes、read_status），不包括：

- GitHub 仓库事实数据
- README 缓存
- AI 摘要、标签网络与自然语言搜索计划
- 完整 SQLite 数据库

导入时仅合并到本地已存在的仓库记录。

## 开发注意事项

### 修改前端代码

- 前端代码位于 `apps/desktop/src/`
- 修改后运行 `COREPACK_HOME="$PWD/.corepack" pnpm dev:desktop` 查看效果。不要用 `pnpm tauri dev`。
- TypeScript 类型错误会在 `pnpm build` 时暴露

### 修改 Rust 后端

- 后端代码位于 `apps/desktop/src-tauri/src/`
- 修改后通过 `cargo check` 验证编译
- 修改 Tauri 命令签名时，需同步更新前端调用代码

### 修改共享包

- 共享包位于 `packages/*/src/`
- 修改后必须运行 `pnpm build:packages` 重新构建
- `packages/domain` 的类型定义是前后端契约，变更需谨慎

### 修改数据库 Schema

1. 在 `packages/storage/migrations/` 下新建 `.sql` 文件
2. 文件名按序号递增（如 `002_add_new_table.sql`）
3. 更新后端必需表字段检测
4. 更新 `packages/domain` 中的 TypeScript 类型
5. 运行 `pnpm build` 和 `cargo check --manifest-path "$PWD/apps/desktop/src-tauri/Cargo.toml"` 检查

### 清理 Rust 缓存

如果 Rust 编译报错提示旧路径或依赖冲突：

```bash
cargo clean --manifest-path "$PWD/apps/desktop/src-tauri/Cargo.toml"
cargo check --manifest-path "$PWD/apps/desktop/src-tauri/Cargo.toml"
```

## 当前开发阶段

已完成 Phase 1-6.2（基础设施、GitHub 同步、Star 管理、AI 知识库、增量同步、Gist 同步）。

下一步：Phase 6.3（成本与任务监控）。
